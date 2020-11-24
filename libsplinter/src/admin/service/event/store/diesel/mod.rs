// Copyright 2018-2020 Cargill Incorporated
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

//! Database backend support for the `AdminServiceEventStore`, powered by
//! [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselAdminServiceStore`].
//!
//! [`DieselAdminServiceEventStore`]: struct.DieselAdminServiceEventStore.html
//! [`AdminServiceEventStore`]: ../trait.AdminServiceEventStore.html

mod models;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

/// A database-backed AdminServiceEventStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselAdminServiceEventStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselAdminServiceEventStore<C> {
    /// Creates a new `DieselAdminServiceEventStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn _new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselAdminServiceEventStore { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselAdminServiceEventStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselAdminServiceEventStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }

    #[test]
    /// Test that the sqlite migrations can be run successfully
    fn test_admin_service_event_store_sqlite_migrations() {
        create_connection_pool_and_migrate();
    }
}
