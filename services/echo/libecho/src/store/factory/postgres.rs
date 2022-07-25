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

use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};

use crate::store::{
    diesel::{DieselConnectionEchoStore, DieselEchoStore},
    factory::{EchoStoreFactory, PooledEchoStoreFactory},
    EchoStore,
};

pub struct PgEchoStoreFactory;

impl EchoStoreFactory<PgConnection> for PgEchoStoreFactory {
    fn new_store<'a>(&'a self, conn: &'a PgConnection) -> Box<dyn EchoStore + 'a> {
        Box::new(DieselConnectionEchoStore::new(conn))
    }
}

#[derive(Clone)]
pub struct PooledPgEchoStoreFactory {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl PooledPgEchoStoreFactory {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pool }
    }
}

impl PooledEchoStoreFactory for PooledPgEchoStoreFactory {
    fn new_store(&self) -> Box<dyn EchoStore + Send> {
        Box::new(DieselEchoStore::new(self.pool.clone()))
    }

    fn clone_box(&self) -> Box<dyn PooledEchoStoreFactory> {
        Box::new(self.clone())
    }
}
