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

//! Data structures, traits, and implementations for tracking and managing known Splinter entities.
//!
//! The public registry interface is defined primarily by the [`Node`] data structure (along with
//! its builder, [`NodeBuilder`]), and the registry traits: [`RegistryReader`], [`RegistryWriter`],
//! and [`RwRegistry`].
//!
//! [`Node`]: struct.Node.html
//! [`NodeBuilder`]: struct.NodeBuilder.html
//! [`RegistryReader`]: trait.RegistryReader.html
//! [`RegistryWriter`]: trait.RegistryWriter.html
//! [`RwRegistry`]: trait.RwRegistry.html

#[cfg(feature = "registry-database")]
mod diesel;
mod error;
#[cfg(feature = "rest-api")]
mod rest_api;
mod unified;
mod yaml;

use std::collections::HashMap;
use std::iter::ExactSizeIterator;

#[cfg(all(feature = "registry-database", feature = "postgres"))]
pub use self::diesel::migrations::run_postgres_migrations;
#[cfg(all(feature = "registry-database", feature = "sqlite"))]
pub use self::diesel::migrations::run_sqlite_migrations;
#[cfg(feature = "registry-database")]
pub use self::diesel::DieselRegistry;
pub use error::{InvalidNodeError, RegistryError};
pub use unified::UnifiedRegistry;
pub use yaml::LocalYamlRegistry;
#[cfg(feature = "registry-remote")]
pub use yaml::{RemoteYamlRegistry, RemoteYamlShutdownHandle};

/// Native representation of a node in a registry.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Node {
    /// The Splinter identity of the node; must be non-empty and unique in the registry.
    pub identity: String,
    /// The endpoints the node can be reached at; at least one endpoint must be provided, and each
    /// endpoint must be non-empty and unique in the registry.
    pub endpoints: Vec<String>,
    /// A human-readable name for the node; must be non-empty.
    pub display_name: String,
    /// The list of public keys that are permitted to act on behalf of the node; at least one key
    /// must be provided, and each key must be non-empty.
    pub keys: Vec<String>,
    /// A map with node metadata.
    pub metadata: HashMap<String, String>,
}

impl Node {
    /// Creates a new `NodeBuilder` with the node's `identity`.
    pub fn builder<S: Into<String>>(identity: S) -> NodeBuilder {
        NodeBuilder::new(identity)
    }

    /// Returns `true` if the given key is listed for the node; returns `false` otherwise.
    pub fn has_key(&self, key: &str) -> bool {
        self.keys.iter().any(|node_key| node_key == key)
    }
}

/// A builder for creating new nodes.
pub struct NodeBuilder {
    identity: String,
    endpoints: Vec<String>,
    display_name: Option<String>,
    keys: Vec<String>,
    metadata: HashMap<String, String>,
}

impl NodeBuilder {
    /// Create a new `NodeBuilder` with the node's `identity`.
    pub fn new<S: Into<String>>(identity: S) -> Self {
        Self {
            identity: identity.into(),
            endpoints: vec![],
            display_name: None,
            keys: vec![],
            metadata: HashMap::new(),
        }
    }

    /// Add the `endpoint` to the builder.
    pub fn with_endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.endpoints.push(endpoint.into());
        self
    }

    /// Add all of the `endpoints` to the builder.
    pub fn with_endpoints<V: Into<Vec<String>>>(mut self, endpoints: V) -> Self {
        self.endpoints.append(&mut endpoints.into());
        self
    }

    /// Set the node's `display_name`.
    pub fn with_display_name<S: Into<String>>(mut self, display_name: S) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    /// Add the `key` to the builder.
    pub fn with_key<S: Into<String>>(mut self, key: S) -> Self {
        self.keys.push(key.into());
        self
    }

    /// Add all of the `keys` to the builder.
    pub fn with_keys<V: Into<Vec<String>>>(mut self, keys: V) -> Self {
        self.keys.append(&mut keys.into());
        self
    }

    /// Add the `key`/`value` pair to the node's metadata.
    pub fn with_metadata<S: Into<String>>(mut self, key: S, value: S) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Attempt to build the `Node`.
    pub fn build(self) -> Result<Node, InvalidNodeError> {
        let identity = self.identity;
        let display_name = self
            .display_name
            .unwrap_or_else(|| format!("Node {}", identity));

        let node = Node {
            identity,
            endpoints: self.endpoints,
            display_name,
            keys: self.keys,
            metadata: self.metadata,
        };

        check_node_required_fields_are_not_empty(&node)?;

        Ok(node)
    }
}

/// A predicate on a key/value pair in a Node's metadata table.
///
/// Each variant is an operator, and supplies a tuple representing a key/value pair. It is applied
/// by the comparison operator on the value found at the given key (the first item in the tuple)
/// against the predicate's value (the second item in the tuple).
///
/// If the item is missing in a node's metadata table, the predicate returns false (with the
/// exception of the `Ne` variant).
#[derive(Clone)]
pub enum MetadataPredicate {
    /// Applies the `==` operator.
    Eq(String, String),
    /// Applies the `!=` operator.
    Ne(String, String),
    /// Applies the `>` operator.
    Gt(String, String),
    /// Applies the `>=` operator.
    Ge(String, String),
    /// Applies the `<` operator.
    Lt(String, String),
    /// Applies the `<=` operator.
    Le(String, String),
}

impl MetadataPredicate {
    /// Apply this predicate against a given node.
    pub fn apply(&self, node: &Node) -> bool {
        match self {
            MetadataPredicate::Eq(key, val) => {
                node.metadata.get(key).map(|v| v == val).unwrap_or(false)
            }
            MetadataPredicate::Ne(key, val) => {
                // This returns true, if not found.  I.e. `val != nil == true`
                node.metadata.get(key).map(|v| v != val).unwrap_or(true)
            }
            MetadataPredicate::Gt(key, val) => {
                node.metadata.get(key).map(|v| v > val).unwrap_or(false)
            }
            MetadataPredicate::Ge(key, val) => {
                node.metadata.get(key).map(|v| v >= val).unwrap_or(false)
            }
            MetadataPredicate::Lt(key, val) => {
                node.metadata.get(key).map(|v| v < val).unwrap_or(false)
            }
            MetadataPredicate::Le(key, val) => {
                node.metadata.get(key).map(|v| v <= val).unwrap_or(false)
            }
        }
    }

    /// Returns the `Eq` predicate for the given key and value
    pub fn eq<S: Into<String>>(key: S, value: S) -> MetadataPredicate {
        MetadataPredicate::Eq(key.into(), value.into())
    }

    /// Returns the `Ne` predicate for the given key and value
    pub fn ne<S: Into<String>>(key: S, value: S) -> MetadataPredicate {
        MetadataPredicate::Ne(key.into(), value.into())
    }
}

/// Type returned by the `RegistryReader::list_nodes` method
pub type NodeIter<'a> = Box<dyn ExactSizeIterator<Item = Node> + Send + 'a>;

/// Defines registry read capabilities.
pub trait RegistryReader: Send + Sync {
    /// Returns an iterator over the nodes in the registry.
    ///
    /// # Arguments
    ///
    /// * `predicates` - A list of predicates to be applied to the resulting list. These are
    /// applied as an AND, from a query perspective. If the list is empty, it is the equivalent of
    /// no predicates (i.e. return all).
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError>;

    /// Returns the count of nodes in the registry.
    ///
    /// # Arguments
    ///
    /// * `predicates` - A list of predicates to be applied before counting the nodes. These are
    /// applied as an AND, from a query perspective. If the list is empty, it is the equivalent of
    /// no predicates (i.e. return all).
    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError>;

    /// Returns the node with the given identity, if it exists in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The identity of the node.
    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError>;

    /// Determines whether or not the node exists in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The identity of the node.
    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        self.fetch_node(identity).map(|opt| opt.is_some())
    }
}

/// Defines registry write capabilities.
pub trait RegistryWriter: Send + Sync {
    /// Adds a new node to the registry, or replaces an existing node with the same identity.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to be added to or updated in the registry.
    ///
    fn insert_node(&self, node: Node) -> Result<(), RegistryError>;

    /// Deletes a node with the given identity and returns the node if it was in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The Splinter identity of the node.
    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError>;
}

/// Provides a marker trait for a clonable, readable and writable registry.
pub trait RwRegistry: RegistryWriter + RegistryReader {
    /// Clone implementation for `RwRegistry`. The implementation of the `Clone` trait for
    /// `Box<RwRegistry>` calls this method.
    ///
    /// # Example
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn RwRegistry> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn RwRegistry>;

    /// Clone the `RwRegistry` as a `Box<dyn RegistryReader>`.
    fn clone_box_as_reader(&self) -> Box<dyn RegistryReader>;

    /// Clone the `RwRegistry` as a `Box<dyn RegistryWriter>`.
    fn clone_box_as_writer(&self) -> Box<dyn RegistryWriter>;
}

impl Clone for Box<dyn RwRegistry> {
    fn clone(&self) -> Box<dyn RwRegistry> {
        self.clone_box()
    }
}

impl<NR> RegistryReader for Box<NR>
where
    NR: RegistryReader + ?Sized,
{
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError> {
        (**self).list_nodes(predicates)
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        (**self).count_nodes(predicates)
    }

    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        (**self).fetch_node(identity)
    }

    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        (**self).has_node(identity)
    }
}

impl<NW> RegistryWriter for Box<NW>
where
    NW: RegistryWriter + ?Sized,
{
    fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
        (**self).insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        (**self).delete_node(identity)
    }
}

/// Returns `Err` if not all `nodes` are valid.
fn validate_nodes(nodes: &[Node]) -> Result<(), InvalidNodeError> {
    for (idx, node) in nodes.iter().enumerate() {
        check_node_required_fields_are_not_empty(node)?;
        check_if_node_is_duplicate(node, &nodes[idx + 1..])?;
    }
    Ok(())
}

/// Checks emptiness properties of all fields on the given `node`.
fn check_node_required_fields_are_not_empty(node: &Node) -> Result<(), InvalidNodeError> {
    if node.identity.is_empty() {
        Err(InvalidNodeError::EmptyIdentity)
    } else if node.endpoints.is_empty() {
        Err(InvalidNodeError::MissingEndpoints)
    } else if node.endpoints.iter().any(|endpoint| endpoint.is_empty()) {
        Err(InvalidNodeError::EmptyEndpoint)
    } else if node.display_name.is_empty() {
        Err(InvalidNodeError::EmptyDisplayName)
    } else if node.keys.is_empty() {
        Err(InvalidNodeError::MissingKeys)
    } else if node.keys.iter().any(|key| key.is_empty()) {
        Err(InvalidNodeError::EmptyKey)
    } else {
        Ok(())
    }
}

/// Checks if the given `node` is a duplicate of any in the slice of `existing_nodes`.
fn check_if_node_is_duplicate(
    node: &Node,
    existing_nodes: &[Node],
) -> Result<(), InvalidNodeError> {
    existing_nodes.iter().try_for_each(|existing_node| {
        if existing_node.identity == node.identity {
            Err(InvalidNodeError::DuplicateIdentity(node.identity.clone()))
        } else if let Some(endpoint) = existing_node
            .endpoints
            .iter()
            .find(|endpoint| node.endpoints.contains(endpoint))
        {
            Err(InvalidNodeError::DuplicateEndpoint(endpoint.clone()))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the `NodeBuilder` properly constructs a new `Node` when just the minimum values
    /// are provided. Also verifies that the node builder can be initialized with the
    /// `Node::builder` method.
    ///
    /// * The identity field should match the value provided to the `Node::builder` method
    /// * The set endpoint should be the only endpoint for the node
    /// * The display name should be set to a default value of "Node <identity>"
    /// * The set key should be the only key for the node
    /// * The metadata should be empty, because metadata is optional and no entries are provided
    #[test]
    fn node_builder_minimum() {
        let node = Node::builder("identity")
            .with_endpoint("endpoint")
            .with_key("key")
            .build()
            .expect("Failed to build node");

        assert_eq!(&node.identity, "identity");
        assert_eq!(node.endpoints, vec!["endpoint".to_string()]);
        assert_eq!(node.display_name, format!("Node {}", node.identity));
        assert_eq!(node.keys, vec!["key".to_string()]);
        assert!(node.metadata.is_empty());
    }

    /// Verify that the `NodeBuilder` properly constructs a new `Node` when all builder methods are
    /// used.
    ///
    /// * The identity field should match the value provided to the `NodeBuilder::new` method
    /// * All endpoints provided using the `with_endpoint` and `with_endpoints` methods should be
    ///   in the node's endpoints
    /// * The display name should match the value provided using the `with_display_name` method
    /// * All keys provided using the `with_key` and `with_keys` methods should be in the node's
    ///   keys
    /// * The metadata should include all of the entries provided using the `with_metadata` method
    #[test]
    fn node_builder_all_fields() {
        let node = NodeBuilder::new("identity")
            .with_endpoint("endpoint1")
            .with_endpoints(vec!["endpoint2".into(), "endpoint3".into()])
            .with_display_name("display name")
            .with_key("key1")
            .with_keys(vec!["key2".into(), "key3".into()])
            .with_metadata("k1", "v1")
            .with_metadata("k2", "v2")
            .build()
            .expect("Failed to build node");

        assert_eq!(&node.identity, "identity");
        assert_eq!(
            node.endpoints,
            vec![
                "endpoint1".to_string(),
                "endpoint2".to_string(),
                "endpoint3".to_string()
            ]
        );
        assert_eq!(&node.display_name, "display name");
        assert_eq!(
            node.keys,
            vec!["key1".to_string(), "key2".to_string(), "key3".to_string()]
        );
        assert_eq!(node.metadata.len(), 2);
        assert_eq!(node.metadata.get("k1"), Some(&"v1".to_string()));
        assert_eq!(node.metadata.get("k2"), Some(&"v2".to_string()));
    }

    /// Verify that the `NodeBuilder` checks all the required fields for emptiness.
    ///
    /// * `identity` must be non-empty
    /// * `endpoints` must be non-empty
    /// * All `endpoints` entries must be non-empty
    /// * `keys` must be non-empty
    /// * All `keys` entries must be non-empty
    #[test]
    fn node_builder_required_fields_emptiness() {
        match NodeBuilder::new("")
            .with_endpoint("endpoint")
            .with_key("key")
            .build()
        {
            Err(InvalidNodeError::EmptyIdentity) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyIdentity), got: {:?}",
                res
            ),
        }

        match NodeBuilder::new("identity").with_key("key").build() {
            Err(InvalidNodeError::MissingEndpoints) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::MissingEndpoints), got: {:?}",
                res
            ),
        }

        match NodeBuilder::new("identity")
            .with_endpoint("")
            .with_key("key")
            .build()
        {
            Err(InvalidNodeError::EmptyEndpoint) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyEndpoint), got: {:?}",
                res
            ),
        }

        match NodeBuilder::new("identity")
            .with_endpoint("endpoint")
            .build()
        {
            Err(InvalidNodeError::MissingKeys) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::MissingKeys), got: {:?}",
                res
            ),
        }

        match NodeBuilder::new("identity")
            .with_endpoint("endpoint")
            .with_key("")
            .build()
        {
            Err(InvalidNodeError::EmptyKey) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyKey), got: {:?}",
                res
            ),
        }
    }

    /// Verify that the `Node::has_key` method properly determines whether or not a key belongs to
    /// a node.
    #[test]
    fn node_has_key() {
        let node = Node::builder("identity")
            .with_endpoint("endpoint")
            .with_key("key")
            .build()
            .expect("Failed to build node");

        assert!(node.has_key("key"));
        assert!(!node.has_key("other"));
    }

    /// Verify that the `MetadataPredicate::apply` method properly determines if a node satisfies
    /// the predicate for each of the predicate variants.
    #[test]
    fn metadata_predicates() {
        let node = Node::builder("identity")
            .with_endpoint("endpoint")
            .with_key("key")
            .with_metadata("key", "5".into())
            .build()
            .expect("Failed to build node");

        assert!(MetadataPredicate::Eq("key".into(), "5".into()).apply(&node));
        assert!(!MetadataPredicate::Eq("key".into(), "4".into()).apply(&node));

        assert!(MetadataPredicate::Ne("key".into(), "4".into()).apply(&node));
        assert!(!MetadataPredicate::Ne("key".into(), "5".into()).apply(&node));

        assert!(MetadataPredicate::Gt("key".into(), "4".into()).apply(&node));
        assert!(!MetadataPredicate::Gt("key".into(), "5".into()).apply(&node));
        assert!(!MetadataPredicate::Gt("key".into(), "6".into()).apply(&node));

        assert!(MetadataPredicate::Ge("key".into(), "4".into()).apply(&node));
        assert!(MetadataPredicate::Ge("key".into(), "5".into()).apply(&node));
        assert!(!MetadataPredicate::Ge("key".into(), "6".into()).apply(&node));

        assert!(MetadataPredicate::Lt("key".into(), "6".into()).apply(&node));
        assert!(!MetadataPredicate::Lt("key".into(), "5".into()).apply(&node));
        assert!(!MetadataPredicate::Lt("key".into(), "4".into()).apply(&node));

        assert!(MetadataPredicate::Le("key".into(), "6".into()).apply(&node));
        assert!(MetadataPredicate::Le("key".into(), "5".into()).apply(&node));
        assert!(!MetadataPredicate::Le("key".into(), "4".into()).apply(&node));
    }

    /// Verify that the `validate_nodes` method properly validates nodes based on the following
    /// criteria:
    ///
    /// * `identity` must be non-empty
    /// * `endpoints` must be non-empty
    /// * All `endpoints` entries must be non-empty
    /// * `display_name` must be non-empty
    /// * `keys` must be non-empty
    /// * All `keys` entries must be non-empty
    /// * All identities must be unique with respect to the other nodes
    /// * All endpoints must be unique with respect to the other nodes
    #[test]
    fn node_validation() {
        let node1 = Node::builder("identity1")
            .with_endpoint("endpoint1")
            .with_key("key1")
            .build()
            .expect("Failed to build node1");
        let node2 = Node::builder("identity2")
            .with_endpoint("endpoint2")
            .with_key("key2")
            .build()
            .expect("Failed to build node2");

        let empty_identity = Node {
            identity: "".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), empty_identity]) {
            Err(InvalidNodeError::EmptyIdentity) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyIdentity), got: {:?}",
                res
            ),
        }

        let missing_endpoints = Node {
            identity: "identity3".into(),
            endpoints: vec![],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), missing_endpoints]) {
            Err(InvalidNodeError::MissingEndpoints) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::MissingEndpoints), got: {:?}",
                res
            ),
        }

        let empty_endpoint = Node {
            identity: "identity3".into(),
            endpoints: vec!["".into()],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), empty_endpoint]) {
            Err(InvalidNodeError::EmptyEndpoint) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyEndpoint), got: {:?}",
                res
            ),
        }

        let empty_display_name = Node {
            identity: "identity3".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), empty_display_name]) {
            Err(InvalidNodeError::EmptyDisplayName) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyDisplayName), got: {:?}",
                res
            ),
        }

        let missing_keys = Node {
            identity: "identity3".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "display name".into(),
            keys: vec![],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), missing_keys]) {
            Err(InvalidNodeError::MissingKeys) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::MissingKeys), got: {:?}",
                res
            ),
        }

        let empty_key = Node {
            identity: "identity3".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "display name".into(),
            keys: vec!["".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), empty_key]) {
            Err(InvalidNodeError::EmptyKey) => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::EmptyKey), got: {:?}",
                res
            ),
        }

        let duplicate_identity = Node {
            identity: "identity1".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), duplicate_identity]) {
            Err(InvalidNodeError::DuplicateIdentity(id)) if &id == "identity1" => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::DuplicateIdentity), got: {:?}",
                res
            ),
        }

        let duplicate_endpoint = Node {
            identity: "identity3".into(),
            endpoints: vec!["endpoint1".into()],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        match validate_nodes(&[node1.clone(), node2.clone(), duplicate_endpoint]) {
            Err(InvalidNodeError::DuplicateEndpoint(endpoint)) if &endpoint == "endpoint1" => {}
            res => panic!(
                "Result should have been Err(InvalidNodeError::DuplicateEndpoint), got: {:?}",
                res
            ),
        }

        let valid_node3 = Node {
            identity: "identity3".into(),
            endpoints: vec!["endpoint3".into()],
            display_name: "display name".into(),
            keys: vec!["key3".into()],
            metadata: HashMap::new(),
        };
        assert!(validate_nodes(&[node1, node2, valid_node3]).is_ok());
    }
}
