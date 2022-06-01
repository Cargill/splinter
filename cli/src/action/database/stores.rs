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

use diesel::r2d2::{ConnectionManager, Pool};
use sawtooth::receipt::store::{diesel::DieselReceiptStore, ReceiptStore};
use scabbard::store::transact::factory::LmdbDatabaseFactory;
use scabbard::store::{diesel::DieselCommitHashStore, CommitHashStore};
use splinter::{
    admin::store::{diesel::DieselAdminServiceStore, AdminServiceStore},
    error::InternalError,
    node_id::store::{diesel::DieselNodeIdStore, NodeIdStore},
};
use transact::state::merkle::{
    kv::{MerkleRadixTree, MerkleState as TransactMerkleState},
    sql::{backend, SqlMerkleStateBuilder},
};

use super::state::MerkleState;
use super::ConnectionUri;

pub trait UpgradeStores {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore>;

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore>;

    fn new_commit_hash_store(&self, circuit_id: &str, service_id: &str)
        -> Box<dyn CommitHashStore>;

    fn new_receipt_store(&self, circuit_id: &str, service_id: &str) -> Box<dyn ReceiptStore>;

    #[cfg(feature = "sqlite")]
    fn get_sqlite_pool(&self) -> Pool<ConnectionManager<diesel::SqliteConnection>>;

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError>;

    #[cfg(feature = "postgres")]
    fn get_postgres_pool(&self) -> Pool<ConnectionManager<diesel::PgConnection>>;
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

    // should never be used if using postgres
    #[cfg(feature = "sqlite")]
    fn get_sqlite_pool(&self) -> Pool<ConnectionManager<diesel::SqliteConnection>> {
        unimplemented!()
    }

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError> {
        let backend = backend::PostgresBackend::from(self.0.clone());
        let mut builder = SqlMerkleStateBuilder::new()
            .with_backend(backend)
            .with_tree(format!("{}::{}", circuit_id, service_id));

        if create_tree {
            builder = builder.create_tree_if_necessary();
        }

        let state = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(MerkleState::Postgres { state })
    }

    fn get_postgres_pool(&self) -> Pool<ConnectionManager<diesel::PgConnection>> {
        self.0.clone()
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

    fn get_sqlite_pool(&self) -> Pool<ConnectionManager<diesel::SqliteConnection>> {
        self.0.clone()
    }

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError> {
        let backend = backend::SqliteBackend::from(self.0.clone());
        let mut builder = SqlMerkleStateBuilder::new()
            .with_backend(backend)
            .with_tree(format!("{}::{}", circuit_id, service_id));

        if create_tree {
            builder = builder.create_tree_if_necessary();
        }

        let state = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(MerkleState::Sqlite { state })
    }

    // should never be used if using sqlite
    #[cfg(feature = "postgres")]
    fn get_postgres_pool(&self) -> Pool<ConnectionManager<diesel::PgConnection>> {
        unimplemented!()
    }
}

pub struct UpgradeStoresWithLmdb {
    upgrade_stores: Box<dyn UpgradeStores>,
    lmdb_db_factory: LmdbDatabaseFactory,
}

impl UpgradeStoresWithLmdb {
    pub fn new(
        upgrade_stores: Box<dyn UpgradeStores>,
        lmdb_db_factory: LmdbDatabaseFactory,
    ) -> Self {
        Self {
            upgrade_stores,
            lmdb_db_factory,
        }
    }
}

impl UpgradeStores for UpgradeStoresWithLmdb {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore> {
        self.upgrade_stores.new_admin_service_store()
    }

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore> {
        self.upgrade_stores.new_node_id_store()
    }

    fn new_commit_hash_store(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore> {
        self.upgrade_stores
            .new_commit_hash_store(circuit_id, service_id)
    }

    fn new_receipt_store(&self, circuit_id: &str, service_id: &str) -> Box<dyn ReceiptStore> {
        self.upgrade_stores
            .new_receipt_store(circuit_id, service_id)
    }

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError> {
        create_lmdb_merkle_state(&self.lmdb_db_factory, circuit_id, service_id, create_tree)
    }

    fn get_sqlite_pool(&self) -> Pool<ConnectionManager<diesel::SqliteConnection>> {
        self.upgrade_stores.get_sqlite_pool()
    }

    #[cfg(feature = "postgres")]
    fn get_postgres_pool(&self) -> Pool<ConnectionManager<diesel::PgConnection>> {
        self.upgrade_stores.get_postgres_pool()
    }
}

fn create_lmdb_merkle_state(
    lmdb_db_factory: &LmdbDatabaseFactory,
    circuit_id: &str,
    service_id: &str,
    create_tree: bool,
) -> Result<MerkleState, InternalError> {
    if !create_tree {
        let path = lmdb_db_factory
            .compute_path(circuit_id, service_id)
            .map_err(|e| InternalError::with_message(format!("{}", e)))?
            .with_extension("lmdb");

        if !path.is_file() {
            return Err(InternalError::with_message(format!(
                "LMDB file for service {}::{} ({:?}) does not exist",
                circuit_id, service_id, path
            )));
        }
    }
    let state = lmdb_db_factory
        .get_database(circuit_id, service_id)
        .map_err(|e| InternalError::with_message(format!("{}", e)))?;
    let merkle_root = MerkleRadixTree::new(Box::new(state.clone()), None)
        .map_err(|e| InternalError::with_message(format!("{}", e)))?
        .get_merkle_root();
    Ok(MerkleState::Lmdb {
        state: TransactMerkleState::new(Box::new(state)),
        merkle_root,
        tree_id: (circuit_id.to_string(), service_id.to_string()),
    })
}
