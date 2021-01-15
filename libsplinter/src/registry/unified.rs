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

//! A registry with multiple sources.
//!
//! This module contains the [`UnifiedRegistry`], which provides an implementation of the
//! [`RwRegistry`] trait.
//!
//! [`UnifiedRegistry`]: struct.UnifiedRegistry.html
//! [`RwRegistry`]: ../trait.RwRegistry.html

use std::collections::HashMap;
use std::sync::Arc;

use super::{
    MetadataPredicate, Node, NodeIter, RegistryError, RegistryReader, RegistryWriter, RwRegistry,
};

/// A registry with multiple sources.
///
/// The `UnifiedRegistry` provides a unified view of multiple source registries. It has one internal
/// read-write registry and an arbitrary number of external read-only registries.
///
/// # Writing
///
/// All write operations (provided by the implementation of the [`RegistryWriter`] trait) affect
/// only the internal read-write registry.
///
/// # Reading
///
/// Read operations (provided by the [`RegistryReader`] implementation) provide [`Node`] data from
/// all source registries.
///
/// If a [`Node`] exists in more than one registry (nodes are considered duplicates if they have the
/// same [`identity`]), then the definition of the [`Node`] from the registry with the highest
/// precedence is used, with the exception of the node's [`metadata`] (see the [`Metadata Merging`]
/// section below).
///
/// If reading a source registry fails, the error will be logged and the registry will be ignored.
///
/// ## Registry Precedence
///
/// The internal read-write registry has the highest precedence, followed by the read-only
/// registries. The precedence of the read-only registries is based on the order they appear (the
/// earlier in the list, the higher the priority).
///
/// ## Metadata Merging
///
/// When the same node exists in multiple registries, the [`metadata`] is merged from all sources.
/// If the same metadata key is set for the node in different registires, the value for that key
/// from the highest-precedence registry will be used.
///
/// [`RegistryReader`]: ../trait.RegistryReader.html
/// [`RegistryWriter`]: ../trait.RegistryWriter.html
/// [`RwRegistry`]: ../trait.RwRegistry.html
/// [`Node`]: ../struct.Node.html
/// [`identity`]: ../struct.Node.html#structfield.identity
/// [`metadata`]: ../struct.Node.html#structfield.metadata
/// [`Metadata Merging`]: #metadata-merging
#[derive(Clone)]
pub struct UnifiedRegistry {
    internal_source: Arc<dyn RwRegistry>,
    external_sources: Vec<Arc<dyn RegistryReader>>,
}

impl UnifiedRegistry {
    /// Constructs a new `UnifiedRegistry` with an internal read-write registry and an arbitrary
    /// number of read-only registries.
    pub fn new(
        internal_source: Box<dyn RwRegistry>,
        external_sources: Vec<Box<dyn RegistryReader>>,
    ) -> Self {
        Self {
            internal_source: internal_source.into(),
            external_sources: external_sources.into_iter().map(Arc::from).collect(),
        }
    }

    /// Gets all nodes from all sources (in ascending order of precedence) without deduplication.
    fn all_nodes<'a>(&'a self) -> Box<dyn Iterator<Item = Node> + 'a> {
        Box::new(
            // Get node iterators from all read-only sources
            self.external_sources
                .iter()
                .map(|registry| registry.list_nodes(&[]))
                // Reverse the sources, so lowest precedence is first
                .rev()
                // Add the internal source's node iterator to the end, since it has highest
                // precedence
                .chain(std::iter::once(self.internal_source.list_nodes(&[])))
                // Log any errors from the `list_nodes` calls and ignore the failing registries
                .filter_map(|res| {
                    res.map_err(|err| debug!("Failed to list nodes in source registry: {}", err))
                        .ok()
                })
                // Flatten into a single iterator
                .flatten(),
        )
    }
}

impl RegistryReader for UnifiedRegistry {
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError> {
        let mut id_map = self
            // Get all nodes from all sources
            .all_nodes()
            // Deduplicate and merge metadata
            .fold(HashMap::<String, Node>::new(), |mut acc, mut node| {
                // If the node is already present, merge metadata
                if let Some(existing) = acc.remove(&node.identity) {
                    // Overwrite the existing node's metadata with the new node's if they share
                    // the same metadata keys
                    let mut merged_metadata = existing.metadata;
                    merged_metadata.extend(node.metadata);
                    node.metadata = merged_metadata;
                }
                acc.insert(node.identity.clone(), node);
                acc
            });
        // Apply predicate filters
        id_map.retain(|_, node| predicates.iter().all(|predicate| predicate.apply(node)));

        Ok(Box::new(id_map.into_iter().map(|(_, node)| node)))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        self.list_nodes(predicates).map(|iter| iter.count() as u32)
    }

    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        // Get node from all read-only sources
        Ok(self
            .external_sources
            .iter()
            .map(|registry| registry.fetch_node(identity))
            // Reverse the sources, so lowest precedence is first
            .rev()
            // Get node from the internal source and add it to the end, since it has highest
            // precedence
            .chain(std::iter::once(self.internal_source.fetch_node(identity)))
            // Log any errors from the `fetch_node` calls and ignore the failing registries
            .filter_map(|res| {
                res.map_err(|err| debug!("Failed to fetch node from source registry: {}", err))
                    .ok()
            })
            // Merge metadata and get the highest-precedence definition of the node if it exists
            .fold(None, |final_opt, fetch_opt| {
                match fetch_opt {
                    Some(mut node) => {
                        // If the node was already found at a lower precedence, merge metadata
                        if let Some(existing) = final_opt {
                            // Overwrite the existing node's metadata with the new node's if they
                            // share the same metadata keys
                            let mut merged_metadata = existing.metadata;
                            merged_metadata.extend(node.metadata);
                            node.metadata = merged_metadata;
                        }
                        Some(node)
                    }
                    None => final_opt,
                }
            }))
    }
}

impl RegistryWriter for UnifiedRegistry {
    fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
        self.internal_source.insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        self.internal_source.delete_node(identity)
    }
}

impl RwRegistry for UnifiedRegistry {
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::iter::FromIterator;
    use std::sync::{Arc, Mutex};

    use super::*;

    fn new_node(id: &str, endpoint: &str, metadata: &[(&str, &str)]) -> Node {
        let mut builder = Node::builder(id).with_endpoint(endpoint).with_key("abcd");
        for (key, val) in metadata {
            builder = builder.with_metadata(*key, *val);
        }
        builder.build().expect("Failed to build node")
    }

    /// Verify that the number of nodes is correctly reported when all registries are empty.
    #[test]
    fn node_count_empty() {
        let unified = UnifiedRegistry::new(
            Box::new(MemRegistry::default()),
            vec![Box::new(MemRegistry::default())],
        );
        assert_eq!(0, unified.count_nodes(&[]).expect("Unable to get count"));
    }

    /// Verify that the number of nodes is correctly reported when the same node exists across
    /// registries.
    #[test]
    fn node_count_multiple() {
        let node1 = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let node2 = new_node("node2", "endpoint2", &[("meta_b", "val_b")]);
        let node3 = new_node("node1", "endpoint3", &[("meta_c", "val_c")]);

        let writeable = MemRegistry::default();
        writeable
            .insert_node(node1)
            .expect("Unable to insert node1");
        writeable
            .insert_node(node2)
            .expect("Unable to insert node2");

        let readable = MemRegistry::default();
        writeable
            .insert_node(node3)
            .expect("Unable to insert node3");

        let unified = UnifiedRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

        assert_eq!(2, unified.count_nodes(&[]).expect("Unable to get count"));
    }

    /// Verify that the number of nodes is correctly reported when metadata predicate are provided.
    #[test]
    fn node_count_with_predicates() {
        let node1 = new_node(
            "node1",
            "endpoint1",
            &[("meta_a", "val_a"), ("meta_b", "val_b")],
        );
        let node2 = new_node(
            "node2",
            "endpoint2",
            &[("meta_a", "val_c"), ("meta_b", "val_b")],
        );
        let node3 = new_node(
            "node1",
            "endpoint3",
            &[("meta_a", "val_a"), ("meta_b", "val_c")],
        );

        let writeable = MemRegistry::default();
        writeable
            .insert_node(node1)
            .expect("Unable to insert node1");
        writeable
            .insert_node(node2)
            .expect("Unable to insert node2");

        let readable = MemRegistry::default();
        readable.insert_node(node3).expect("Unable to insert node3");

        let unified = UnifiedRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

        assert_eq!(
            1,
            unified
                .count_nodes(&[
                    MetadataPredicate::eq("meta_a", "val_a"),
                    MetadataPredicate::ne("meta_b", "val_c")
                ])
                .expect("Unable to get count")
        );
    }

    /// Verify that a node is fetched from a read-only source if it only exists there.
    #[test]
    fn fetch_node_read_only() {
        let node = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);

        let readable = MemRegistry::default();
        readable
            .insert_node(node.clone())
            .expect("Unable to insert node");

        let unified =
            UnifiedRegistry::new(Box::new(MemRegistry::default()), vec![Box::new(readable)]);

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(node, retreived_node);
    }

    /// Verify that a node is fetched from the internal source if it only exists there.
    #[test]
    fn fetch_node_internal() {
        let node = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);

        let writable = MemRegistry::default();
        writable
            .insert_node(node.clone())
            .expect("Unable to insert node");

        let unified =
            UnifiedRegistry::new(Box::new(writable), vec![Box::new(MemRegistry::default())]);

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(node, retreived_node);
    }

    /// Verify that a node is fetched from the highest-precedence read-only source if it does not
    /// exist in the internal registry, and that the metadata is properly merged.
    ///
    /// 1. Add the same node to three read-only registries with different endpoints and metadata.
    /// 2. Add the read-only registries to a unified registry, along with an empty writable
    ///    registry.
    /// 3. Fetch the node and verify that it has the correct data (endpoint from highest-precedence
    ///    registry, metadata merged from all registries).
    #[test]
    fn fetch_node_read_only_precedence() {
        let high_precedence_node = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let med_precedence_node = new_node("node1", "endpoint2", &[("meta_b", "val_b")]);
        let low_precedence_node = new_node("node1", "endpoint3", &[("meta_a", "val_c")]);
        let expected_node = new_node(
            "node1",
            "endpoint1",
            &[("meta_a", "val_a"), ("meta_b", "val_b")],
        );

        let high_precedence_readable = MemRegistry::default();
        high_precedence_readable
            .insert_node(high_precedence_node)
            .expect("Unable to insert high-precedence node");

        let med_precedence_readable = MemRegistry::default();
        med_precedence_readable
            .insert_node(med_precedence_node)
            .expect("Unable to insert medium-precedence node");

        let low_precedence_readable = MemRegistry::default();
        low_precedence_readable
            .insert_node(low_precedence_node)
            .expect("Unable to insert low-precedence node");

        let unified = UnifiedRegistry::new(
            Box::new(MemRegistry::default()),
            vec![
                Box::new(high_precedence_readable),
                Box::new(med_precedence_readable),
                Box::new(low_precedence_readable),
            ],
        );

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(expected_node, retreived_node);
    }

    /// Verify that a node is fetched from the internal source even if it exists in one or more
    /// read-only registries, and that the metadata is properly merged.
    ///
    /// 1. Add the same node to the internal registry and two read-only registries with different
    ///    endpoints and metadata.
    /// 2. Add the registries to a unified registry.
    /// 3. Fetch the node and verify that it has the correct data (endpoints from internal registry,
    ///    metadata merged from all sources).
    #[test]
    fn fetch_node_internal_precedence() {
        let high_precedence_node = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let med_precedence_node = new_node("node1", "endpoint2", &[("meta_b", "val_b")]);
        let low_precedence_node = new_node("node1", "endpoint3", &[("meta_a", "val_c")]);
        let expected_node = new_node(
            "node1",
            "endpoint1",
            &[("meta_a", "val_a"), ("meta_b", "val_b")],
        );

        let writable = MemRegistry::default();
        writable
            .insert_node(high_precedence_node)
            .expect("Unable to insert high-precedence node");

        let med_precedence_readable = MemRegistry::default();
        med_precedence_readable
            .insert_node(med_precedence_node)
            .expect("Unable to insert medium-precedence node");

        let low_precedence_readable = MemRegistry::default();
        low_precedence_readable
            .insert_node(low_precedence_node)
            .expect("Unable to insert low-precedence node");

        let unified = UnifiedRegistry::new(
            Box::new(writable),
            vec![
                Box::new(med_precedence_readable),
                Box::new(low_precedence_readable),
            ],
        );

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(expected_node, retreived_node);
    }

    /// Verify that listed nodes are properly returned based on precedence and that metadata is
    /// correctly merged.
    ///
    /// 1. Add the same node to the internal registry and a read-only registry with different data.
    /// 2. Add the same node to two read-only registries with different data.
    /// 3. Add all three registries to a unified registry.
    /// 4. List the nodes and verify that the correct node data is returned.
    #[test]
    fn list_nodes_precedence() {
        let node1_internal = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let node1_read_only = new_node(
            "node1",
            "endpoint3",
            &[("meta_a", "val_c"), ("meta_b", "val_b")],
        );
        let node2_high = new_node("node2", "endpoint2", &[("meta_a", "val_a")]);
        let node2_low = new_node(
            "node2",
            "endpoint3",
            &[("meta_a", "val_c"), ("meta_b", "val_b")],
        );

        let expected_nodes = HashMap::from_iter(vec![
            (
                "node1".to_string(),
                new_node(
                    "node1",
                    "endpoint1",
                    &[("meta_a", "val_a"), ("meta_b", "val_b")],
                ),
            ),
            (
                "node2".to_string(),
                new_node(
                    "node2",
                    "endpoint2",
                    &[("meta_a", "val_a"), ("meta_b", "val_b")],
                ),
            ),
        ]);

        let writable = MemRegistry::default();
        writable
            .insert_node(node1_internal)
            .expect("Unable to insert internal node1");

        let readable_high = MemRegistry::default();
        readable_high
            .insert_node(node1_read_only)
            .expect("Unable to insert read-only node1");
        readable_high
            .insert_node(node2_high)
            .expect("Unable to insert high-precedence node2");

        let readable_low = MemRegistry::default();
        readable_low
            .insert_node(node2_low)
            .expect("Unable to insert low-precedence node2");

        let unified = UnifiedRegistry::new(
            Box::new(writable),
            vec![Box::new(readable_high), Box::new(readable_low)],
        );

        let nodes = unified
            .list_nodes(&[])
            .expect("Unable to list nodes")
            .map(|node| (node.identity.clone(), node))
            .collect::<HashMap<_, _>>();

        assert_eq!(expected_nodes, nodes);
    }

    /// Verify that listed nodes are properly returned when metadata predicates are provided.
    #[test]
    fn list_nodes_with_predicates() {
        let node1 = new_node(
            "node1",
            "endpoint1",
            &[("meta_a", "val_a"), ("meta_b", "val_b")],
        );
        let node2 = new_node(
            "node2",
            "endpoint2",
            &[("meta_a", "val_c"), ("meta_b", "val_b")],
        );
        let node3 = new_node(
            "node1",
            "endpoint3",
            &[("meta_a", "val_a"), ("meta_b", "val_c")],
        );

        let writeable = MemRegistry::default();
        writeable
            .insert_node(node1.clone())
            .expect("Unable to insert node1");
        writeable
            .insert_node(node2)
            .expect("Unable to insert node2");

        let readable = MemRegistry::default();
        readable.insert_node(node3).expect("Unable to insert node3");

        let unified = UnifiedRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

        let predicates = vec![
            MetadataPredicate::eq("meta_a", "val_a"),
            MetadataPredicate::ne("meta_b", "val_c"),
        ];
        let mut nodes = unified
            .list_nodes(&predicates)
            .expect("Unable to get count");

        assert_eq!(Some(node1), nodes.next());
        assert_eq!(None, nodes.next());
    }

    /// Verify that the `NodeRegistryWriter` implementation affects only the internal registry.
    #[test]
    fn write_nodes() {
        let node1 = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let node2 = new_node("node2", "endpoint2", &[("meta_b", "val_b")]);

        let writeable = MemRegistry::default();

        let readable = MemRegistry::default();
        readable
            .insert_node(node2.clone())
            .expect("Unable to insert node2 into read-only registry");

        let unified = UnifiedRegistry::new(
            Box::new(writeable.clone()),
            vec![Box::new(readable.clone())],
        );

        // Verify node1 is only added to writeable
        unified
            .insert_node(node1.clone())
            .expect("Unable to add node1");
        assert!(unified
            .has_node(&node1.identity)
            .expect("Unable to check unified for node1"));
        assert!(writeable
            .has_node(&node1.identity)
            .expect("Unable to check writeable for node1"));
        assert!(!readable
            .has_node(&node1.identity)
            .expect("Unable to check readable for node1"));

        // Verify removing node2 is None, node stays in readable
        assert!(unified
            .delete_node(&node2.identity)
            .expect("Unable to remove node2")
            .is_none());
        assert!(unified
            .has_node(&node2.identity)
            .expect("Unable to check unified for node2"));
        assert!(readable
            .has_node(&node2.identity)
            .expect("Unable to check readable for node2"));

        // Verify removing node1 is Some, node no longer in writeable
        assert_eq!(
            Some(node1.clone()),
            unified
                .delete_node(&node1.identity)
                .expect("Unable to remove node1")
        );
        assert!(!unified
            .has_node(&node1.identity)
            .expect("Unable to check unified for node1"));
        assert!(!writeable
            .has_node(&node1.identity)
            .expect("Unable to check writeable for node1"));
    }

    #[derive(Clone, Default)]
    struct MemRegistry {
        nodes: Arc<Mutex<HashMap<String, Node>>>,
    }

    impl RegistryReader for MemRegistry {
        fn list_nodes<'a, 'b: 'a>(
            &'b self,
            predicates: &'a [MetadataPredicate],
        ) -> Result<NodeIter<'a>, RegistryError> {
            let mut nodes = self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .clone();
            nodes.retain(|_, node| predicates.iter().all(|predicate| predicate.apply(node)));
            Ok(Box::new(nodes.into_iter().map(|(_, node)| node)))
        }

        fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
            self.list_nodes(predicates).map(|iter| iter.count() as u32)
        }

        fn fetch_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .get(identity)
                .cloned())
        }
    }

    impl RegistryWriter for MemRegistry {
        fn insert_node(&self, node: Node) -> Result<(), RegistryError> {
            self.nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .insert(node.identity.clone(), node);
            Ok(())
        }

        fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .remove(identity))
        }
    }

    impl RwRegistry for MemRegistry {
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
}
