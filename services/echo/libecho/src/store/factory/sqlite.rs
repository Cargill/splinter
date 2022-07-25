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
use std::sync::{Arc, RwLock};

use diesel::{
    r2d2::{ConnectionManager, Pool},
    sqlite::SqliteConnection,
};

use crate::store::{
    diesel::{DieselConnectionEchoStore, DieselEchoStore},
    factory::{EchoStoreFactory, PooledEchoStoreFactory},
    EchoStore,
};

pub struct SqliteEchoStoreFactory;

impl EchoStoreFactory<SqliteConnection> for SqliteEchoStoreFactory {
    fn new_store<'a>(&'a self, conn: &'a SqliteConnection) -> Box<dyn EchoStore + 'a> {
        Box::new(DieselConnectionEchoStore::new(conn))
    }
}

#[derive(Clone)]
pub struct PooledSqliteEchoStoreFactory {
    pool: Arc<RwLock<Pool<ConnectionManager<SqliteConnection>>>>,
}

impl PooledSqliteEchoStoreFactory {
    /// Create a new `SqliteStoreFactory`.
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self {
            pool: Arc::new(RwLock::new(pool)),
        }
    }

    /// Create a new `SqliteStoreFactory` with shared write-exclusivity.
    pub fn new_with_write_exclusivity(
        pool: Arc<RwLock<Pool<ConnectionManager<SqliteConnection>>>>,
    ) -> Self {
        Self { pool }
    }
}

impl PooledEchoStoreFactory for PooledSqliteEchoStoreFactory {
    fn new_store(&self) -> Box<dyn EchoStore + Send> {
        Box::new(DieselEchoStore::new_with_write_exclusivity(
            self.pool.clone(),
        ))
    }

    fn clone_box(&self) -> Box<dyn PooledEchoStoreFactory> {
        Box::new(self.clone())
    }
}
