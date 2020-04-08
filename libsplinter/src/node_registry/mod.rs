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

//! Data structures, traits, and implementations for tracking and managing known Splinter nodes.
//!
//! The public node registry interface is defined primarily by the [`Node`] data structure (along
//! with its builder, [`NodeBuilder`]), and the node registry traits: [`NodeRegistryReader`],
//! [`NodeRegistryWriter`], and [`RwNodeRegistry`].
//!
//! The following node registry implementations are provided by this module:
//!
//! * [`LocalYamlNodeRegistry`] - A read/write registry that is backed by a local YAML file.
//! * [`UnifiedNodeRegistry`] - A read/write registry with a single read/write sub-registry and an
//!   arbitrary number of read-only sub-registries.
//!
//! [`Node`]: struct.Node.html
//! [`NodeBuilder`]: struct.NodeBuilder.html
//! [`NodeRegistryReader`]: trait.NodeRegistryReader.html
//! [`NodeRegistryWriter`]: trait.NodeRegistryWriter.html
//! [`RwNodeRegistry`]: trait.RwNodeRegistry.html
//! [`LocalYamlNodeRegistry`]: struct.LocalYamlNodeRegistry.html
//! [`UnifiedNodeRegistry`]: struct.UnifiedNodeRegistry.html

mod error;
#[cfg(feature = "rest-api")]
pub mod rest_api;
mod unified;
mod yaml;

use std::collections::HashMap;

pub use error::{InvalidNodeError, NodeRegistryError};
pub use unified::UnifiedNodeRegistry;
pub use yaml::LocalYamlNodeRegistry;
#[cfg(feature = "registry-remote")]
pub use yaml::{RemoteYamlNodeRegistry, RemoteYamlShutdownHandle};

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
    /// A map with node metadata.
    pub metadata: HashMap<String, String>,
}

/// A builder for creating new nodes.
pub struct NodeBuilder {
    identity: String,
    endpoints: Vec<String>,
    display_name: Option<String>,
    metadata: HashMap<String, String>,
}

impl NodeBuilder {
    /// Create a new `NodeBuilder` with the node's `identity`.
    pub fn new<S: Into<String>>(identity: S) -> Self {
        Self {
            identity: identity.into(),
            endpoints: vec![],
            display_name: None,
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

/// Defines node registry read capabilities.
pub trait NodeRegistryReader: Send + Sync {
    /// Returns an iterator over the nodes in the registry.
    ///
    /// # Arguments
    ///
    /// * `predicates` - A list of of predicates to be applied to the resulting list. These are
    /// applied as an AND, from a query perspective. If the list is empty, it is the equivalent of
    /// no predicates (i.e. return all).
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<Box<dyn Iterator<Item = Node> + Send + 'a>, NodeRegistryError>;

    /// Returns the count of nodes in the registry.
    ///
    /// # Arguments
    ///
    /// * `predicates` - A list of of predicates to be applied before counting the nodes. These are
    /// applied as an AND, from a query perspective. If the list is empty, it is the equivalent of
    /// no predicates (i.e. return all).
    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError>;

    /// Returns the node with the given identity, if it exists in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The identity of the node.
    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError>;

    /// Determines whether or not the node exists in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The identity of the node.
    fn has_node(&self, identity: &str) -> Result<bool, NodeRegistryError> {
        self.fetch_node(identity).map(|opt| opt.is_some())
    }
}

/// Defines node registry write capabilities.
pub trait NodeRegistryWriter: Send + Sync {
    /// Adds a new node to the registry, or replaces an existing node with the same identity.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to be added to or updated in the registry.
    ///
    fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError>;

    /// Deletes a node with the given identity and returns the node if it was in the registry.
    ///
    /// # Arguments
    ///
    ///  * `identity` - The Splinter identity of the node.
    fn delete_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError>;
}

/// Provides a marker trait for a clonable, readable and writable node registry.
pub trait RwNodeRegistry: NodeRegistryWriter + NodeRegistryReader {
    /// Clone implementation for `RwNodeRegistry`. The implementation of the `Clone` trait for
    /// `Box<RwNodeRegistry>` calls this method.
    ///
    /// # Example
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn RwNodeRegistry> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn RwNodeRegistry>;

    /// Clone the `RwNodeRegistry` as a `Box<dyn NodeRegistryReader>`.
    fn clone_box_as_reader(&self) -> Box<dyn NodeRegistryReader>;

    /// Clone the `RwNodeRegistry` as a `Box<dyn NodeRegistryWriter>`.
    fn clone_box_as_writer(&self) -> Box<dyn NodeRegistryWriter>;
}

impl Clone for Box<dyn RwNodeRegistry> {
    fn clone(&self) -> Box<dyn RwNodeRegistry> {
        self.clone_box()
    }
}

impl<NR> NodeRegistryReader for Box<NR>
where
    NR: NodeRegistryReader + ?Sized,
{
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<Box<dyn Iterator<Item = Node> + Send + 'a>, NodeRegistryError> {
        (**self).list_nodes(predicates)
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError> {
        (**self).count_nodes(predicates)
    }

    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
        (**self).fetch_node(identity)
    }

    fn has_node(&self, identity: &str) -> Result<bool, NodeRegistryError> {
        (**self).has_node(identity)
    }
}

impl<NW> NodeRegistryWriter for Box<NW>
where
    NW: NodeRegistryWriter + ?Sized,
{
    fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError> {
        (**self).insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
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
