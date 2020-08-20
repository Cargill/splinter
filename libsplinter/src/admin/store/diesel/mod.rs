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

//! Database backend support for the AdminServiceStore, powered by
//! [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselAdminServiceStore`], which provides an implementation of the
//! [`AdminServiceStore`] trait.
//!
//! [`DieselAdminServiceStore`]: struct.DieselAdminServiceStore.html
//! [`AdminServiceStore`]: ../trait.AdminServiceStore.html

pub mod migrations;
mod models;
pub mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

/// A database-backed AdminServiceStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselAdminServiceStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselAdminServiceStore<C> {
    /// Creates a new `DieselAdminServiceStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselAdminServiceStore { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselAdminServiceStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselAdminServiceStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}
