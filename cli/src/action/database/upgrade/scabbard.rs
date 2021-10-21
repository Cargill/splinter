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

//! Provides scabbard state upgrade functionality

use std::path::Path;

use diesel::r2d2::{ConnectionManager, Pool};
use scabbard::store::{
    diesel::DieselCommitHashStore,
    transact::{factory::LmdbDatabaseFactory, TransactCommitHashStore},
    CommitHashStore,
};
use splinter::{
    admin::store::{diesel::DieselAdminServiceStore, AdminServiceStore},
    error::InternalError,
    node_id::store::{diesel::DieselNodeIdStore, NodeIdStore},
};

use super::{error::UpgradeError, ConnectionUri};

/// Migrate all of the service state's current commit hashes to the [`CommitHashStore`].
pub(super) fn upgrade_scabbard_commit_hash_state(
    state_dir: &Path,
    database_uri: &ConnectionUri,
) -> Result<(), UpgradeError> {
    let lmdb_db_factory = LmdbDatabaseFactory::new_state_db_factory(state_dir, None);
    let upgrade_stores = new_upgrade_stores(database_uri)?;

    let node_id = if let Some(node_id) = upgrade_stores
        .new_node_id_store()
        .get_node_id()
        .map_err(|e| InternalError::from_source(Box::new(e)))?
    {
        node_id
    } else {
        // This node has not even set a node id, so it cannot have any circuits.
        info!("Skipping scabbard commit hash store upgrade, no local node ID found");
        return Ok(());
    };

    let circuits = upgrade_stores
        .new_admin_service_store()
        .list_circuits(&[])
        .map_err(|e| InternalError::from_source(Box::new(e)))?;

    let local_services = circuits
        .into_iter()
        .map(|circuit| {
            circuit
                .roster()
                .iter()
                .filter_map(|svc| {
                    if svc.node_id() == node_id && svc.service_type() == "scabbard" {
                        Some((
                            circuit.circuit_id().to_string(),
                            svc.service_id().to_string(),
                        ))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .flatten();

    for (circuit_id, service_id) in local_services {
        let lmdb_commit_hash_store =
            TransactCommitHashStore::new(lmdb_db_factory.get_database(&circuit_id, &service_id)?);
        let db_commit_hash_store = upgrade_stores.new_commit_hash_store(&circuit_id, &service_id);

        if let Some(current_commit_hash) = lmdb_commit_hash_store
            .get_current_commit_hash()
            .map_err(|e| InternalError::from_source(Box::new(e)))?
        {
            db_commit_hash_store
                .set_current_commit_hash(&current_commit_hash)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
            info!("Upgraded scabbard service {}::{}", circuit_id, service_id);
        } else {
            debug!(
                "No commit hash found for service {}::{}",
                circuit_id, service_id
            );
        }
    }

    Ok(())
}

trait UpgradeStores {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore>;

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore>;

    fn new_commit_hash_store(&self, circuit_id: &str, service_id: &str)
        -> Box<dyn CommitHashStore>;
}

fn new_upgrade_stores(
    database_uri: &ConnectionUri,
) -> Result<Box<dyn UpgradeStores>, UpgradeError> {
    match database_uri {
        #[cfg(feature = "postgres")]
        ConnectionUri::Postgres(url) => {
            let connection_manager = ConnectionManager::<diesel::pg::PgConnection>::new(url);
            let pool = Pool::builder().build(connection_manager).map_err(|err| {
                InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to build connection pool".to_string(),
                )
            })?;
            // Test the connection
            let _conn = pool
                .get()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
            Ok(Box::new(PostgresUpgradeStores(pool)))
        }
        #[cfg(feature = "sqlite")]
        ConnectionUri::Sqlite(conn_str) => {
            if (conn_str != ":memory:") && !std::path::Path::new(&conn_str).exists() {
                return Err(UpgradeError::Internal(InternalError::with_message(
                    format!("Database file '{}' does not exist", conn_str),
                )));
            }
            let connection_manager =
                ConnectionManager::<diesel::sqlite::SqliteConnection>::new(conn_str);
            let mut pool_builder = Pool::builder();
            // A new database is created for each connection to the in-memory SQLite
            // implementation; to ensure that the resulting stores will operate on the same
            // database, only one connection is allowed.
            if conn_str == ":memory:" {
                pool_builder = pool_builder.max_size(1);
            }
            let pool = pool_builder.build(connection_manager).map_err(|err| {
                InternalError::from_source_with_prefix(
                    Box::new(err),
                    "Failed to build connection pool".to_string(),
                )
            })?;
            // Test the connection
            let _conn = pool
                .get()
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
            Ok(Box::new(SqliteUpgradeStores(pool)))
        }
    }
}

#[cfg(feature = "postgres")]
struct PostgresUpgradeStores(Pool<ConnectionManager<diesel::pg::PgConnection>>);

#[cfg(feature = "postgres")]
impl UpgradeStores for PostgresUpgradeStores {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore> {
        Box::new(DieselAdminServiceStore::new(self.0.clone()))
    }

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore> {
        Box::new(DieselNodeIdStore::new(self.0.clone()))
    }

    fn new_commit_hash_store(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore> {
        Box::new(DieselCommitHashStore::new(
            self.0.clone(),
            circuit_id,
            service_id,
        ))
    }
}

#[cfg(feature = "sqlite")]
struct SqliteUpgradeStores(Pool<ConnectionManager<diesel::SqliteConnection>>);

#[cfg(feature = "sqlite")]
impl UpgradeStores for SqliteUpgradeStores {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore> {
        Box::new(DieselAdminServiceStore::new(self.0.clone()))
    }

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore> {
        Box::new(DieselNodeIdStore::new(self.0.clone()))
    }

    fn new_commit_hash_store(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore> {
        Box::new(DieselCommitHashStore::new(
            self.0.clone(),
            circuit_id,
            service_id,
        ))
    }
}
