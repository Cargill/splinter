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

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use sawtooth::receipt::store::{diesel::DieselReceiptStore, ReceiptStore};
use scabbard::store::transact::factory::LmdbDatabaseFactory;
use scabbard::store::{
    diesel::{DieselCommitHashStore, DieselInTransactionCommitHashStore},
    CommitHashStore,
};
use splinter::{
    admin::store::{diesel::DieselAdminServiceStore, AdminServiceStore},
    error::InternalError,
    node_id::store::{diesel::DieselNodeIdStore, NodeIdStore},
};
use transact::state::merkle::sql::{backend, SqlMerkleStateBuilder};

#[cfg(any(feature = "postgres", feature = "sqlite"))]
use super::state::{DieselInTransactionStateTreeStore, DieselStateTreeStore};
use super::state::{LazyLmdbMerkleState, LmdbStateTreeStore, MerkleState, StateTreeStore};
use super::ConnectionUri;

pub trait UpgradeStores {
    fn new_admin_service_store<'a>(&'a self) -> Box<dyn AdminServiceStore + 'a>;

    fn new_node_id_store<'a>(&'a self) -> Box<dyn NodeIdStore + 'a>;

    fn new_commit_hash_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore + 'a>;

    fn new_receipt_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn ReceiptStore + 'a>;

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError>;

    fn new_state_tree_store<'a>(&'a self) -> Box<dyn StateTreeStore + 'a>;
}

type InTransactionHandle<'a> =
    Box<dyn FnOnce(&dyn UpgradeStores) -> Result<(), InternalError> + 'a>;

pub trait TransactionalUpgradeStores: UpgradeStores {
    fn in_transaction(&self, f: InTransactionHandle<'_>) -> Result<(), InternalError>;

    fn as_upgrade_stores(&self) -> &dyn UpgradeStores;
}

pub fn new_upgrade_stores(
    database_uri: &ConnectionUri,
) -> Result<Box<dyn TransactionalUpgradeStores>, InternalError> {
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

    fn new_state_tree_store(&self) -> Box<dyn StateTreeStore> {
        Box::new(DieselStateTreeStore::new(self.0.clone()))
    }
}

#[cfg(feature = "postgres")]
impl TransactionalUpgradeStores for PostgresUpgradeStores {
    fn in_transaction<'a>(
        &self,
        f: Box<dyn FnOnce(&dyn UpgradeStores) -> Result<(), InternalError> + 'a>,
    ) -> Result<(), InternalError> {
        let conn = self
            .0
            .get()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        conn.transaction::<_, _, _>(|| f(&InTransactionPostgresUpgradeStores(&*conn)))
    }

    fn as_upgrade_stores(&self) -> &dyn UpgradeStores {
        self
    }
}

#[cfg(feature = "postgres")]
struct InTransactionPostgresUpgradeStores<'a>(&'a diesel::pg::PgConnection);

#[cfg(feature = "postgres")]
impl<'a> UpgradeStores for InTransactionPostgresUpgradeStores<'a> {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore> {
        unimplemented!("AdminServiceStore does not yet in-transaction behaviour")
    }

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore> {
        unimplemented!("NodeIdStore does not yet in-transaction behaviour")
    }

    fn new_commit_hash_store<'b>(
        &'b self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore + 'a> {
        Box::new(DieselInTransactionCommitHashStore::new(
            self.0, circuit_id, service_id,
        ))
    }

    fn new_receipt_store(&self, _circuit_id: &str, _service_id: &str) -> Box<dyn ReceiptStore> {
        unimplemented!("ReceiptStore does not yet in-transaction behaviour")
    }

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError> {
        let backend = backend::InTransactionPostgresBackend::from(self.0);
        let mut builder = SqlMerkleStateBuilder::new()
            .with_backend(backend)
            .with_tree(format!("{}::{}", circuit_id, service_id));

        if create_tree {
            builder = builder.create_tree_if_necessary();
        }

        let state = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(MerkleState::InTransactionPostgres { state })
    }

    fn new_state_tree_store<'b>(&'b self) -> Box<dyn StateTreeStore + 'b> {
        Box::new(DieselInTransactionStateTreeStore::new(self.0))
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

    fn new_state_tree_store(&self) -> Box<dyn StateTreeStore> {
        Box::new(DieselStateTreeStore::new(self.0.clone()))
    }
}

impl TransactionalUpgradeStores for SqliteUpgradeStores {
    fn in_transaction<'a>(
        &self,
        f: Box<dyn FnOnce(&dyn UpgradeStores) -> Result<(), InternalError> + 'a>,
    ) -> Result<(), InternalError> {
        let conn = self
            .0
            .get()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        conn.transaction::<_, _, _>(|| f(&InTransactionSqliteUpgradeStores(&*conn)))
    }

    fn as_upgrade_stores(&self) -> &dyn UpgradeStores {
        self
    }
}

#[cfg(feature = "sqlite")]
struct InTransactionSqliteUpgradeStores<'a>(&'a diesel::SqliteConnection);

#[cfg(feature = "sqlite")]
impl<'a> UpgradeStores for InTransactionSqliteUpgradeStores<'a> {
    fn new_admin_service_store(&self) -> Box<dyn AdminServiceStore> {
        unimplemented!("AdminServiceStore does not yet in-transaction behaviour")
    }

    fn new_node_id_store(&self) -> Box<dyn NodeIdStore> {
        unimplemented!("NodeIdStore does not yet in-transaction behaviour")
    }

    fn new_commit_hash_store<'b>(
        &'b self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore + 'a> {
        Box::new(DieselInTransactionCommitHashStore::new(
            self.0, circuit_id, service_id,
        ))
    }

    fn new_receipt_store(&self, _circuit_id: &str, _service_id: &str) -> Box<dyn ReceiptStore> {
        unimplemented!("ReceiptStore does not yet in-transaction behaviour")
    }

    fn get_merkle_state(
        &self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState, InternalError> {
        let backend = backend::InTransactionSqliteBackend::from(self.0);
        let mut builder = SqlMerkleStateBuilder::new()
            .with_backend(backend)
            .with_tree(format!("{}::{}", circuit_id, service_id));

        if create_tree {
            builder = builder.create_tree_if_necessary();
        }

        let state = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(MerkleState::InTransactionSqlite { state })
    }

    fn new_state_tree_store<'b>(&'b self) -> Box<dyn StateTreeStore + 'b> {
        Box::new(DieselInTransactionStateTreeStore::new(self.0))
    }
}

pub struct UpgradeStoresWithLmdb {
    upgrade_stores: Box<dyn TransactionalUpgradeStores>,
    lmdb_db_factory: LmdbDatabaseFactory,
}

impl UpgradeStoresWithLmdb {
    pub fn new(
        upgrade_stores: Box<dyn TransactionalUpgradeStores>,
        lmdb_db_factory: LmdbDatabaseFactory,
    ) -> Self {
        Self {
            upgrade_stores,
            lmdb_db_factory,
        }
    }
}

impl UpgradeStores for UpgradeStoresWithLmdb {
    fn new_admin_service_store<'a>(&'a self) -> Box<dyn AdminServiceStore + 'a> {
        self.upgrade_stores.new_admin_service_store()
    }

    fn new_node_id_store<'a>(&'a self) -> Box<dyn NodeIdStore + 'a> {
        self.upgrade_stores.new_node_id_store()
    }

    fn new_commit_hash_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore + 'a> {
        self.upgrade_stores
            .new_commit_hash_store(circuit_id, service_id)
    }

    fn new_receipt_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn ReceiptStore + 'a> {
        self.upgrade_stores
            .new_receipt_store(circuit_id, service_id)
    }

    fn get_merkle_state<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<MerkleState<'a>, InternalError> {
        create_lmdb_merkle_state(&self.lmdb_db_factory, circuit_id, service_id, create_tree)
    }

    fn new_state_tree_store<'a>(&'a self) -> Box<dyn StateTreeStore + 'a> {
        Box::new(LmdbStateTreeStore::new(self.lmdb_db_factory.clone()))
    }
}

impl TransactionalUpgradeStores for UpgradeStoresWithLmdb {
    fn in_transaction<'a>(
        &self,
        f: Box<dyn FnOnce(&dyn UpgradeStores) -> Result<(), InternalError> + 'a>,
    ) -> Result<(), InternalError> {
        let lmdb_db_factory = self.lmdb_db_factory.clone();
        self.upgrade_stores
            .in_transaction(Box::new(move |txn_stores| {
                f(&InTransactionUpgradeStoresWithLmdb {
                    upgrade_stores: txn_stores,
                    lmdb_db_factory,
                })
            }))
    }

    fn as_upgrade_stores(&self) -> &dyn UpgradeStores {
        self
    }
}

struct InTransactionUpgradeStoresWithLmdb<'a> {
    upgrade_stores: &'a dyn UpgradeStores,
    lmdb_db_factory: LmdbDatabaseFactory,
}

impl<'u> UpgradeStores for InTransactionUpgradeStoresWithLmdb<'u> {
    fn new_admin_service_store<'a>(&'a self) -> Box<dyn AdminServiceStore + 'a> {
        self.upgrade_stores.new_admin_service_store()
    }

    fn new_node_id_store<'a>(&'a self) -> Box<dyn NodeIdStore + 'a> {
        self.upgrade_stores.new_node_id_store()
    }

    fn new_commit_hash_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn CommitHashStore + 'a> {
        self.upgrade_stores
            .new_commit_hash_store(circuit_id, service_id)
    }

    fn new_receipt_store<'a>(
        &'a self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn ReceiptStore + 'a> {
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

    fn new_state_tree_store(&self) -> Box<dyn StateTreeStore> {
        Box::new(LmdbStateTreeStore::new(self.lmdb_db_factory.clone()))
    }
}

fn create_lmdb_merkle_state<'a>(
    lmdb_db_factory: &LmdbDatabaseFactory,
    circuit_id: &str,
    service_id: &str,
    create_tree: bool,
) -> Result<MerkleState<'a>, InternalError> {
    Ok(MerkleState::Lmdb {
        state: LazyLmdbMerkleState::new(
            lmdb_db_factory.clone(),
            circuit_id,
            service_id,
            create_tree,
        )?,
    })
}
