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
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};

use crate::store::scabbard_store::{
    diesel::{DieselConnectionScabbardStore, DieselScabbardStore},
    factory::{PooledScabbardStoreFactory, ScabbardStoreFactory},
    ScabbardStore,
};

#[derive(Clone)]
pub struct PgScabbardStoreFactory;

impl ScabbardStoreFactory<PgConnection> for PgScabbardStoreFactory {
    fn new_store<'a>(&'a self, conn: &'a PgConnection) -> Box<dyn ScabbardStore + 'a> {
        Box::new(DieselConnectionScabbardStore::new(conn))
    }
}

#[derive(Clone)]
pub struct PooledPgScabbardStoreFactory {
    pool: Arc<RwLock<Pool<ConnectionManager<PgConnection>>>>,
}

impl PooledPgScabbardStoreFactory {
    /// Create a new `PooledPgScabbardStoreFactory`.
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self {
            pool: Arc::new(RwLock::new(pool)),
        }
    }

    /// Create a new `PooledPgScabbardStoreFactory` with shared write-exclusivity.
    pub fn new_with_write_exclusivity(
        pool: Arc<RwLock<Pool<ConnectionManager<PgConnection>>>>,
    ) -> Self {
        Self { pool }
    }
}

impl PooledScabbardStoreFactory for PooledPgScabbardStoreFactory {
    fn new_store(&self) -> Box<dyn ScabbardStore> {
        Box::new(DieselScabbardStore::new_with_write_exclusivity(
            self.pool.clone(),
        ))
    }

    fn clone_box(&self) -> Box<dyn PooledScabbardStoreFactory> {
        Box::new(self.clone())
    }
}
