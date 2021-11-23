// Copyright 2018-2021 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;

#[cfg(feature = "diesel")]
use diesel::r2d2::{ConnectionManager, Pool};
use splinter::error::InternalError;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use transact::state::merkle::sql::{
    self,
    store::{MerkleRadixStore, SqlMerkleRadixStore},
};
use transact::{
    database::Database,
    state::{
        merkle::{kv, MerkleRadixLeafReadError, MerkleRadixLeafReader},
        Prune, Read, StateChange, StatePruneError, StateReadError, StateWriteError, Write,
    },
};

pub enum MerkleStateConfig {
    KeyValue {
        database: Box<dyn Database>,
    },
    #[cfg(feature = "postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
        tree_name: String,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
        tree_name: String,
    },
}

impl MerkleStateConfig {
    pub fn key_value(database: Box<dyn Database>) -> Self {
        Self::KeyValue { database }
    }
}

#[derive(Clone)]
pub enum MerkleState {
    KeyValue {
        state: kv::MerkleState,
        initial_state_root: String,
        database: Box<dyn Database>,
    },
    #[cfg(feature = "postgres")]
    SqlPostgres {
        state: sql::SqlMerkleState<sql::backend::PostgresBackend>,
    },
    #[cfg(feature = "sqlite")]
    SqlSqlite {
        state: sql::SqlMerkleState<sql::backend::SqliteBackend>,
    },
}

impl MerkleState {
    pub fn new(merkle_state_config: MerkleStateConfig) -> Result<Self, InternalError> {
        match merkle_state_config {
            MerkleStateConfig::KeyValue { database } => {
                let initial_state_root = kv::MerkleRadixTree::new(database.clone_box(), None)
                    .map_err(|e| InternalError::from_source(Box::new(e)))?
                    .get_merkle_root();
                let state = kv::MerkleState::new(database.clone_box());
                Ok(MerkleState::KeyValue {
                    state,
                    initial_state_root,
                    database,
                })
            }
            #[cfg(feature = "postgres")]
            MerkleStateConfig::Postgres { pool, tree_name } => {
                let postgres_backend = sql::backend::PostgresBackend::from(pool);

                let state = sql::SqlMerkleStateBuilder::new()
                    .with_backend(postgres_backend)
                    .with_tree(tree_name)
                    .create_tree_if_necessary()
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;

                Ok(MerkleState::SqlPostgres { state })
            }
            #[cfg(feature = "sqlite")]
            MerkleStateConfig::Sqlite { pool, tree_name } => {
                let sqlite_backend = sql::backend::SqliteBackend::from(pool);

                let state = sql::SqlMerkleStateBuilder::new()
                    .with_backend(sqlite_backend)
                    .with_tree(tree_name)
                    .create_tree_if_necessary()
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;

                Ok(MerkleState::SqlSqlite { state })
            }
        }
    }

    /// Check to see if the merkle state exists, based on the underlying implementation's criteria.
    pub fn check_existence(merkle_state_config: &MerkleStateConfig) -> bool {
        match merkle_state_config {
            // These are always auto-generated
            MerkleStateConfig::KeyValue { .. } => true,
            #[cfg(feature = "postgres")]
            MerkleStateConfig::Postgres { pool, tree_name } => {
                let postgres_backend = sql::backend::PostgresBackend::from(pool.clone());

                // This will fail if the tree does not previously exist.
                sql::SqlMerkleStateBuilder::new()
                    .with_backend(postgres_backend)
                    .with_tree(tree_name)
                    .build()
                    .is_ok()
            }
            #[cfg(feature = "sqlite")]
            MerkleStateConfig::Sqlite { pool, tree_name } => {
                let sqlite_backend = sql::backend::SqliteBackend::from(pool.clone());

                // This will fail if the tree does not previously exist.
                sql::SqlMerkleStateBuilder::new()
                    .with_backend(sqlite_backend)
                    .with_tree(tree_name)
                    .build()
                    .is_ok()
            }
        }
    }

    pub fn get_initial_state_root(&self) -> Result<String, InternalError> {
        match self {
            MerkleState::KeyValue {
                initial_state_root, ..
            } => Ok(initial_state_root.clone()),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state
                .initial_state_root_hash()
                .map_err(|err| InternalError::from_source(Box::new(err))),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state
                .initial_state_root_hash()
                .map_err(|err| InternalError::from_source(Box::new(err))),
        }
    }
}

impl Read for MerkleState {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;

    fn get(
        &self,
        state_id: &Self::StateId,
        keys: &[Self::Key],
    ) -> Result<HashMap<Self::Key, Self::Value>, StateReadError> {
        match self {
            MerkleState::KeyValue { state, .. } => state.get(state_id, keys),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state.get(state_id, keys),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state.get(state_id, keys),
        }
    }

    fn clone_box(
        &self,
    ) -> Box<dyn Read<StateId = Self::StateId, Key = Self::Key, Value = Self::Value>> {
        Box::new(self.clone())
    }
}

type IterResult<T> = Result<T, MerkleRadixLeafReadError>;
type LeafIter<T> = Box<dyn Iterator<Item = IterResult<T>>>;

impl MerkleRadixLeafReader for MerkleState {
    fn leaves(
        &self,
        state_id: &Self::StateId,
        subtree: Option<&str>,
    ) -> IterResult<LeafIter<(Self::Key, Self::Value)>> {
        match self {
            MerkleState::KeyValue { state, .. } => state.leaves(state_id, subtree),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state.leaves(state_id, subtree),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state.leaves(state_id, subtree),
        }
    }
}

impl Write for MerkleState {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;

    fn commit(
        &self,
        state_id: &Self::StateId,
        state_changes: &[StateChange],
    ) -> Result<Self::StateId, StateWriteError> {
        match self {
            MerkleState::KeyValue { state, .. } => state.commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state.commit(state_id, state_changes),
        }
    }

    fn compute_state_id(
        &self,
        state_id: &Self::StateId,
        state_changes: &[StateChange],
    ) -> Result<Self::StateId, StateWriteError> {
        match self {
            MerkleState::KeyValue { state, .. } => state.compute_state_id(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state.compute_state_id(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state.compute_state_id(state_id, state_changes),
        }
    }
}

impl Prune for MerkleState {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;

    fn prune(&self, state_ids: Vec<Self::StateId>) -> Result<Vec<Self::Key>, StatePruneError> {
        match self {
            MerkleState::KeyValue { state, .. } => state.prune(state_ids),
            #[cfg(feature = "postgres")]
            MerkleState::SqlPostgres { state } => state.prune(state_ids),
            #[cfg(feature = "sqlite")]
            MerkleState::SqlSqlite { state } => state.prune(state_ids),
        }
    }
}

#[cfg(feature = "sqlite")]
pub fn sqlite_list_available_trees(
    pool: &Pool<ConnectionManager<diesel::SqliteConnection>>,
) -> Result<Vec<String>, InternalError> {
    let sqlite_backend = sql::backend::SqliteBackend::from(pool.clone());
    SqlMerkleRadixStore::new(&sqlite_backend)
        .list_trees()
        .and_then(|iter| iter.collect::<Result<Vec<_>, _>>())
        .map_err(|e| InternalError::from_source(Box::new(e)))
}

#[cfg(feature = "postgres")]
pub fn postgres_list_available_trees(
    pool: &Pool<ConnectionManager<diesel::pg::PgConnection>>,
) -> Result<Vec<String>, InternalError> {
    let postgres_backend = sql::backend::PostgresBackend::from(pool.clone());
    SqlMerkleRadixStore::new(&postgres_backend)
        .list_trees()
        .and_then(|iter| iter.collect::<Result<Vec<_>, _>>())
        .map_err(|e| InternalError::from_source(Box::new(e)))
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

    /// Check for the existence of a tree:
    ///
    /// 1. Create a connection pool
    /// 2. Create a merkle state config with the pool
    /// 3. Check that the config is shown not to exist
    /// 4. Use that config to create a state
    /// 5. Verify that an identical config is shown to exist
    #[test]
    fn check_existence() -> TestResult<()> {
        let pool = create_connection_pool()?;

        let config = MerkleStateConfig::Sqlite {
            pool: pool.clone(),
            tree_name: "test_check_existence".into(),
        };

        assert!(
            !MerkleState::check_existence(&config),
            "Tree in DB should not exist"
        );

        // this should create the state
        let _ = MerkleState::new(config)?;

        let config = MerkleStateConfig::Sqlite {
            pool,
            tree_name: "test_check_existence".into(),
        };
        assert!(
            MerkleState::check_existence(&config),
            "Tree in DB should exist"
        );

        Ok(())
    }

    fn create_connection_pool() -> TestResult<Pool<ConnectionManager<diesel::SqliteConnection>>> {
        let connection_manager = ConnectionManager::<diesel::SqliteConnection>::new(":memory:");
        let pool = Pool::builder().max_size(1).build(connection_manager)?;

        crate::migrations::run_sqlite_migrations(&*pool.get()?)?;

        Ok(pool)
    }
}
