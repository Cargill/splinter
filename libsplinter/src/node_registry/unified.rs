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

//! A node registry with multiple sources.
//!
//! This module contains the [`UnifiedNodeRegistry`], which provides an implementation of the
//! [`RwNodeRegistry`] trait.
//!
//! [`UnifiedNodeRegistry`]: struct.UnifiedNodeRegistry.html
//! [`RwNodeRegistry`]: ../trait.RwNodeRegistry.html

use std::collections::HashMap;
use std::sync::Arc;

use super::{
    MetadataPredicate, Node, NodeRegistryError, NodeRegistryReader, NodeRegistryWriter,
    RwNodeRegistry,
};

/// A node registry with multiple sources.
///
/// The `UnifiedNodeRegistry` provides a unified view of multiple node registries. It has one local
/// read-write registry and an arbitrary number of read-only registries.
///
/// # Writing
///
/// All write operations (provided by the implementation of the [`NodeRegistryWriter`] trait) affect
/// only the local read-write registry.
///
/// # Reading
///
/// Read operations (provided by the [`NodeRegistryReader`] implementation) provide [`Node`] data
/// from all source registries.
///
/// If a [`Node`] exists in more than one registry (nodes are considered duplicates if they have the
/// same [`identity`]), then the definition of the [`Node`] from the registry with the highest
/// precedence is used, with the exception of the node's [`metadata`] (see the
/// [`Metadata Merging`] section below).
///
/// ## Registry Precedence
///
/// The local read-write registry has the highest precedence, followed by the read-only registries.
/// The precedence of the read-only registries is based on the order they appear (the earlier in the
/// list, the higher the priority).
///
/// ## Metadata Merging
///
/// When the same node exists in multiple registries, the [`metadata`] is merged from all sources.
/// If the same metadata key is set for the node in different registires, the value for that key
/// from the highest-precedence registry will be used.
///
/// [`NodeRegistryReader`]: ../trait.NodeRegistryReader.html
/// [`NodeRegistryWriter`]: ../trait.NodeRegistryWriter.html
/// [`RwNodeRegistry`]: ../trait.RwNodeRegistry.html
/// [`Node`]: ../struct.Node.html
/// [`identity`]: ../struct.Node.html#structfield.identity
/// [`metadata`]: ../struct.Node.html#structfield.metadata
/// [`Metadata Merging`]: #metadata-merging
#[derive(Clone)]
pub struct UnifiedNodeRegistry {
    local_source: Arc<dyn RwNodeRegistry>,
    readable_sources: Vec<Arc<dyn NodeRegistryReader>>,
}

impl UnifiedNodeRegistry {
    /// Constructs a new `UnifiedNodeRegistry` with a local, read-write node registry and an
    /// arbitrary number of read-only node registries.
    pub fn new(
        local_source: Box<dyn RwNodeRegistry>,
        readable_sources: Vec<Box<dyn NodeRegistryReader>>,
    ) -> Self {
        Self {
            local_source: local_source.into(),
            readable_sources: readable_sources.into_iter().map(Arc::from).collect(),
        }
    }

    /// Gets all nodes from all sources (in ascending order of precedence) without deduplication.
    fn all_nodes<'a>(&'a self) -> Result<NodeIter<'a>, NodeRegistryError> {
        // Get node iterators from all read-only sources
        self.readable_sources
            .iter()
            .map(|registry| registry.list_nodes(&[]))
            // Reverse the sources, so lowest precedence is first
            .rev()
            // Add the local source's node iterator to the end, since it has highest precedence
            .chain(std::iter::once(self.local_source.list_nodes(&[])))
            // Flatten into a single iterator, returning any errors from the `list_nodes` calls
            .try_fold(
                Box::new(std::iter::empty()) as NodeIter<'a>,
                |chain, nodes_res| {
                    let v = nodes_res?.collect::<Vec<_>>();
                    Ok(Box::new(chain.chain(v.into_iter())) as NodeIter<'a>)
                },
            )
    }
}

// A convenience type to cleanup the `NodeRegistryReader::list_nodes` implementation
type NodeIter<'a> = Box<dyn Iterator<Item = Node> + Send + 'a>;

impl NodeRegistryReader for UnifiedNodeRegistry {
    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, NodeRegistryError> {
        Ok(Box::new(
            // Get all nodes from all sources
            self.all_nodes()?
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
                })
                // Convert to iterator of just the nodes
                .into_iter()
                .map(|(_, node)| node)
                // Apply predicate filters
                .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node))),
        ))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError> {
        self.list_nodes(predicates).map(|iter| iter.count() as u32)
    }

    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
        // Get node from all read-only sources
        self.readable_sources
            .iter()
            .map(|registry| registry.fetch_node(identity))
            // Reverse the sources, so lowest precedence is first
            .rev()
            // Get node from the local source and add it to the end, since it has highest precedence
            .chain(std::iter::once(self.local_source.fetch_node(identity)))
            // Merge metadata and get the highest-precedence definition of the node if it exists
            .try_fold(None, |final_opt: Option<Node>, fetch_res| {
                fetch_res.map(|fetch_opt| match fetch_opt {
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
                })
            })
    }
}

impl NodeRegistryWriter for UnifiedNodeRegistry {
    fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError> {
        self.local_source.insert_node(node)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
        self.local_source.delete_node(identity)
    }
}

impl RwNodeRegistry for UnifiedNodeRegistry {
    fn clone_box(&self) -> Box<dyn RwNodeRegistry> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;
    use std::iter::FromIterator;
    use std::sync::{Arc, Mutex};

    use super::*;

    fn new_node(id: &str, endpoint: &str, metadata: &[(&str, &str)]) -> Node {
        let mut node = Node::new(id, vec![endpoint.into()]);
        for (key, val) in metadata {
            node.metadata.insert(key.to_string(), val.to_string());
        }
        node
    }

    /// Verify that the number of nodes is correctly reported when all registries are empty.
    #[test]
    fn node_count_empty() {
        let unified = UnifiedNodeRegistry::new(
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

        let unified = UnifiedNodeRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

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

        let unified = UnifiedNodeRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

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
            UnifiedNodeRegistry::new(Box::new(MemRegistry::default()), vec![Box::new(readable)]);

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(node, retreived_node);
    }

    /// Verify that a node is fetched from the local source if it only exists there.
    #[test]
    fn fetch_node_local() {
        let node = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);

        let writable = MemRegistry::default();
        writable
            .insert_node(node.clone())
            .expect("Unable to insert node");

        let unified =
            UnifiedNodeRegistry::new(Box::new(writable), vec![Box::new(MemRegistry::default())]);

        let retreived_node = unified
            .fetch_node("node1")
            .expect("Unable to fetch node")
            .expect("Node not found");

        assert_eq!(node, retreived_node);
    }

    /// Verify that a node is fetched from the highest-precedence read-only source if it does not
    /// exist in the local registry, and that the metadata is properly merged.
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

        let unified = UnifiedNodeRegistry::new(
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

    /// Verify that a node is fetched from the local source even if it exists in one or more
    /// read-only registries, and that the metadata is properly merged.
    ///
    /// 1. Add the same node to the local registry and two read-only registries with different
    ///    endpoints and metadata.
    /// 2. Add the registries to a unified registry.
    /// 3. Fetch the node and verify that it has the correct data (endpoints from local registry,
    ///    metadata merged from all sources).
    #[test]
    fn fetch_node_local_precedence() {
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

        let unified = UnifiedNodeRegistry::new(
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
    /// 1. Add the same node to the local registry and a read-only registry with different data.
    /// 2. Add the same node to two read-only registries with different data.
    /// 3. Add all three registries to a unified registry.
    /// 4. List the nodes and verify that the correct node data is returned.
    #[test]
    fn list_nodes_precedence() {
        let node1_local = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
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
            .insert_node(node1_local)
            .expect("Unable to insert local node1");

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

        let unified = UnifiedNodeRegistry::new(
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

        let unified = UnifiedNodeRegistry::new(Box::new(writeable), vec![Box::new(readable)]);

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

    /// Verify that the `NodeRegistryWriter` implementation affects only the local registry.
    #[test]
    fn write_nodes() {
        let node1 = new_node("node1", "endpoint1", &[("meta_a", "val_a")]);
        let node2 = new_node("node2", "endpoint2", &[("meta_b", "val_b")]);

        let writeable = MemRegistry::default();

        let readable = MemRegistry::default();
        readable
            .insert_node(node2.clone())
            .expect("Unable to insert node2 into read-only registry");

        let unified = UnifiedNodeRegistry::new(
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
        nodes: Arc<Mutex<BTreeMap<String, Node>>>,
    }

    impl NodeRegistryReader for MemRegistry {
        fn list_nodes<'a, 'b: 'a>(
            &'b self,
            predicates: &'a [MetadataPredicate],
        ) -> Result<Box<dyn Iterator<Item = Node> + Send + 'a>, NodeRegistryError> {
            Ok(Box::new(SnapShotIter {
                snapshot: self
                    .nodes
                    .lock()
                    .expect("mem registry lock was poisoned")
                    .iter()
                    .map(|(_, node)| node)
                    .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node)))
                    .cloned()
                    .collect(),
            }))
        }

        fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError> {
            self.list_nodes(predicates).map(|iter| iter.count() as u32)
        }

        fn fetch_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .get(identity)
                .cloned())
        }
    }

    impl NodeRegistryWriter for MemRegistry {
        fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError> {
            self.nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .insert(node.identity.clone(), node);
            Ok(())
        }

        fn delete_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
            Ok(self
                .nodes
                .lock()
                .expect("mem registry lock was poisoned")
                .remove(identity))
        }
    }

    impl RwNodeRegistry for MemRegistry {
        fn clone_box(&self) -> Box<dyn RwNodeRegistry> {
            Box::new(self.clone())
        }
    }

    struct SnapShotIter<V: Send + Clone> {
        snapshot: std::collections::VecDeque<V>,
    }

    impl<V: Send + Clone> Iterator for SnapShotIter<V> {
        type Item = V;

        fn next(&mut self) -> Option<Self::Item> {
            self.snapshot.pop_front()
        }
    }
}
