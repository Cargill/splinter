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

use std::sync::{Arc, RwLock};

use diesel::r2d2::{ConnectionManager, Pool};

use crate::store::pool::ConnectionPool;

use super::{
    MetadataPredicate, Node, NodeIter, RegistryError, RegistryReader, RegistryWriter, RwRegistry,
};

use operations::add_node::RegistryAddNodeOperation as _;
use operations::count_nodes::RegistryCountNodesOperation as _;
use operations::delete_node::RegistryDeleteNodeOperation as _;
use operations::get_node::RegistryFetchNodeOperation as _;
use operations::has_node::RegistryHasNodeOperation as _;
use operations::list_nodes::RegistryListNodesOperation as _;
use operations::update_node::RegistryUpdateNodeOperation as _;
use operations::RegistryOperations;

/// A database-backed registry, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselRegistry<C: diesel::Connection + 'static> {
    connection_pool: ConnectionPool<C>,
}

impl<C: diesel::Connection> DieselRegistry<C> {
    /// Creates a new `DieselRegistry`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselRegistry {
            connection_pool: connection_pool.into(),
        }
    }

    /// Create a new `DieselRegistry` with write exclusivity enabled.
    ///
    /// Write exclusivity is enforced by providing a connection pool that is wrapped in a
    /// [`RwLock`]. This ensures that there may be only one writer, but many readers.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: read-write lock-guarded connection pool for the database
    pub fn new_with_write_exclusivity(
        connection_pool: Arc<RwLock<Pool<ConnectionManager<C>>>>,
    ) -> Self {
        Self {
            connection_pool: connection_pool.into(),
        }
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
        self.connection_pool.execute_read(|conn| {
            RegistryOperations::new(conn)
                .list_nodes(predicates)
                .map(|nodes| Box::new(nodes.into_iter()) as NodeIter<'a>)
        })
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        self.connection_pool
            .execute_read(|conn| RegistryOperations::new(conn).count_nodes(predicates))
    }

    fn get_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        self.connection_pool
            .execute_read(|conn| RegistryOperations::new(conn).get_node(identity))
    }

    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        self.connection_pool
            .execute_read(|conn| RegistryOperations::new(conn).has_node(identity))
    }
}

#[cfg(feature = "postgres")]
impl RegistryWriter for DieselRegistry<diesel::pg::PgConnection> {
    fn add_node(&self, node: Node) -> Result<(), RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).add_node(node))
    }

    fn update_node(&self, node: Node) -> Result<(), RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).update_node(node))
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).delete_node(identity))
    }
}

#[cfg(feature = "sqlite")]
impl RegistryWriter for DieselRegistry<diesel::sqlite::SqliteConnection> {
    fn add_node(&self, node: Node) -> Result<(), RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).add_node(node))
    }

    fn update_node(&self, node: Node) -> Result<(), RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).update_node(node))
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        self.connection_pool
            .execute_write(|conn| RegistryOperations::new(conn).delete_node(identity))
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

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    ///  Test that a new node can be add to the registry and fetched
    ///
    /// 1. Setup sqlite database
    /// 2. Add node 1
    /// 3. Validate that the node can be fetched correctly from state
    #[test]
    fn test_add_node() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        #[allow(deprecated)]
        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        let node = registry
            .get_node(&get_node_1().identity)
            .expect("Failed to fetch node")
            .expect("Node not found");

        assert_eq!(node, get_node_1());
    }

    /// Verifies that `update_node` properly updates a node
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1
    /// 3. Verify updating node 1 works (with no updates)
    /// 4. Insert node 2
    /// 5. Verify updating node 2 with one of node 1 endpoints fails
    #[test]
    fn test_update_node() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");

        let mut node = registry
            .get_node(&get_node_1().identity)
            .expect("Failed to fetch node")
            .expect("Node not found");

        assert_eq!(node, get_node_1());

        node.display_name = "Changed Name".to_string();

        registry
            .update_node(node.clone())
            .expect("Unable to update node 1");

        let updated_node = registry
            .get_node(&get_node_1().identity)
            .expect("Failed to fetch node")
            .expect("Node not found");

        assert_eq!(updated_node, node);

        registry
            .add_node(get_node_2())
            .expect("Unable to insert node 2");

        let mut node = get_node_2();
        // add node 1 endpoint
        node.endpoints.push("tcps://12.0.0.123:8431".to_string());

        // This should fail becasue the added endpoint already belongs to node 1
        assert!(registry.update_node(node).is_err());
    }

    ///  Test that a new node can be inserted into the registry and fetched
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Try to get that does not exist
    #[test]
    fn test_get_node_not_found() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        assert_eq!(
            registry
                .get_node("DoesNotExist")
                .expect("Failed to fetch node"),
            None
        )
    }

    /// Verifies that `has_node` properly determines if a node exists in the registry.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1
    /// 3. Validate that the registry has node 1 but not node 2
    #[test]
    fn test_has_node() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");

        assert!(registry
            .has_node(&get_node_1().identity)
            .expect("Failed to check if node1 exists"));
        assert!(!registry
            .has_node(&get_node_2().identity)
            .expect("Failed to check if node2 exists"));
    }

    /// Verifies that list_nodes returns a list of nodes.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Validate that the registry returns both nodes in the list
    #[test]
    fn test_list_nodes_ok() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0], get_node_1());
        assert_eq!(nodes[1], get_node_2());
    }

    /// Verifies that list_nodes returns an empty list when there are no nodes in the registry.
    ///
    /// 1. Setup sqlite database
    /// 2. Validate that the registry returns an empty list
    #[test]
    fn test_list_nodes_empty_ok() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 0);
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Validate that the registry returns only node 2 when filtered by company
    #[test]
    fn test_list_nodes_filter_metadata_ok() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Eq(
            "company".into(),
            get_node_2().metadata.get("company").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], get_node_2());
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by multiple
    /// metadata fields.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2 and 3
    /// 3. Validate that the registry returns only node 3 when filtered by company and admin
    #[test]
    fn test_list_nodes_filter_metadata_mutliple() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        let filter = vec![
            MetadataPredicate::Eq(
                "company".to_string(),
                get_node_3().metadata.get("company").unwrap().to_string(),
            ),
            MetadataPredicate::Eq(
                "admin".to_string(),
                get_node_3().metadata.get("admin").unwrap().to_string(),
            ),
        ];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], get_node_3());
    }

    /// Verifies that list_nodes returns an empty list when no nodes fits the filtering criteria.
    ///
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, and
    /// 3. Validate that the registry returns an empty list
    #[test]
    fn test_list_nodes_filter_empty_ok() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Eq(
            "admin".to_string(),
            get_node_3().metadata.get("admin").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 0);
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Validate that the registry returns only node 1 when filtered by company
    #[test]
    fn test_list_nodes_filter_metadata_not_equal() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Ne(
            "company".into(),
            get_node_2().metadata.get("company").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], get_node_1());
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Validate that the registry returns only node 2 when filtered by gt admin Bob
    #[test]
    fn test_list_nodes_filter_metadata_gt() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Gt(
            "admin".into(),
            get_node_1().metadata.get("admin").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], get_node_2());
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2, and 3
    /// 3. Validate that the registry returns node 2 and 3 when filtered by ge admin Carol
    #[test]
    fn test_list_nodes_filter_metadata_ge() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Ge(
            "admin".into(),
            get_node_2().metadata.get("admin").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes, [get_node_2(), get_node_3()]);
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1 and 2
    /// 3. Validate that the registry returns only node 1 when filtered by lt admin Carol
    #[test]
    fn test_list_nodes_filter_metadata_lt() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Lt(
            "admin".into(),
            get_node_2().metadata.get("admin").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0], get_node_1());
    }

    /// Verifies that list_nodes returns the correct items when it is filtered by metadata.
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2, and 3
    /// 3. Validate that the registry returns node 1 and 2 when filtered by le admin Carol
    #[test]
    fn test_list_nodes_filter_metadata_le() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Le(
            "admin".into(),
            get_node_2().metadata.get("admin").unwrap().to_string(),
        )];

        let nodes = registry
            .list_nodes(&filter)
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes, [get_node_1(), get_node_2()]);
    }

    /// Verifies that delete_nodes removes the required node
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2, and 3
    /// 3. Delete node 2
    /// 4. Verify that only node 1 and 3 are returned from list
    #[test]
    fn test_delete_node() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        registry
            .delete_node("Node-456")
            .expect("Unable to delete node");

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes, [get_node_1(), get_node_3()]);
    }

    /// Verifies that count_nodes returns the correct number of nodes
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2, and 3
    /// 4. Verify that the registry count_nodes returns 3
    #[test]
    fn test_count_node() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        let count = registry.count_nodes(&[]).expect("Failed to retrieve nodes");

        assert_eq!(count, 3);
    }

    /// Verifies that count_nodes returns the correct number of nodes when filtered with metadata
    ///
    /// 1. Setup sqlite database
    /// 2. Insert node 1, 2, and 3
    /// 4. Verify that the registry count_nodes returns 2 when filtered by company Cargill
    #[test]
    fn test_count_node_metadata() {
        let pool = create_connection_pool_and_migrate();
        let registry = DieselRegistry::new(pool);

        registry
            .add_node(get_node_1())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_2())
            .expect("Unable to insert node");
        registry
            .add_node(get_node_3())
            .expect("Unable to insert node");

        let filter = vec![MetadataPredicate::Eq(
            "company".into(),
            get_node_2().metadata.get("company").unwrap().to_string(),
        )];

        let count = registry
            .count_nodes(&filter)
            .expect("Failed to retrieve nodes");

        assert_eq!(count, 2);
    }

    fn get_node_1() -> Node {
        Node::builder("Node-123")
            .with_endpoint("tcps://12.0.0.123:8431")
            .with_display_name("Bitwise IO - Node 1")
            .with_key("abcd")
            .with_metadata("company", "Bitwise IO")
            .with_metadata("admin", "Bob")
            .build()
            .expect("Failed to build node1")
    }

    fn get_node_2() -> Node {
        Node::builder("Node-456")
            .with_endpoint("tcps://12.0.0.123:8434")
            .with_display_name("Cargill - Node 1")
            .with_key("0123")
            .with_metadata("company", "Cargill")
            .with_metadata("admin", "Carol")
            .build()
            .expect("Failed to build node2")
    }

    fn get_node_3() -> Node {
        Node::builder("Node-789")
            .with_endpoint("tcps://12.0.0.123:8435")
            .with_display_name("Cargill - Node 2")
            .with_key("4567")
            .with_metadata("company", "Cargill")
            .with_metadata("admin", "Charlie")
            .build()
            .expect("Failed to build node3")
    }

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
}
