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

//! A local, read/write registry.
//!
//! This module contains the [`LocalYamlRegistry`], which provides an implementation of the
//! [`RwRegistry`] trait.
//!
//! [`LocalYamlRegistry`]: struct.LocalYamlRegistry.html
//! [`RwRegistry`]: ../../trait.RwRegistry.html

use std::convert::TryFrom;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::registry::{
    error::InvalidNodeError, validate_nodes, MetadataPredicate, Node, NodeIter, RegistryError,
    RegistryReader, RegistryWriter, RwRegistry,
};

use crate::error::{InternalError, InvalidStateError};

use super::YamlNode;

/// A local, read/write registry.
///
/// The `LocalYamlRegistry` provides access to and modification of a local registry YAML file. The
/// local registry file must be a YAML sequence of nodes, where each node is valid (see [`Node`] for
/// validity criteria).
///
/// The contents of the YAML file are cached in-memory by the registry; this means that the registry
/// will continue to be available even if the backing YAML file becomes unavailable. Each time the
/// registry is read, it will check the backing file for any changes since the last read and
/// refresh the internal cache if necessary.
///
/// On initializaion, the registry will check if its backing file already exists. If the backing
/// file already exists, the registry will attempt to load, parse, and validate it. If the backing
/// file does not already exist, the registry will attempt to create it.
///
/// [`Node`]: struct.Node.html
#[derive(Clone)]
pub struct LocalYamlRegistry {
    internal: Arc<Mutex<Internal>>,
}

impl LocalYamlRegistry {
    /// Construct a new `LocalYamlRegistry`. If the backing file already exists, it will be
    /// loaded, parsed, and validated; if any of these steps fails, the error will be returned. If
    /// the backing file doesn't already exist, it will be created and initialized; if file creation
    /// fails, the error will be returned.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the backing YAML file.
    pub fn new(file_path: &str) -> Result<LocalYamlRegistry, RegistryError> {
        Ok(LocalYamlRegistry {
            internal: Arc::new(Mutex::new(Internal::new(file_path)?)),
        })
    }

    /// Get all nodes in the registry.
    pub(super) fn get_nodes(&self) -> Result<Vec<Node>, RegistryError> {
        Ok(self
            .internal
            .lock()
            .map_err(|_| {
                RegistryError::InternalError(InternalError::with_message(
                    "YAML registry's internal lock poisoned".into(),
                ))
            })?
            .get_nodes())
    }

    /// Write the given list of nodes to the backing YAML file.
    pub(super) fn write_nodes(&self, nodes: Vec<Node>) -> Result<(), RegistryError> {
        self.internal
            .lock()
            .map_err(|_| {
                RegistryError::InternalError(InternalError::with_message(
                    "YAML registry's internal lock poisoned".into(),
                ))
            })?
            .write_nodes(nodes)
    }
}

impl RegistryReader for LocalYamlRegistry {
    fn get_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        Ok(self
            .get_nodes()?
            .iter()
            .find(|node| node.identity == identity)
            .cloned())
    }

    fn list_nodes<'a, 'b: 'a>(
        &'b self,
        predicates: &'a [MetadataPredicate],
    ) -> Result<NodeIter<'a>, RegistryError> {
        let mut nodes = self.get_nodes()?;
        nodes.retain(|node| predicates.iter().all(|predicate| predicate.apply(node)));
        Ok(Box::new(nodes.into_iter()))
    }

    fn count_nodes(&self, predicates: &[MetadataPredicate]) -> Result<u32, RegistryError> {
        Ok(self
            .get_nodes()?
            .iter()
            .filter(move |node| predicates.iter().all(|predicate| predicate.apply(node)))
            .count() as u32)
    }

    fn has_node(&self, identity: &str) -> Result<bool, RegistryError> {
        Ok(self
            .get_nodes()?
            .iter()
            .any(|node| node.identity == identity))
    }
}

impl RegistryWriter for LocalYamlRegistry {
    fn add_node(&self, node: Node) -> Result<(), RegistryError> {
        let mut nodes = self.get_nodes()?;
        nodes.push(node);
        self.write_nodes(nodes)
    }

    fn update_node(&self, node: Node) -> Result<(), RegistryError> {
        let mut nodes = self.get_nodes()?;
        if nodes.iter().any(|n| n.identity == node.identity) {
            nodes.retain(|existing_node| existing_node.identity != node.identity);
            nodes.push(node);
            self.write_nodes(nodes)
        } else {
            Err(RegistryError::InvalidStateError(
                InvalidStateError::with_message(format!(
                    "Node does not exist in the registry: {}",
                    node.identity
                )),
            ))
        }
    }

    fn delete_node(&self, identity: &str) -> Result<Option<Node>, RegistryError> {
        let mut nodes = self.get_nodes()?;
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

impl RwRegistry for LocalYamlRegistry {
    fn clone_box(&self) -> Box<dyn RwRegistry> {
        Box::new(self.clone())
    }

    fn clone_box_as_reader(&self) -> Box<dyn RegistryReader> {
        Box::new(Clone::clone(self))
    }

    fn clone_box_as_writer(&self) -> Box<dyn RegistryWriter> {
        Box::new(Clone::clone(self))
    }
}

/// Internal state of the registry
struct Internal {
    file_path: String,
    cached_nodes: Vec<Node>,
    last_read: SystemTime,
}

impl Internal {
    fn new(file_path: &str) -> Result<Self, RegistryError> {
        let mut internal = Self {
            file_path: file_path.into(),
            cached_nodes: vec![],
            last_read: SystemTime::UNIX_EPOCH,
        };

        // If file already exists, read it; otherwise initialize it.
        if PathBuf::from(file_path).is_file() {
            internal.read_nodes()?;
        } else {
            internal.write_nodes(vec![])?;
        }

        Ok(internal)
    }

    /// Get the internal list of nodes. If the backing file has been modified since the last read,
    /// attempt to refresh the cache.
    fn get_nodes(&mut self) -> Vec<Node> {
        let file_read_result = std::fs::metadata(&self.file_path)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| {
                RegistryError::InternalError(InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to read YAML registry file's last modification time".into(),
                ))
            })
            .and_then(|last_modified| {
                if last_modified > self.last_read {
                    self.read_nodes()
                } else {
                    Ok(())
                }
            });

        // Log any errors that occurred with checking or reading the backing file and use the
        // in-memory cache.
        if let Err(err) = file_read_result {
            warn!(
                "Using cached nodes; failed to read from YAML registry file: {}",
                err
            );
        }

        self.cached_nodes.clone()
    }

    /// Read the backing file, verify that it's valid, and cache its contents.
    fn read_nodes(&mut self) -> Result<(), RegistryError> {
        let mut file = File::open(&self.file_path).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                "Failed to open YAML registry file".into(),
            ))
        })?;

        let mut buffer = vec![];
        file.read_to_end(&mut buffer).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                format!("Failed to read YAML registry file {}", self.file_path),
            ))
        })?;

        let nodes = if buffer.is_empty() {
            vec![]
        } else {
            let yaml_nodes: Option<Vec<YamlNode>> =
                serde_yaml::from_slice(&buffer).map_err(|err| {
                    RegistryError::InternalError(InternalError::from_source_with_message(
                        Box::new(err),
                        "Failed to read YAML registry file".into(),
                    ))
                })?;

            let nodes: Vec<Node> = yaml_nodes
                .unwrap_or_default()
                .into_iter()
                .map(Node::try_from)
                .collect::<Result<Vec<Node>, InvalidNodeError>>()
                .map_err(|err| {
                    RegistryError::InvalidStateError(InvalidStateError::with_message(format!(
                        "Unable to get node list: {}",
                        err
                    )))
                })?;

            validate_nodes(&nodes).map_err(|err| {
                RegistryError::InvalidStateError(InvalidStateError::with_message(err.to_string()))
            })?;

            nodes
        };

        self.cached_nodes = nodes;
        self.last_read = SystemTime::now();

        Ok(())
    }

    /// Verify that the given nodes represent a valid registry, write them to the backing file, and
    /// update the in-memory cache.
    fn write_nodes(&mut self, nodes: Vec<Node>) -> Result<(), RegistryError> {
        validate_nodes(&nodes).map_err(|err| {
            RegistryError::InvalidStateError(InvalidStateError::with_message(err.to_string()))
        })?;

        let yaml_nodes: Vec<YamlNode> = nodes
            .iter()
            .map(|node| YamlNode::from(node.clone()))
            .collect::<Vec<YamlNode>>();

        let output = serde_yaml::to_vec(&yaml_nodes).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                "Failed to write nodes to YAML".into(),
            ))
        })?;

        let mut file = File::create(&self.file_path).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                format!("Failed to open YAML registry file '{}'", self.file_path),
            ))
        })?;

        file.write_all(&output).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                format!("Failed to write to YAML registry file '{}'", self.file_path),
            ))
        })?;

        // Append newline to file
        writeln!(file).map_err(|err| {
            RegistryError::InternalError(InternalError::from_source_with_message(
                Box::new(err),
                format!("Failed to write to YAML registry file '{}'", self.file_path),
            ))
        })?;

        self.cached_nodes = nodes;
        self.last_read = SystemTime::now();

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::fs::{remove_file, File};

    use tempfile::Builder;

    ///
    /// Verifies that reading from an empty YAML file still successfully returns an empty result
    #[test]
    fn test_read_empty_yaml_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_duplicate_identity_error")
            .tempdir()?;

        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        File::create(&path)?;

        let registry = LocalYamlRegistry::new(&path)?;
        assert!(registry.get_nodes()?.is_empty());

        Ok(())
    }

    ///
    /// Verifies that reading from a YAML file that contains two nodes with the same identity
    /// returns InvalidNodeError::DuplicateIdentity.
    ///
    #[test]
    fn test_read_yaml_duplicate_identity_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_duplicate_identity_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let node1 = get_node_1();
        let mut node2 = get_node_2();
        node2.identity = node1.identity.clone();

        write_to_file(&vec![node1.clone(), node2], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Two nodes with same identity in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains two nodes with the same endpoint
    /// returns InvalidNodeError::DuplicateEndpoint.
    ///
    #[test]
    fn test_read_yaml_duplicate_endpoint_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_duplicate_endpoint_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let node1 = get_node_1();
        let mut node2 = get_node_2();
        node2.endpoints = node1.endpoints.clone();

        write_to_file(&vec![node1.clone(), node2], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Two nodes with same endpoint in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string as its
    /// identity returns InvalidNodeError::EmptyIdentity.
    ///
    #[test]
    fn test_read_yaml_empty_identity_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_empty_identity_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.identity = "".to_string();

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with empty identity in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string in its
    /// endpoints returns InvalidNodeError::EmptyEndpoint.
    ///
    #[test]
    fn test_read_yaml_empty_endpoint_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_empty_endpoint_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.endpoints = vec!["".to_string()];

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with empty endpoint in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string as its
    /// display_name returns InvalidNodeError::EmptyDisplayName.
    ///
    #[test]
    fn test_read_yaml_empty_display_name_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_empty_display_name_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.display_name = "".to_string();

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with empty display_name in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with an empty string in its
    /// keys returns InvalidNodeError::EmptyKey.
    ///
    #[test]
    fn test_read_yaml_empty_key_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_empty_key_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.keys = vec!["".to_string()];

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with empty key in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with no endpoints returns
    /// InvalidNodeError::MissingEndpoints.
    ///
    #[test]
    fn test_read_yaml_missing_endpoints_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_missing_endpoints_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.endpoints = vec![];

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with no endpoint in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that reading from a YAML file that contains a node with no keys returns
    /// InvalidNodeError::MissingKeys.
    ///
    #[test]
    fn test_read_yaml_missing_keys_error() {
        let temp_dir = Builder::new()
            .prefix("test_read_yaml_missing_keys_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        node.keys = vec![];

        write_to_file(&vec![node], &path);

        let result = LocalYamlRegistry::new(&path);
        match result {
            Ok(_) => panic!("Node with no keys in YAML file. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => (),
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that get_node with a valid identity, returns the correct node.
    ///
    #[test]
    fn test_get_node_ok() {
        let temp_dir = Builder::new()
            .prefix("test_get_node_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let node = registry
            .get_node(&get_node_1().identity)
            .expect("Failed to fetch node")
            .expect("Node not found");
        assert_eq!(node, get_node_1());
    }

    ///
    /// Verifies that get_node with an invalid identity returns Ok(None)
    ///
    #[test]
    fn test_get_node_not_found() {
        let temp_dir = Builder::new()
            .prefix("test_get_node_not_found")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let result = registry.get_node("NodeNotInRegistry");
        match result {
            Ok(None) => {}
            res => panic!("Should have gotten Ok(None) but got {:?}", res),
        }
    }

    ///
    /// Verifies that `has_node` properly determines if a node exists in the registry.
    ///
    #[test]
    fn test_has_node() {
        let temp_dir = Builder::new()
            .prefix("test_has_node")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        assert!(registry
            .has_node(&get_node_1().identity)
            .expect("Failed to check if node1 exists"));
        assert!(!registry
            .has_node(&get_node_2().identity)
            .expect("Failed to check if node2 exists"));
    }

    ///
    /// Verifies that list_nodes returns a list of nodes.
    ///
    #[test]
    fn test_list_nodes_ok() {
        let temp_dir = Builder::new()
            .prefix("test_list_nodes_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0], get_node_1());
        assert_eq!(nodes[1], get_node_2());
    }

    ///
    /// Verifies that list_nodes returns an empty list when there are no nodes in the registry.
    ///
    #[test]
    fn test_list_nodes_empty_ok() {
        let temp_dir = Builder::new()
            .prefix("test_list_nodes_empty_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();
        assert_eq!(nodes.len(), 0);
    }

    ///
    /// Verifies that list_nodes returns the correct items when there is a filter by metadata.
    ///
    #[test]
    fn test_list_nodes_filter_metadata_ok() {
        let temp_dir = Builder::new()
            .prefix("test_list_nodes_filter_metadata_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

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

    ///
    /// Verifies that list_nodes returns the correct items when there is more than one filter.
    ///
    #[test]
    fn test_list_nodes_filter_multiple_ok() {
        let temp_dir = Builder::new()
            .prefix("test_list_nodes_filter_multiple_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2(), get_node_3()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

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
    ///
    ///
    /// Verifies that list_nodes returns an empty list when no nodes fits the filtering criteria.
    ///
    #[test]
    fn test_list_nodes_filter_empty_ok() {
        let temp_dir = Builder::new()
            .prefix("test_list_nodes_filter_empty_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

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

    ///
    /// Verifies that add_node returns InvalidNodeError::DuplicateEndpoint when a node
    /// with the same endpoint already exists in the yaml file.
    ///
    #[test]
    fn test_add_node_duplicate_endpoint_error() {
        let temp_dir = Builder::new()
            .prefix("test_xv_node_duplicate_endpoint_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let node1 = get_node_1();

        write_to_file(&vec![node1.clone()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_2();
        node.endpoints = node1.endpoints.clone();
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node with endpoint already exists. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::EmptyIdentity when a node with
    /// an empty string as its identity is added to the registry.
    ///
    #[test]
    fn test_add_node_empty_identity_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_empty_identity_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.identity = "".to_string();
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node identity is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::EmptyEndpoint when a node with
    /// an empty string in its endpoints is added to the registry.
    ///
    #[test]
    fn test_add_node_empty_endpoint_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_empty_endpoint_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.endpoints = vec!["".to_string()];
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node endpoint is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::EmptyDisplayName when a node
    /// with an empty string as its display_name is added to the registry.
    ///
    #[test]
    fn test_add_node_empty_display_name_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_missing_endpoints_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.display_name = "".to_string();
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node display_name is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::EmptyKey when a node with
    /// an empty string in its keys is added to the registry.
    ///
    #[test]
    fn test_add_node_empty_key_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_missing_endpoints_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.keys = vec!["".to_string()];
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node key is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::MissingEndpoints when a node with no
    /// endpoints is added to the registry.
    ///
    #[test]
    fn test_add_node_missing_endpoints_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_missing_endpoints_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.endpoints = vec![];
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node endpoints is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that add_node returns InvalidNodeError::MissingKeys when a node with no
    /// keys is added to the registry.
    ///
    #[test]
    fn test_add_node_missing_keys_error() {
        let temp_dir = Builder::new()
            .prefix("test_add_node_missing_endpoints_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_1();
        node.keys = vec![];
        let result = registry.add_node(node);

        match result {
            Ok(_) => panic!("Node keys is empty. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that update_node successfully updates an existing node in the yaml file.
    ///
    #[test]
    fn test_update_node_ok() {
        let temp_dir = Builder::new()
            .prefix("test_update_node_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let mut node = get_node_1();
        write_to_file(&vec![node.clone()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        node.metadata
            .insert("location".to_string(), "Minneapolis".to_string());

        #[allow(deprecated)]
        registry
            .update_node(node.clone())
            .expect("Failed to insert node");

        let nodes = registry
            .list_nodes(&[])
            .expect("Failed to retrieve nodes")
            .collect::<Vec<_>>();

        assert_eq!(nodes, vec![node]);
    }

    ///
    /// Verifies that update_node returns InvalidNodeError::DuplicateEndpoint when a node
    /// with the same endpoint already exists in the yaml file.
    ///
    #[test]
    fn test_update_node_duplicate_endpoint_error() {
        let temp_dir = Builder::new()
            .prefix("test_xv_node_duplicate_endpoint_error")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let node1 = get_node_1();
        let node2 = get_node_2();

        write_to_file(&vec![node1.clone(), node2.clone()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let mut node = get_node_2();
        node.endpoints = node1.endpoints.clone();
        let result = registry.update_node(node);

        match result {
            Ok(_) => panic!("Node with endpoint already exists. Error should be returned"),
            Err(RegistryError::InvalidStateError(_)) => {}
            Err(err) => panic!(
                "Should have gotten RegistryError::InvalidStateError but got {}",
                err
            ),
        }
    }

    ///
    /// Verifies that delete_node with a valid identity deletes the correct node and returns it.
    ///
    #[test]
    fn test_delete_node_ok() {
        let temp_dir = Builder::new()
            .prefix("test_delete_node_ok")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

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
    }

    ///
    /// Verifies that delete_node with an invalid identity returns Ok(None)
    ///
    #[test]
    fn test_delete_node_not_found() {
        let temp_dir = Builder::new()
            .prefix("test_delete_node_not_found")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&vec![get_node_1(), get_node_2()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let result = registry.delete_node("NodeNotInRegistry");
        match result {
            Ok(None) => {}
            res => panic!("Should have gotten Ok(None) but got {:?}", res),
        }
    }

    ///
    /// Verifies that if the YAML file does not exist on initialization, `LocalYamlRegistry` will
    /// create and initialize it as an empty registry.
    ///
    #[test]
    fn test_create_file() {
        let temp_dir = Builder::new()
            .prefix("test_create_file")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        // Verify that the internal cache is empty
        assert!(registry
            .get_nodes()
            .expect("Failed to get nodes")
            .is_empty());

        // Verify that the file exists and is empty
        let file = File::open(&path).expect("Failed to open file");
        let file_contents: Vec<YamlNode> =
            serde_yaml::from_reader(file).expect("Failed to deserialize file");
        assert!(file_contents.is_empty());
    }

    ///
    /// Verifies that if the YAML file is modified directly, it will be reloaded on the next read.
    ///
    #[test]
    fn test_reload_modified_file() {
        let temp_dir = Builder::new()
            .prefix("test_reload_modified_file")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&[], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        assert!(registry
            .get_nodes()
            .expect("Failed to get nodes from original file")
            .is_empty());

        // Allow some time before writing the file to make sure the read time is earlier than the
        // write time; the system clock may not be very precise.
        std::thread::sleep(std::time::Duration::from_secs(1));

        write_to_file(&[get_node_1()], &path);

        let nodes = registry
            .get_nodes()
            .expect("Failed to get nodes from updated file");
        assert_eq!(nodes, vec![get_node_1()]);
    }

    ///
    /// Verifies that if the YAML file is removed, the registry will still return nodes using its
    /// in-memory cache.
    ///
    #[test]
    fn test_file_removed() {
        let temp_dir = Builder::new()
            .prefix("test_file_removed")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("registry.yaml")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        write_to_file(&[get_node_1()], &path);

        let registry = LocalYamlRegistry::new(&path).expect("Failed to create LocalYamlRegistry");

        let nodes = registry.get_nodes().expect("Failed to get nodes with file");
        assert_eq!(nodes, vec![get_node_1()]);

        remove_file(&path).expect("Failed to remove file");

        let nodes = registry
            .get_nodes()
            .expect("Failed to get nodes without file");
        assert_eq!(nodes, vec![get_node_1()]);
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

    fn write_to_file(data: &[Node], file_path: &str) {
        let yaml_data: Vec<YamlNode> = data
            .iter()
            .map(|node| YamlNode::from(node.clone()))
            .collect();
        let file = File::create(file_path).expect("Error creating test yaml file.");
        serde_yaml::to_writer(file, &yaml_data).expect("Error writing nodes to file.");
    }
}
