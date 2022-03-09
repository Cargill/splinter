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

mod error;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;

use std::sync::{Arc, RwLock};

use ::diesel::r2d2::{ConnectionManager, Pool};

use crate::store::pool::ConnectionPool;

/// A `StoreCommandExecutor`, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselStoreCommandExecutor<C: diesel::Connection + 'static> {
    conn: ConnectionPool<C>,
}

impl<C: diesel::Connection> DieselStoreCommandExecutor<C> {
    /// Creates a new `DieselStoreCommandExecutor`.
    ///
    /// # Arguments
    ///
    ///  * `conn`: connection pool for the database
    pub fn new(conn: Pool<ConnectionManager<C>>) -> Self {
        DieselStoreCommandExecutor { conn: conn.into() }
    }

    /// Create a new `DieselStoreCommandExecutor` with write exclusivity enabled.
    ///
    /// Write exclusivity is enforced by providing a connection pool that is wrapped in a
    /// [`RwLock`]. This ensures that there may be only one writer, but many readers.
    ///
    /// # Arguments
    ///
    ///  * `conn`: read-write lock-guarded connection pool for the database
    pub fn new_with_write_exclusivity(conn: Arc<RwLock<Pool<ConnectionManager<C>>>>) -> Self {
        Self { conn: conn.into() }
    }
}
