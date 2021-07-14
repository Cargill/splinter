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

//! Diesel based NodeIdStore

mod operations;
mod schema;
use operations::NodeIdOperator;

use super::error::NodeIdStoreError;
use super::NodeIdStore;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{Insertable, Queryable};
use schema::node_id;

/// Database backed NodeIdStore implementation
pub struct NodeIdDbStore<Conn: diesel::Connection + 'static> {
    pool: Pool<ConnectionManager<Conn>>,
}

impl<C: diesel::Connection> NodeIdDbStore<C> {
    /// Constructs new NodeIdDbStore
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool
    pub fn new(pool: Pool<ConnectionManager<C>>) -> Self {
        Self { pool }
    }
}

#[derive(Queryable, Insertable)]
#[table_name = "node_id"]
struct NodeID {
    id: String,
}

#[cfg(feature = "postgres")]
impl NodeIdStore for NodeIdDbStore<diesel::pg::PgConnection> {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        NodeIdOperator::new(&*self.pool.get()?).get_node_id()
    }
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        NodeIdOperator::new(&*self.pool.get()?).set_node_id(new_id)
    }
}
#[cfg(feature = "sqlite")]
impl NodeIdStore for NodeIdDbStore<diesel::sqlite::SqliteConnection> {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        NodeIdOperator::new(&*self.pool.get()?).get_node_id()
    }
    fn set_node_id(&self, new_id: String) -> Result<(), NodeIdStoreError> {
        NodeIdOperator::new(&*self.pool.get()?).set_node_id(new_id)
    }
}
