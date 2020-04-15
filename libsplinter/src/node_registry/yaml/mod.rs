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

use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::{
    InvalidNodeError, MetadataPredicate, Node, NodeRegistryError, NodeRegistryReader,
    NodeRegistryWriter, RwNodeRegistry,
};

#[derive(Clone)]
pub struct YamlNodeRegistry {
    file_internal: Arc<Mutex<FileInternal>>,
}

pub struct FileInternal {
    pub file_path: String,
    pub cached_nodes: Vec<Node>,
}

impl YamlNodeRegistry {
    pub fn new(file_path: &str) -> Result<YamlNodeRegistry, NodeRegistryError> {
        // If file already exists, read and verify its contents; otherwise create it and initialize
        // the nodes list.
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

            for (idx, node) in cached_nodes.iter().enumerate() {
                check_node_required_fields_are_not_empty(node)?;
                check_if_node_is_duplicate(node, &cached_nodes[idx + 1..])?;
            }

            Ok(YamlNodeRegistry {
                file_internal: Arc::new(Mutex::new(FileInternal {
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

            let registry = YamlNodeRegistry {
                file_internal: Arc::new(Mutex::new(FileInternal {
                    file_path: file_path.into(),
                    cached_nodes: vec![],
                })),
            };

            registry.write_nodes(&[])?;

            Ok(registry)
        }
    }

    fn get_cached_nodes(&self) -> Result<Vec<Node>, NodeRegistryError> {
        let file_backend = self.file_internal.lock().map_err(|_| {
            NodeRegistryError::general_error("YAML registry's internal lock poisoned")
        })?;
        Ok(file_backend.cached_nodes.clone())
    }

    fn write_nodes(&self, data: &[Node]) -> Result<(), NodeRegistryError> {
        let mut file_backend = self.file_internal.lock().map_err(|_| {
            NodeRegistryError::general_error("YAML registry's internal lock poisoned")
        })?;
        let output = serde_yaml::to_vec(&data).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                "Failed to write nodes to YAML",
                Box::new(err),
            )
        })?;
        std::fs::write(&file_backend.file_path, &output).map_err(|err| {
            NodeRegistryError::general_error_with_source(
                &format!(
                    "Failed to write to YAML registry file '{}'",
                    file_backend.file_path
                ),
                Box::new(err),
            )
        })?;
        file_backend.cached_nodes = data.to_vec();
        Ok(())
    }
}

impl NodeRegistryReader for YamlNodeRegistry {
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
    ) -> Result<Box<dyn Iterator<Item = Node> + Send + 'a>, NodeRegistryError> {
        let nodes = self.get_cached_nodes()?;

        Ok(Box::new(nodes.into_iter().filter(move |node| {
            predicates.iter().all(|predicate| predicate.apply(node))
        })))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, NodeRegistryError> {
        let nodes = self.get_cached_nodes()?;

        Ok(nodes
            .iter()
            .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node)))
            .count() as u32)
    }
}

impl NodeRegistryWriter for YamlNodeRegistry {
    fn insert_node(&self, node: Node) -> Result<(), NodeRegistryError> {
        let mut nodes = self.get_cached_nodes()?;

        check_node_required_fields_are_not_empty(&node)?;

        // If a node with the same identity already exists, remove it
        nodes.retain(|existing_node| existing_node.identity != node.identity);

        check_if_node_is_duplicate(&node, &nodes)?;

        nodes.push(node);

        self.write_nodes(&nodes)
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

        self.write_nodes(&nodes)?;

        Ok(opt)
    }
}

impl RwNodeRegistry for YamlNodeRegistry {
    fn clone_box(&self) -> Box<dyn RwNodeRegistry> {
        Box::new(Clone::clone(self))
    }
}

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
mod test {
    use super::*;

    use std::collections::HashMap;
    use std::env;
    use std::fs::{remove_file, File};
    use std::panic;
    use std::thread;

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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let result = YamlNodeRegistry::new(test_yaml_file_path);
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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

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

            let registry = YamlNodeRegistry::new(test_yaml_file_path)
                .expect("Failed to create YamlNodeRegistry");

            let result = registry.delete_node("NodeNotInRegistry");
            match result {
                Ok(None) => {}
                res => panic!("Should have gotten Ok(None) but got {:?}", res),
            }
        })
    }

    fn get_node_1() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("company".to_string(), "Bitwise IO".to_string());
        metadata.insert("admin".to_string(), "Bob".to_string());
        Node {
            identity: "Node-123".to_string(),
            endpoints: vec!["tcps://12.0.0.123:8431".to_string()],
            display_name: "Bitwise IO - Node 1".to_string(),
            metadata,
        }
    }

    fn get_node_2() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("company".to_string(), "Cargill".to_string());
        metadata.insert("admin".to_string(), "Carol".to_string());
        Node {
            identity: "Node-456".to_string(),
            endpoints: vec!["tcps://12.0.0.123:8434".to_string()],
            display_name: "Cargill - Node 1".to_string(),
            metadata,
        }
    }

    fn get_node_3() -> Node {
        let mut metadata = HashMap::new();
        metadata.insert("company".to_string(), "Cargill".to_string());
        metadata.insert("admin".to_string(), "Charlie".to_string());
        Node {
            identity: "Node-789".to_string(),
            endpoints: vec!["tcps://12.0.0.123:8435".to_string()],
            display_name: "Cargill - Node 2".to_string(),
            metadata,
        }
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
