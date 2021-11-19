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

use diesel::r2d2::{ConnectionManager, Pool};
use sawtooth::receipt::store::{diesel::DieselReceiptStore, ReceiptStore};
use scabbard::store::{diesel::DieselCommitHashStore, CommitHashStore};
use splinter::{
    admin::store::{diesel::DieselAdminServiceStore, AdminServiceStore},
    error::InternalError,
    node_id::store::{diesel::DieselNodeIdStore, NodeIdStore},
};

use super::ConnectionUri;

pub trait UpgradeStores {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore>;

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore>;

    fn new_commit_hash_store(&self, circuit_id: &str, service_id: &str)
        -> Box<dyn CommitHashStore>;

    fn new_receipt_store(&self, circuit_id: &str, service_id: &str) -> Box<dyn ReceiptStore>;
}

pub fn new_upgrade_stores(
    database_uri: &ConnectionUri,
) -> Result<Box<dyn UpgradeStores>, InternalError> {
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
                return Err(InternalError::with_message(format!(
                    "Database file '{}' does not exist",
                    conn_str
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

    fn new_receipt_store(&self, circuit_id: &str, service_id: &str) -> Box<dyn ReceiptStore> {
        Box::new(DieselReceiptStore::new(
            self.0.clone(),
            Some(format!("{}::{}", circuit_id, service_id)),
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

    fn new_receipt_store(&self, circuit_id: &str, service_id: &str) -> Box<dyn ReceiptStore> {
        Box::new(DieselReceiptStore::new(
            self.0.clone(),
            Some(format!("{}::{}", circuit_id, service_id)),
        ))
    }
}
