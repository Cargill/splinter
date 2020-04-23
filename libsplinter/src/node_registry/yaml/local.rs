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

//! A local, read/write node registry.
//!
//! This module contains the [`LocalYamlNodeRegistry`], which provides an implementation of the
//! [`RwNodeRegistry`] trait.
//!
//! [`LocalYamlNodeRegistry`]: struct.LocalYamlNodeRegistry.html
//! [`RwNodeRegistry`]: ../../trait.RwNodeRegistry.html

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::node_registry::{
    check_if_node_is_duplicate, check_node_required_fields_are_not_empty, validate_nodes,
    MetadataPredicate, Node, NodeIter, NodeRegistryError, NodeRegistryReader, NodeRegistryWriter,
    RwNodeRegistry,
};

/// A local, read/write node registry.
///
/// The `LocalYamlNodeRegistry` provides access to and modification of a local node registry YAML
/// file. The local registry file must be a YAML sequence of nodes, where each node is valid (see
/// [`Node`] for validity criteria).
///
/// The contents of the YAML file are cached in-memory by the registry so that a disk operation is
/// not necessary when reading from the registry. This means that the YAML file is only read on
/// initialization, so it should not be modified directly while in use by the
/// `LocalYamlNodeRegistry`.
///
/// On initializaion, the registry will check if its backing file already exists. If the backing
/// file already exists, the registry will attempt to load, parse, and validate it. If the backing
/// file does not already exist, the registry will attempt to create it.
///
/// [`Node`]: struct.Node.html
#[derive(Clone)]
pub struct LocalYamlNodeRegistry {
    internal: Arc<Mutex<Internal>>,
}

/// Internal state of the registry
struct Internal {
    file_path: String,
    cached_nodes: Vec<Node>,
}

impl LocalYamlNodeRegistry {
    /// Construct a new `LocalYamlNodeRegistry`. If the backing file already exists, it will be
    /// loaded, parsed, and validated; if any of these steps fails, the error will be returned. If
    /// the backing file doesn't already exist, it will be created and initialized; if file creation
    /// fails, the error will be returned.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the backing YAML file.
    pub fn new(file_path: &str) -> Result<LocalYamlNodeRegistry, NodeRegistryError> {
        // If file already exists, read, verify and cache its contents; otherwise create it and
        // initialize the nodes list.
        if PathBuf::from(file_path).is_file() {
            let file = File::open(file_path).map_err(|err| {
                NodeRegistryError::general_error_with_source(
                    "Failed to open YAML registry file",
                    Box::new(err),
                )
            })?;
            let cached_nodes: Vec<Node> = serde_yaml::from_reader(&file).map_err(|err| {
                NodeRegistryError::general_error_with_source(
                    "Failed to read YAML registry file",
                    Box::new(err),
                )
            })?;

            validate_nodes(&cached_nodes)?;

            Ok(LocalYamlNodeRegistry {
                internal: Arc::new(Mutex::new(Internal {
                    file_path: file_path.into(),
                    cached_nodes,
                })),
            })
        } else {
            File::create(file_path).map_err(|err| {
                NodeRegistryError::general_error_with_source(
                    "Failed to create YAML registry file",
                    Box::new(err),
                )
            })?;

            let registry = LocalYamlNodeRegistry {
                internal: Arc::new(Mutex::new(Internal {
                    file_path: file_path.into(),
                    cached_nodes: vec![],
                })),
            };

            registry.write_nodes(vec![])?;

            Ok(registry)
        }
    }

    /// Get the list of nodes from the in-memory cache.
    pub(super) fn get_cached_nodes(&self) -> Result<Vec<Node>, NodeRegistryError> {
        Ok(self
            .internal
            .lock()
            .map_err(|_| {
                NodeRegistryError::general_error("YAML registry's internal lock poisoned")
            })?
            .cached_nodes
            .clone())
    }

    /// Write the given list of nodes to the backing YAML file.
    pub(super) fn write_nodes(&self, nodes: Vec<Node>) -> Result<(), NodeRegistryError> {
        let mut file_backend = self.internal.lock().map_err(|_| {
            NodeRegistryError::general_error("YAML registry's internal lock poisoned")
        })?;
        let output = serde_yaml::to_vec(&nodes).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                "Failed to write nodes to YAML",
                Box::new(err),
            )
        })?;

        let mut file = File::create(&file_backend.file_path).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                &format!(
                    "Failed to open YAML registry file '{}'",
                    file_backend.file_path
                ),
                Box::new(err),
            )
        })?;
        file.write_all(&output).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                &format!(
                    "Failed to write to YAML registry file '{}'",
                    file_backend.file_path
                ),
                Box::new(err),
            )
        })?;
        // Append newline to file
        writeln!(file).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                &format!(
                    "Failed to write to YAML registry file '{}'",
                    file_backend.file_path
                ),
                Box::new(err),
            )
        })?;

        file_backend.cached_nodes = nodes;
        Ok(())
    }
}

impl NodeRegistryReader for LocalYamlNodeRegistry {
    fn fetch_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
        Ok(self
            .get_cached_nodes()?
            .iter()
            .find(|node| node.identity == identity)
            .cloned())
    }

    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, NodeRegistryError> {
        let mut nodes = self.get_cached_nodes()?;
        nodes.retain(|node| predicates.iter().all(|predicate| predicate.apply(node)));
        Ok(Box::new(nodes.into_iter()))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError> {
        Ok(self
            .get_cached_nodes()?
            .iter()
            .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node)))
            .count() as u32)
    }
}

impl NodeRegistryWriter for LocalYamlNodeRegistry {
    fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError> {
        let mut nodes = self.get_cached_nodes()?;

        check_node_required_fields_are_not_empty(&node)?;

        // If a node with the same identity already exists, remove it
        nodes.retain(|existing_node| existing_node.identity != node.identity);

        check_if_node_is_duplicate(&node, &nodes)?;

        nodes.push(node);

        self.write_nodes(nodes)
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, NodeRegistryError> {
        let mut nodes = self.get_cached_nodes()?;
        let mut index = None;
        for (i, node) in nodes.iter().enumerate() {
            if node.identity == identity {
                index = Some(i);
                break;
            }
        }
        let opt = index.map(|i| nodes.remove(i));

        self.write_nodes(nodes)?;

        Ok(opt)
    }
}

impl RwNodeRegistry for LocalYamlNodeRegistry {
    fn clone_box(&self) -> Box<dyn RwNodeRegistry> {
        Box::new(self.clone())
    }

    fn clone_box_as_reader(&self) -> Box<dyn NodeRegistryReader> {
        Box::new(Clone::clone(self))
    }

    fn clone_box_as_writer(&self) -> Box<dyn NodeRegistryWriter> {
        Box::new(Clone::clone(self))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::env;
    use std::fs::{remove_file, File};
    use std::panic;
    use std::thread;

    use crate::node_registry::{InvalidNodeError, NodeBuilder};

    ///
    /// Verifies that reading from a YAML file that contains two nodes with the same identity
    /// returns InvalidNodeError::DuplicateIdentity.
    ///
    #[test]
    fn test_read_yaml_duplicate_identity_error() {
        run_test(|test_yaml_file_path| {
            let node1 = get_node_1();
            let mut node2 = get_node_2();
            node2.identity = node1.identity.clone();

            write_to_file(&vec![node1.clone(), node2], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => {
                    panic!("Two nodes with same identity in YAML file. Error should be returned")
                }
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::DuplicateIdentity(id))) => {
                    assert_eq!(id, node1.identity)
                }
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::DuplicateIdentity but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that reading from a YAML file that contains two nodes with the same endpoint
    /// returns InvalidNodeError::DuplicateEndpoint.
    ///
    #[test]
    fn test_read_yaml_duplicate_endpoint_error() {
        run_test(|test_yaml_file_path| {
            let node1 = get_node_1();
            let mut node2 = get_node_2();
            node2.endpoints = node1.endpoints.clone();

            write_to_file(&vec![node1.clone(), node2], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => {
                    panic!("Two nodes with same endpoint in YAML file. Error should be returned")
                }
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::DuplicateEndpoint(
                    endpoint,
                ))) => assert!(node1.endpoints.contains(&endpoint)),
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::DuplicateEndpoint but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string as its
    /// identity returns InvalidNodeError::EmptyIdentity.
    ///
    #[test]
    fn test_read_yaml_empty_identity_error() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            node.identity = "".to_string();

            write_to_file(&vec![node], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => panic!("Node with empty identity in YAML file. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyIdentity)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyIdentity but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string in its
    /// endpoints returns InvalidNodeError::EmptyEndpoint.
    ///
    #[test]
    fn test_read_yaml_empty_endpoint_error() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            node.endpoints = vec!["".to_string()];

            write_to_file(&vec![node], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => panic!("Node with empty endpoint in YAML file. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyEndpoint)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyEndpoint but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string as its
    /// display_name returns InvalidNodeError::EmptyDisplayName.
    ///
    #[test]
    fn test_read_yaml_empty_display_name_error() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            node.display_name = "".to_string();

            write_to_file(&vec![node], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => {
                    panic!("Node with empty display_name in YAML file. Error should be returned")
                }
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyDisplayName)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyDisplayName but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with no endpoints returns
    /// InvalidNodeError::MissingEndpoints.
    ///
    #[test]
    fn test_read_yaml_missing_endpoints_error() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            node.endpoints = vec![];

            write_to_file(&vec![node], test_yaml_file_path);

            let result = LocalYamlNodeRegistry::new(test_yaml_file_path);
            match result {
                Ok(_) => panic!("Node with no endpoint in YAML file. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::MissingEndpoints)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::MissingEndpoints but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that fetch_node with a valid identity, returns the correct node.
    ///
    #[test]
    fn test_fetch_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let node = registry
                .fetch_node(&get_node_1().identity)
                .expect("Failed to fetch node")
                .expect("Node not found");
            assert_eq!(node, get_node_1());
        })
    }

    ///
    /// Verifies that fetch_node with an invalid identity returns Ok(None)
    ///
    #[test]
    fn test_fetch_node_not_found() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let result = registry.fetch_node("NodeNotInRegistry");
            match result {
                Ok(None) => {}
                res => panic!("Should have gotten Ok(None) but got {:?}", res),
            }
        })
    }

    ///
    /// Verifies that list_nodes returns a list of nodes.
    ///
    #[test]
    fn test_list_nodes_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let nodes = registry
                .list_nodes(&[])
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();

            assert_eq!(nodes.len(), 2);
            assert_eq!(nodes[0], get_node_1());
            assert_eq!(nodes[1], get_node_2());
        })
    }

    ///
    /// Verifies that list_nodes returns an empty list when there are no nodes in the registry.
    ///
    #[test]
    fn test_list_nodes_empty_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let nodes = registry
                .list_nodes(&[])
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();
            assert_eq!(nodes.len(), 0);
        })
    }

    ///
    /// Verifies that list_nodes returns the correct items when there is a filter by metadata.
    ///
    #[test]
    fn test_list_nodes_filter_metadata_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

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
        })
    }

    ///
    /// Verifies that list_nodes returns the correct items when there is more than one filter.
    ///
    #[test]
    fn test_list_nodes_filter_multiple_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(
                &vec![get_node_1(), get_node_2(), get_node_3()],
                test_yaml_file_path,
            );

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

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
        })
    }
    ///
    ///
    /// Verifies that list_nodes returns an empty list when no nodes fits the filtering criteria.
    ///
    #[test]
    fn test_list_nodes_filter_empty_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let filter = vec![MetadataPredicate::Eq(
                "admin".to_string(),
                get_node_3().metadata.get("admin").unwrap().to_string(),
            )];

            let nodes = registry
                .list_nodes(&filter)
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();

            assert_eq!(nodes.len(), 0);
        })
    }

    ///
    /// Verifies that insert_node successfully adds a new node to the yaml file.
    ///
    #[test]
    fn test_add_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let node = get_node_1();

            registry
                .insert_node(node.clone())
                .expect("Failed to insert node");

            let nodes = registry
                .list_nodes(&[])
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();

            assert_eq!(nodes, vec![node]);
        })
    }

    ///
    /// Verifies that insert_node successfully updates an existing node in the yaml file.
    ///
    #[test]
    fn test_update_node_ok() {
        run_test(|test_yaml_file_path| {
            let mut node = get_node_1();
            write_to_file(&vec![node.clone()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            node.metadata
                .insert("location".to_string(), "Minneapolis".to_string());

            registry
                .insert_node(node.clone())
                .expect("Failed to insert node");

            let nodes = registry
                .list_nodes(&[])
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();

            assert_eq!(nodes, vec![node]);
        })
    }

    ///
    /// Verifies that insert_node returns InvalidNodeError::DuplicateEndpoint when a node
    /// with the same endpoint already exists in the yaml file.
    ///
    #[test]
    fn test_insert_node_duplicate_endpoint_error() {
        run_test(|test_yaml_file_path| {
            let node1 = get_node_1();

            write_to_file(&vec![node1.clone()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let mut node = get_node_2();
            node.endpoints = node1.endpoints.clone();
            let result = registry.insert_node(node);

            match result {
                Ok(_) => panic!("Node with endpoint already exists. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::DuplicateEndpoint(
                    endpoint,
                ))) => assert!(node1.endpoints.contains(&endpoint)),
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::DuplicateEndpoint but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that insert_node returns InvalidNodeError::EmptyIdentity when a node with
    /// an empty string as its identity is added to the registry.
    ///
    #[test]
    fn test_insert_node_empty_identity_error() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let mut node = get_node_1();
            node.identity = "".to_string();
            let result = registry.insert_node(node);

            match result {
                Ok(_) => panic!("Node identity is empty. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyIdentity)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyIdentity but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that insert_node returns InvalidNodeError::EmptyEndpoint when a node with
    /// an empty string in its endpoints is added to the registry.
    ///
    #[test]
    fn test_insert_node_empty_endpoint_error() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let mut node = get_node_1();
            node.endpoints = vec!["".to_string()];
            let result = registry.insert_node(node);

            match result {
                Ok(_) => panic!("Node endpoint is empty. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyEndpoint)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyEndpoint but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that insert_node returns InvalidNodeError::EmptyDisplayName when a node
    /// with an empty string as its display_name is added to the registry.
    ///
    #[test]
    fn test_insert_node_empty_display_name_error() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let mut node = get_node_1();
            node.display_name = "".to_string();
            let result = registry.insert_node(node);

            match result {
                Ok(_) => panic!("Node display_name is empty. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::EmptyDisplayName)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::EmptyDisplayName but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that insert_node returns InvalidNodeError::MissingEndpoints when a node with no
    /// endpoints is added to the registry.
    ///
    #[test]
    fn test_insert_node_missing_endpoints_error() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let mut node = get_node_1();
            node.endpoints = vec![];
            let result = registry.insert_node(node);

            match result {
                Ok(_) => panic!("Node endpoints is empty. Error should be returned"),
                Err(NodeRegistryError::InvalidNode(InvalidNodeError::MissingEndpoints)) => {}
                Err(err) => panic!(
                    "Should have gotten InvalidNodeError::MissingEndpoints but got {}",
                    err
                ),
            }
        })
    }

    ///
    /// Verifies that delete_node with a valid identity deletes the correct node and returns it.
    ///
    #[test]
    fn test_delete_node_ok() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let node = registry
                .delete_node(&get_node_1().identity)
                .expect("Failed to delete node");

            let nodes = registry
                .list_nodes(&[])
                .expect("Failed to retrieve nodes")
                .collect::<Vec<_>>();

            assert_eq!(nodes.len(), 1);

            assert_eq!(nodes[0], get_node_2());

            assert_eq!(node, Some(get_node_1()));
        })
    }

    ///
    /// Verifies that delete_node with an invalid identity returns Ok(None)
    ///
    #[test]
    fn test_delete_node_not_found() {
        run_test(|test_yaml_file_path| {
            write_to_file(&vec![get_node_1(), get_node_2()], test_yaml_file_path);

            let registry = LocalYamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create LocalYamlNodeRegistry");

            let result = registry.delete_node("NodeNotInRegistry");
            match result {
                Ok(None) => {}
                res => panic!("Should have gotten Ok(None) but got {:?}", res),
            }
        })
    }

    fn get_node_1() -> Node {
        NodeBuilder::new("Node-123")
            .with_endpoint("tcps://12.0.0.123:8431")
            .with_display_name("Bitwise IO - Node 1")
            .with_metadata("company", "Bitwise IO")
            .with_metadata("admin", "Bob")
            .build()
            .expect("Failed to build node1")
    }

    fn get_node_2() -> Node {
        NodeBuilder::new("Node-456")
            .with_endpoint("tcps://12.0.0.123:8434")
            .with_display_name("Cargill - Node 1")
            .with_metadata("company", "Cargill")
            .with_metadata("admin", "Carol")
            .build()
            .expect("Failed to build node2")
    }

    fn get_node_3() -> Node {
        NodeBuilder::new("Node-789")
            .with_endpoint("tcps://12.0.0.123:8435")
            .with_display_name("Cargill - Node 2")
            .with_metadata("company", "Cargill")
            .with_metadata("admin", "Charlie")
            .build()
            .expect("Failed to build node3")
    }

    fn write_to_file(data: &[Node], file_path: &str) {
        let file = File::create(file_path).expect("Error creating test nodes yaml file.");
        serde_yaml::to_writer(file, data).expect("Error writing nodes to file.");
    }

    fn run_test<T>(test: T) -> ()
    where
        T: FnOnce(&str) -> () + panic::UnwindSafe,
    {
        let test_yaml_file = temp_yaml_file_path();

        let test_path = test_yaml_file.clone();
        let result = panic::catch_unwind(move || test(&test_path));

        remove_file(test_yaml_file).unwrap();

        assert!(result.is_ok())
    }

    fn temp_yaml_file_path() -> String {
        let mut temp_dir = env::temp_dir();

        let thread_id = thread::current().id();
        temp_dir.push(format!("test_node_registry-{:?}.yaml", thread_id));
        temp_dir.to_str().unwrap().to_string()
    }
}
