// Copyright 2018-2022 Cargill Incorporated
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

use diesel::r2d2::{ConnectionManager, Pool};
use scabbard::store::transact::factory::LmdbDatabaseFactory;
use transact::state::{
    merkle::{
        kv::MerkleState as TransactMerkleState,
        sql::{
            backend,
            store::{MerkleRadixStore, SqlMerkleRadixStore},
            SqlMerkleState,
        },
        MerkleRadixLeafReadError, MerkleRadixLeafReader,
    },
    Prune, Read, StateChange, StatePruneError, StateReadError, StateWriteError, Write,
};

use super::CliError;

#[derive(Clone)]
pub enum MerkleState {
    Lmdb {
        state: TransactMerkleState,
        merkle_root: String,
        tree_id: (String, String),
    },
    /// Configure scabbard storage using a shared Postgres connection pool.
    #[cfg(feature = "postgres")]
    Postgres {
        state: SqlMerkleState<backend::PostgresBackend>,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        state: SqlMerkleState<backend::SqliteBackend>,
    },
}

impl std::fmt::Debug for MerkleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_tuple("MerkleState");
        match self {
            MerkleState::Lmdb { .. } => debug_struct.field(&"Lmdb".to_string()),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { .. } => debug_struct.field(&"Postgres".to_string()),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { .. } => debug_struct.field(&"Sqlite".to_string()),
        };
        debug_struct.finish()
    }
}

impl MerkleState {
    pub fn get_state_root(&self) -> Result<String, CliError> {
        match self {
            // lmdb provides current state root,
            MerkleState::Lmdb { merkle_root, .. } => Ok(merkle_root.to_string()),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
        }
    }

    pub fn delete_tree(self, lmdb_db_factory: &LmdbDatabaseFactory) -> Result<(), CliError> {
        match self {
            MerkleState::Lmdb { tree_id, .. } => {
                let (circuit_id, service_id) = tree_id;
                lmdb_db_factory
                    .get_database_purge_handle(&circuit_id, &service_id)
                    .map_err(|e| CliError::ActionError(format!("{}", e)))?
                    .purge()
                    .map_err(|e| CliError::ActionError(format!("{}", e)))
            }
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
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
            MerkleState::Lmdb { state, .. } => state.commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.commit(state_id, state_changes),
        }
    }

    fn compute_state_id(
        &self,
        state_id: &Self::StateId,
        state_changes: &[StateChange],
    ) -> Result<Self::StateId, StateWriteError> {
        match self {
            MerkleState::Lmdb { state, .. } => state.compute_state_id(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.compute_state_id(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.compute_state_id(state_id, state_changes),
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
            MerkleState::Lmdb { state, .. } => state.get(state_id, keys),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.get(state_id, keys),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.get(state_id, keys),
        }
    }

    fn clone_box(
        &self,
    ) -> Box<dyn Read<StateId = Self::StateId, Key = Self::Key, Value = Self::Value>> {
        Box::new(self.clone())
    }
}

impl Prune for MerkleState {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;

    fn prune(&self, state_ids: Vec<Self::StateId>) -> Result<Vec<Self::Key>, StatePruneError> {
        match self {
            MerkleState::Lmdb { state, .. } => state.prune(state_ids),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.prune(state_ids),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.prune(state_ids),
        }
    }
}

// These types make the clippy happy
type IterResult<T> = Result<T, MerkleRadixLeafReadError>;
type LeafIter<T> = Box<dyn Iterator<Item = IterResult<T>>>;

impl MerkleRadixLeafReader for MerkleState {
    fn leaves(
        &self,
        state_id: &Self::StateId,
        subtree: Option<&str>,
    ) -> IterResult<LeafIter<(Self::Key, Self::Value)>> {
        match self {
            MerkleState::Lmdb { state, .. } => state.leaves(state_id, subtree),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.leaves(state_id, subtree),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.leaves(state_id, subtree),
        }
    }
}

#[cfg(feature = "sqlite")]
pub fn sqlite_list_available_trees(
    pool: &Pool<ConnectionManager<diesel::SqliteConnection>>,
) -> Result<Vec<String>, CliError> {
    let sqlite_backend = backend::SqliteBackend::from(pool.clone());
    SqlMerkleRadixStore::new(&sqlite_backend)
        .list_trees()
        .and_then(|iter| iter.collect::<Result<Vec<_>, _>>())
        .map_err(|e| CliError::ActionError(format!("{}", e)))
}

#[cfg(feature = "postgres")]
pub fn postgres_list_available_trees(
    pool: &Pool<ConnectionManager<diesel::pg::PgConnection>>,
) -> Result<Vec<String>, CliError> {
    let postgres_backend = backend::PostgresBackend::from(pool.clone());
    SqlMerkleRadixStore::new(&postgres_backend)
        .list_trees()
        .and_then(|iter| iter.collect::<Result<Vec<_>, _>>())
        .map_err(|e| CliError::ActionError(format!("{}", e)))
}
