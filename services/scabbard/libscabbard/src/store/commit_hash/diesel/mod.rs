// Copyright 2021 Cargill Incorporated
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

mod models;
mod operations;
mod schema;

use std::sync::{Arc, RwLock};

use diesel::r2d2::{ConnectionManager, Pool};

use crate::store::pool::ConnectionPool;

use super::{CommitHashStore, CommitHashStoreError};

use operations::get_current_commit_hash::CommitHashStoreGetCurrentCommitHashOperation as _;
use operations::set_current_commit_hash::CommitHashStoreSetCurrentCommitHashOperation as _;
use operations::CommitHashStoreOperations;

/// Database backed [CommitHashStore] implementation.
#[derive(Clone)]
pub struct DieselCommitHashStore<Conn: diesel::Connection + 'static> {
    pool: ConnectionPool<Conn>,
    circuit_id: Arc<str>,
    service_id: Arc<str>,
}

impl<C: diesel::Connection> DieselCommitHashStore<C> {
    /// Constructs new DieselCommitHashStore.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    /// * `circuit_id` - The circuit associated with the store
    /// * `service_id` - The service associated with the store
    pub fn new(pool: Pool<ConnectionManager<C>>, circuit_id: &str, service_id: &str) -> Self {
        Self {
            pool: ConnectionPool::Normal(pool),
            circuit_id: circuit_id.into(),
            service_id: service_id.into(),
        }
    }

    /// Create a new `DieselCommitHashStore` with write exclusivity enabled.
    ///
    /// Write exclusivity is enforced by providing a connection pool that is wrapped in a
    /// [`RwLock`]. This ensures that there may be only one writer, but many readers.
    ///
    /// # Arguments
    ///
    /// * `pool`: read-write lock-guarded connection pool for the database
    /// * `circuit_id` - The circuit associated with the store
    /// * `service_id` - The service associated with the store
    pub fn new_with_write_exclusivity(
        pool: Arc<RwLock<Pool<ConnectionManager<C>>>>,
        circuit_id: &str,
        service_id: &str,
    ) -> Self {
        Self {
            pool: ConnectionPool::WriteExclusive(pool),
            circuit_id: circuit_id.into(),
            service_id: service_id.into(),
        }
    }
}

#[cfg(feature = "postgres")]
impl CommitHashStore for DieselCommitHashStore<diesel::pg::PgConnection> {
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError> {
        self.pool.execute_read(|conn| {
            CommitHashStoreOperations::new(conn)
                .get_current_commit_hash(&*self.circuit_id, &*self.service_id)
        })
    }

    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError> {
        self.pool.execute_write(|conn| {
            CommitHashStoreOperations::new(conn).set_current_commit_hash(
                &*self.circuit_id,
                &*self.service_id,
                commit_hash,
            )
        })
    }
}

#[cfg(feature = "sqlite")]
impl CommitHashStore for DieselCommitHashStore<diesel::sqlite::SqliteConnection> {
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError> {
        self.pool.execute_read(|conn| {
            CommitHashStoreOperations::new(conn)
                .get_current_commit_hash(&*self.circuit_id, &*self.service_id)
        })
    }

    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError> {
        self.pool.execute_write(|conn| {
            CommitHashStoreOperations::new(conn).set_current_commit_hash(
                &*self.circuit_id,
                &*self.service_id,
                commit_hash,
            )
        })
    }
}

/// Database backed [CommitHashStore] implementation.
pub struct DieselInTransactionCommitHashStore<'a, C: diesel::Connection> {
    conn: &'a C,
    circuit_id: Arc<str>,
    service_id: Arc<str>,
}

impl<'a, C: diesel::Connection> DieselInTransactionCommitHashStore<'a, C> {
    /// Constructs new DieselCommitHashStore.
    ///
    /// # Arguments
    ///
    /// * `conn` - The connection reference associated with an ongoing transaction
    /// * `circuit_id` - The circuit associated with the store
    /// * `service_id` - The service associated with the store
    pub fn new(conn: &'a C, circuit_id: &str, service_id: &str) -> Self {
        Self {
            conn,
            circuit_id: circuit_id.into(),
            service_id: service_id.into(),
        }
    }
}

#[cfg(feature = "postgres")]
impl<'a> CommitHashStore for DieselInTransactionCommitHashStore<'a, diesel::pg::PgConnection> {
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError> {
        CommitHashStoreOperations::new(self.conn)
            .get_current_commit_hash(&*self.circuit_id, &*self.service_id)
    }

    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError> {
        CommitHashStoreOperations::new(self.conn).set_current_commit_hash(
            &*self.circuit_id,
            &*self.service_id,
            commit_hash,
        )
    }
}

#[cfg(feature = "sqlite")]
impl<'a> CommitHashStore
    for DieselInTransactionCommitHashStore<'a, diesel::sqlite::SqliteConnection>
{
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError> {
        CommitHashStoreOperations::new(self.conn)
            .get_current_commit_hash(&*self.circuit_id, &*self.service_id)
    }

    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError> {
        CommitHashStoreOperations::new(self.conn).set_current_commit_hash(
            &*self.circuit_id,
            &*self.service_id,
            commit_hash,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    use crate::migrations::run_sqlite_migrations;

    /// Test that a DieselCommitHashStore using a SQLite connection pool can
    /// 1. Set and get a hash on one circuit
    /// 2. Verify that it is isolated to that circuit
    /// 3. The alternate circuit hash can be set and get
    /// 4. Verify that the original is still set
    #[cfg(feature = "sqlite")]
    #[test]
    fn test_sqlite_commit_hash_store() -> Result<(), Box<dyn std::error::Error>> {
        let pool = create_connection_pool_and_migrate()?;
        let commit_log_store_circuit_1 =
            DieselCommitHashStore::new(pool.clone(), "circuit_1", "service");

        assert_eq!(None, commit_log_store_circuit_1.get_current_commit_hash()?);

        commit_log_store_circuit_1.set_current_commit_hash("abcdef0123456789")?;

        assert_eq!(
            Some("abcdef0123456789".to_string()),
            commit_log_store_circuit_1.get_current_commit_hash()?
        );

        // Check that the service on a different circuit has no hash.
        let commit_log_store_circuit_2 = DieselCommitHashStore::new(pool, "circuit_2", "service");
        assert_eq!(None, commit_log_store_circuit_2.get_current_commit_hash()?);

        commit_log_store_circuit_2.set_current_commit_hash("9876543210fedcba")?;

        assert_eq!(
            Some("9876543210fedcba".to_string()),
            commit_log_store_circuit_2.get_current_commit_hash()?
        );

        // Verify that the original is unchanged.
        assert_eq!(
            Some("abcdef0123456789".to_string()),
            commit_log_store_circuit_1.get_current_commit_hash()?
        );

        Ok(())
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    #[cfg(feature = "sqlite")]
    fn create_connection_pool_and_migrate(
    ) -> Result<Pool<ConnectionManager<SqliteConnection>>, Box<dyn std::error::Error>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder().max_size(1).build(connection_manager)?;

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))?;

        Ok(pool)
    }
}
