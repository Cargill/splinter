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

mod lmdb;

use std::collections::HashMap;

use diesel::r2d2::{ConnectionManager, Pool};
use scabbard::store::transact::factory::LmdbDatabaseFactory;
use splinter::error::InternalError;
use transact::state::{
    merkle::sql::{
        backend,
        store::{MerkleRadixStore, SqlMerkleRadixStore},
        SqlMerkleState,
    },
    Committer, DryRunCommitter, Pruner, Reader, State, StateChange, StateError, ValueIter,
    ValueIterResult,
};

use super::CliError;
use super::StateTreeStore;

pub use lmdb::LazyLmdbMerkleState;

pub enum MerkleState<'a> {
    Lmdb {
        state: LazyLmdbMerkleState,
    },
    /// Configure scabbard storage using a shared Postgres connection pool.
    #[cfg(feature = "postgres")]
    Postgres {
        state: SqlMerkleState<backend::PostgresBackend>,
    },
    #[cfg(feature = "postgres")]
    InTransactionPostgres {
        state: SqlMerkleState<backend::InTransactionPostgresBackend<'a>>,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        state: SqlMerkleState<backend::SqliteBackend>,
    },
    #[cfg(feature = "sqlite")]
    InTransactionSqlite {
        state: SqlMerkleState<backend::InTransactionSqliteBackend<'a>>,
    },
}

impl<'a> std::fmt::Debug for MerkleState<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_tuple("MerkleState");
        match self {
            MerkleState::Lmdb { .. } => debug_struct.field(&"Lmdb".to_string()),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { .. } => debug_struct.field(&"Postgres".to_string()),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { .. } => {
                debug_struct.field(&"Postgres (in transaction)".to_string())
            }
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { .. } => debug_struct.field(&"Sqlite".to_string()),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { .. } => {
                debug_struct.field(&"Sqlite (in transaction)".to_string())
            }
        };
        debug_struct.finish()
    }
}

impl<'a> MerkleState<'a> {
    pub fn get_state_root(&self) -> Result<String, CliError> {
        match self {
            // lmdb provides current state root,
            MerkleState::Lmdb { state } => Ok(state.get_state_root()),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state
                .initial_state_root_hash()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
        }
    }

    pub fn delete_tree(self) -> Result<(), CliError> {
        match self {
            MerkleState::Lmdb { state } => state
                .delete()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state
                .delete_tree()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
        }
    }

    pub fn remove_pruned_entries(&self) -> Result<(), CliError> {
        match self {
            // No-op, as LMDB
            MerkleState::Lmdb { .. } => Ok(()),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state
                .remove_pruned_entries()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state
                .remove_pruned_entries()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state
                .remove_pruned_entries()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state
                .remove_pruned_entries()
                .map_err(|e| CliError::ActionError(format!("{}", e))),
        }
    }
}

impl<'a> State for MerkleState<'a> {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;
}

impl<'a> Committer for MerkleState<'a> {
    type StateChange = StateChange;

    fn commit(
        &self,
        state_id: &Self::StateId,
        state_changes: &[Self::StateChange],
    ) -> Result<Self::StateId, StateError> {
        match self {
            MerkleState::Lmdb { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.commit(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state.commit(state_id, state_changes),
        }
    }
}

impl<'a> DryRunCommitter for MerkleState<'a> {
    type StateChange = StateChange;

    fn dry_run_commit(
        &self,
        state_id: &Self::StateId,
        state_changes: &[Self::StateChange],
    ) -> Result<Self::StateId, StateError> {
        match self {
            MerkleState::Lmdb { state } => state.dry_run_commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.dry_run_commit(state_id, state_changes),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => {
                state.dry_run_commit(state_id, state_changes)
            }
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.dry_run_commit(state_id, state_changes),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => {
                state.dry_run_commit(state_id, state_changes)
            }
        }
    }
}

impl<'a> Reader for MerkleState<'a> {
    /// The filter used for the iterating over state values.
    type Filter = str;

    fn get(
        &self,
        state_id: &Self::StateId,
        keys: &[Self::Key],
    ) -> Result<HashMap<Self::Key, Self::Value>, StateError> {
        match self {
            MerkleState::Lmdb { state } => state.get(state_id, keys),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.get(state_id, keys),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state.get(state_id, keys),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.get(state_id, keys),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state.get(state_id, keys),
        }
    }

    fn filter_iter(
        &self,
        state_id: &Self::StateId,
        filter: Option<&Self::Filter>,
    ) -> ValueIterResult<ValueIter<(Self::Key, Self::Value)>> {
        match self {
            MerkleState::Lmdb { state } => state.filter_iter(state_id, filter),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.filter_iter(state_id, filter),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state.filter_iter(state_id, filter),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.filter_iter(state_id, filter),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state.filter_iter(state_id, filter),
        }
    }
}

impl<'a> Pruner for MerkleState<'a> {
    fn prune(&self, state_ids: Vec<Self::StateId>) -> Result<Vec<Self::Key>, StateError> {
        match self {
            MerkleState::Lmdb { state } => state.prune(state_ids),
            #[cfg(feature = "postgres")]
            MerkleState::Postgres { state } => state.prune(state_ids),
            #[cfg(feature = "postgres")]
            MerkleState::InTransactionPostgres { state } => state.prune(state_ids),
            #[cfg(feature = "sqlite")]
            MerkleState::Sqlite { state } => state.prune(state_ids),
            #[cfg(feature = "sqlite")]
            MerkleState::InTransactionSqlite { state } => state.prune(state_ids),
        }
    }
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub struct DieselStateTreeStore<C: diesel::Connection + 'static> {
    pool: Pool<ConnectionManager<C>>,
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
impl<C: diesel::Connection + 'static> DieselStateTreeStore<C> {
    pub fn new(pool: Pool<ConnectionManager<C>>) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "sqlite")]
impl StateTreeStore for DieselStateTreeStore<diesel::SqliteConnection> {
    fn has_tree(&self, circuit_id: &str, service_id: &str) -> Result<bool, InternalError> {
        let sqlite_backend = backend::SqliteBackend::from(self.pool.clone());
        let tree_name = format!("{}::{}", circuit_id, service_id);
        let iter = SqlMerkleRadixStore::new(&sqlite_backend)
            .list_trees()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        for tree_id in iter {
            if tree_id.map_err(|e| InternalError::from_source(Box::new(e)))? == tree_name {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(feature = "postgres")]
impl StateTreeStore for DieselStateTreeStore<diesel::pg::PgConnection> {
    fn has_tree(&self, circuit_id: &str, service_id: &str) -> Result<bool, InternalError> {
        let postgres_backend = backend::PostgresBackend::from(self.pool.clone());
        let tree_name = format!("{}::{}", circuit_id, service_id);
        let iter = SqlMerkleRadixStore::new(&postgres_backend)
            .list_trees()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        for tree_id in iter {
            if tree_id.map_err(|e| InternalError::from_source(Box::new(e)))? == tree_name {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub struct DieselInTransactionStateTreeStore<'a, C: diesel::Connection> {
    conn: &'a C,
}

#[cfg(any(feature = "postgres", feature = "sqlite"))]
impl<'a, C: diesel::Connection> DieselInTransactionStateTreeStore<'a, C> {
    pub fn new(conn: &'a C) -> Self {
        Self { conn }
    }
}

#[cfg(feature = "postgres")]
impl<'a> StateTreeStore for DieselInTransactionStateTreeStore<'a, diesel::pg::PgConnection> {
    fn has_tree(&self, circuit_id: &str, service_id: &str) -> Result<bool, InternalError> {
        let postgres_backend = backend::InTransactionPostgresBackend::from(self.conn);
        let tree_name = format!("{}::{}", circuit_id, service_id);
        let iter = SqlMerkleRadixStore::new(&postgres_backend)
            .list_trees()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        for tree_id in iter {
            if tree_id.map_err(|e| InternalError::from_source(Box::new(e)))? == tree_name {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(feature = "sqlite")]
impl<'a> StateTreeStore for DieselInTransactionStateTreeStore<'a, diesel::SqliteConnection> {
    fn has_tree(&self, circuit_id: &str, service_id: &str) -> Result<bool, InternalError> {
        let sqlite_backend = backend::InTransactionSqliteBackend::from(self.conn);
        let tree_name = format!("{}::{}", circuit_id, service_id);
        let iter = SqlMerkleRadixStore::new(&sqlite_backend)
            .list_trees()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        for tree_id in iter {
            if tree_id.map_err(|e| InternalError::from_source(Box::new(e)))? == tree_name {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub struct LmdbStateTreeStore {
    lmdb_db_factory: LmdbDatabaseFactory,
}

impl LmdbStateTreeStore {
    pub fn new(lmdb_db_factory: LmdbDatabaseFactory) -> Self {
        Self { lmdb_db_factory }
    }
}

impl StateTreeStore for LmdbStateTreeStore {
    fn has_tree(&self, circuit_id: &str, service_id: &str) -> Result<bool, InternalError> {
        let path = self
            .lmdb_db_factory
            .compute_path(circuit_id, service_id)
            .map_err(|e| InternalError::from_source(Box::new(e)))?
            .with_extension("lmdb");

        Ok(path.is_file())
    }
}
