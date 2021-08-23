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

//! Diesel based NodeIdStore.

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::error::NodeIdStoreError;
use super::NodeIdStore;

use models::NodeID;
use operations::{
    get_node_id::NodeIdGetOperation, set_node_id::NodeIdSetOperation, NodeIdOperations,
};

/// Database backed [NodeIdStore] implementation.
pub struct DieselNodeIdStore<Conn: diesel::Connection + 'static> {
    pool: Pool<ConnectionManager<Conn>>,
}

impl<C: diesel::Connection> DieselNodeIdStore<C> {
    /// Constructs new DieselNodeIdStore.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    pub fn new(pool: Pool<ConnectionManager<C>>) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
impl NodeIdStore for DieselNodeIdStore<diesel::pg::PgConnection> {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        NodeIdOperations::new(&*self.pool.get()?).get_node_id()
    }
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        NodeIdOperations::new(&*self.pool.get()?).set_node_id(new_id)
    }
}
#[cfg(feature = "sqlite")]
impl NodeIdStore for DieselNodeIdStore<diesel::sqlite::SqliteConnection> {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        NodeIdOperations::new(&*self.pool.get()?).get_node_id()
    }
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        NodeIdOperations::new(&*self.pool.get()?).set_node_id(new_id)
    }
}
