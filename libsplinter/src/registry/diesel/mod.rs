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

//! A database-backed registry, powered by [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselRegistry`], which provides an implementation of the
//! [`RwRegistry`] trait.
//!
//! [`DieselRegistry`]: ../struct.DieselRegistry.html
//! [`RwRegistry`]: ../trait.RwRegistry.html

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    MetadataPredicate, Node, NodeIter, RegistryError, RegistryReader, RegistryWriter, RwRegistry,
};

use operations::count_nodes::RegistryCountNodesOperation as _;
use operations::delete_node::RegistryDeleteNodeOperation as _;
use operations::fetch_node::RegistryFetchNodeOperation as _;
use operations::has_node::RegistryHasNodeOperation as _;
use operations::insert_node::RegistryInsertNodeOperation as _;
use operations::list_nodes::RegistryListNodesOperation as _;
use operations::RegistryOperations;

/// A database-backed registry, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselRegistry<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselRegistry<C> {
    /// Creates a new `DieselRegistry`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselRegistry { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselRegistry<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselRegistry<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

impl<C> RegistryReader for DieselRegistry<C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?)
            .list_nodes(predicates)
            .map(|nodes| Box::new(nodes.into_iter()) as NodeIter<'a>)
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).count_nodes(predicates)
    }

    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).fetch_node(identity)
    }

    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).has_node(identity)
    }
}

#[cfg(feature = "postgres")]
impl RegistryWriter for DieselRegistry<diesel::pg::PgConnection> {
    fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).delete_node(identity)
    }
}

#[cfg(feature = "sqlite")]
impl RegistryWriter for DieselRegistry<diesel::sqlite::SqliteConnection> {
    fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        RegistryOperations::new(&*self.connection_pool.get()?).delete_node(identity)
    }
}

#[cfg(feature = "postgres")]
impl RwRegistry for DieselRegistry<diesel::pg::PgConnection>
where
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, diesel::pg::Pg>,
{
    fn clone_box(&self) -> Box<dyn RwRegistry> {
        Box::new(self.clone())
    }

    fn clone_box_as_reader(&self) -> Box<dyn RegistryReader> {
        Box::new(self.clone())
    }

    fn clone_box_as_writer(&self) -> Box<dyn RegistryWriter> {
        Box::new(self.clone())
    }
}

#[cfg(feature = "sqlite")]
impl RwRegistry for DieselRegistry<diesel::sqlite::SqliteConnection>
where
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, diesel::sqlite::Sqlite>,
{
    fn clone_box(&self) -> Box<dyn RwRegistry> {
        Box::new(self.clone())
    }

    fn clone_box_as_reader(&self) -> Box<dyn RegistryReader> {
        Box::new(self.clone())
    }

    fn clone_box_as_writer(&self) -> Box<dyn RegistryWriter> {
        Box::new(self.clone())
    }
}
