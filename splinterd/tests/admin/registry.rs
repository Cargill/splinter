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

//! Integration tests for the registry REST API endpoints

use std::collections::HashMap;

use splinter::registry::client::RegistryNode;
use splinterd::node::RestApiVariant;

use crate::framework::network::Network;

/// Test that a single node in the registry can be fetched and listed.
///
/// 1. Create a network with one node
/// 2. List nodes using the registry client
/// 3. Check list length is 1
/// 4. Check returned node fields match the network node
/// 5. Fetch the node from the registry
/// 6. Check returned node fields match the network node
/// 7. Shutdown
#[test]
pub fn test_registry_creation() {
    // Start a single node network
    let network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(1)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the node
    let node = network.node(0).expect("Unable to get node");
    // Get node's registry client
    let registry_client = node.registry_client();
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 1
    assert_eq!(registry_list_response.data.len(), 1);
    // Check that the fields of the one node in the list are correct
    assert_eq!(registry_list_response.data[0].identity, node.node_id());
    assert_eq!(
        registry_list_response.data[0].endpoints,
        node.network_endpoints()
    );
    assert_eq!(
        registry_list_response.data[0].keys[0],
        node.admin_signer().public_key().unwrap().as_hex()
    );
    // Fetch the network node from the registry
    let registry_get_response = registry_client
        .get_node(node.node_id())
        .expect("Registry get node request failed");
    // Check that the fields of the returned node are correct
    assert_eq!(
        registry_get_response.clone().unwrap().identity,
        node.node_id()
    );
    assert_eq!(
        registry_get_response.clone().unwrap().endpoints,
        node.network_endpoints()
    );
    assert_eq!(
        registry_get_response.unwrap().keys[0],
        node.admin_signer().public_key().unwrap().as_hex()
    );
    // Shutdown the network
    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a valid node can be added to the registry and attempting to an
/// invalid node will return an error.
///
/// 1. Create a network with two nodes
/// 2. List nodes using the registry client
/// 3. Check list length is 2
/// 4. Create a node `node_c` and add it to the registry
/// 5. Retrieve `node_c` from the registry
/// 6. Check that the fields of the returned node match those in `node_c`
/// 7. List all nodes in the registry
/// 8. Check that the returned list length is 3
/// 9. Add `node_c` to the registry again
/// 10. Check that an error is returned because a node with the same endpoint already
///     exists in the registry.
/// 11. Shutdown
#[test]
pub fn test_registry_add_node() {
    // Start a two node network
    let network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the first node
    let node_a = network.node(0).expect("Unable to get node");
    // Get the first node's registry client
    let registry_client = node_a.registry_client();
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 2
    assert_eq!(registry_list_response.data.len(), 2);
    let key = node_a
        .admin_signer()
        .clone_box()
        .public_key()
        .expect("Unable to get signer's public key")
        .as_hex();
    // Create metadata
    let mut metadata = HashMap::new();
    metadata.insert("key1".into(), "value1".into());
    // Create `node_c` to add to registry
    let node_c = RegistryNode {
        identity: "node_c".into(),
        endpoints: vec!["tcp://127.0.0.1:8084".into()],
        display_name: "node_c".into(),
        keys: vec![key.to_string()],
        metadata: metadata.clone(),
    };
    // Add `node_c` to the registry
    registry_client
        .add_node(&node_c)
        .expect("registry add node failed");
    // Get `node_c` from the registry
    let registry_get_response = registry_client
        .get_node("node_c")
        .expect("Registry get node request failed");
    // Check that the fields of the returned node are correct
    assert_eq!(
        registry_get_response.clone().unwrap().identity,
        "node_c".to_string()
    );
    assert_eq!(
        registry_get_response.clone().unwrap().endpoints,
        vec!["tcp://127.0.0.1:8084".to_string()]
    );
    assert_eq!(registry_get_response.unwrap().keys[0], key);
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 3
    assert_eq!(registry_list_response.data.len(), 3);
    // Attempt to add `node_c` to the registry again and check that an error is
    // returned because a node with the same endpoint already exists in the registry
    assert_eq!(registry_client.add_node(&node_c).unwrap_err().reduce_to_string(), "Failed to add node to registry: Invalid node: another node with endpoint tcp://127.0.0.1:8084 exists");
    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a node can be updated in the registry with valid information. Additionally
/// test that a node cannot be updated to have the same endpoint as another node in the
/// registry.
///
/// 1. Create a network with two nodes
/// 2. List nodes using the registry client
/// 3. Check list length is 2
/// 4. Create a node `updated_node_a` which has the same id as `node_a`
/// 5. Update `node_a` in the registry
/// 6. Retrieve `node_a` from the registry
/// 7. Check that the fields `node_a` now contain the updated values
/// 8. Create a node with the same id as `node_a` and the same endpoint as the
///    second node in the network
/// 9. Attempt to update `node_a`
/// 10. Check that an error is returned because a node with the same endpoint already
///     exists in the registry.
/// 11. Shutdown
#[test]
pub fn test_registry_update_node() {
    // Start two node network
    let network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the first node
    let node_a = network.node(0).expect("Unable to get node");
    // Get the node's registry client
    let registry_client = node_a.registry_client();
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 2
    assert_eq!(registry_list_response.data.len(), 2);
    let key = node_a
        .admin_signer()
        .clone_box()
        .public_key()
        .expect("Unable to get signer's public key");
    // Create metadata
    let mut metadata = HashMap::new();
    metadata.insert("key1".into(), "value1".into());
    // Create a node with the same id as node_a
    let updated_node_a = RegistryNode {
        identity: node_a.node_id().to_string(),
        endpoints: vec!["tcp://127.0.0.1:8084".into()],
        display_name: "node_a".into(),
        keys: vec![key.to_string()],
        metadata: metadata.clone(),
    };
    // Update `node_a` in the registry
    registry_client
        .update_node(&updated_node_a)
        .expect("registry add node failed");
    // Get `node_a` from the registry
    let registry_get_response = registry_client
        .get_node(node_a.node_id())
        .expect("Registry get node request failed");
    // Check node id of returned node is correct
    assert_eq!(
        registry_get_response.clone().unwrap().identity,
        node_a.node_id()
    );
    // Check that the `endpoint` field of `node_a` was updated
    assert_eq!(
        registry_get_response.unwrap().endpoints[0],
        "tcp://127.0.0.1:8084".to_string()
    );
    // Create a node with the same id as node_a and the same endpoint as the second node
    // in the network
    let update_node_a_invalid = RegistryNode {
        identity: node_a.node_id().to_string(),
        endpoints: network
            .node(1)
            .expect("Unable to get node")
            .network_endpoints()
            .to_vec(),
        display_name: "node_a".into(),
        keys: vec![key.to_string()],
        metadata: metadata.clone(),
    };
    // Attempt to update node_a and check that an error is returned
    assert!(registry_client.update_node(&update_node_a_invalid).is_err());
    // Create a node with an id that does not exist in the registry
    let node_e = RegistryNode {
        identity: "non_existent_node_id".to_string(),
        endpoints: vec!["tcp://127.0.0.1:8085".into()],
        display_name: "node_a".into(),
        keys: vec![key.to_string()],
        metadata: metadata,
    };
    // Attempt to update a non existent node and check that it fails
    assert!(registry_client.update_node(&node_e).is_err());
    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that nodes can be deleted from the registry. Additionally test that
/// deleting a non-existent node from the registry will return an error.
///
/// 1. Create a network with three nodes
/// 2. List nodes using the registry client
/// 3. Check list length is 3
/// 4. Delete the second node from the registry
/// 5. Attempt to get the second node from the registry
/// 6. Check that None is returned
/// 7. List all nodes in the registry
/// 8. Check that the length of the returned list is 2
/// 9. Attempt to delete a node that does not exist from the registry
/// 10. Check that an error is returned
/// 11. Create `node_d` and add it to the registry
/// 12. Delete `node_d` from the registry
/// 13. Attempt to get `node_d` from the registry and check that None is returned
/// 14. Shutdown
#[test]
pub fn test_registry_delete_node() {
    // Start three node network
    let network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the first node
    let node_a = network.node(0).expect("Unable to get node");
    // Get the second node
    let node_b = network.node(1).expect("Unable to get node");
    // Get node's registry client
    let registry_client = node_a.registry_client();
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 3
    assert_eq!(registry_list_response.data.len(), 3);
    // Delete `node_b` from the registry
    registry_client
        .delete_node(node_b.node_id())
        .expect("registry delete node failed");
    // Try to get the deleted node from the registry
    let registry_get_response = registry_client
        .get_node(node_b.node_id())
        .expect("Registry get node request failed");
    // Check that None is returned because the node no longer exists in the registry
    assert!(registry_get_response.is_none());
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 2
    assert_eq!(registry_list_response.data.len(), 2);
    // Attempt to delete a node that doesn't exist from the registry
    assert!(registry_client.delete_node("non_existent_node_id").is_err());

    let key = node_a
        .admin_signer()
        .clone_box()
        .public_key()
        .expect("Unable to get signer's public key");

    let mut metadata = HashMap::new();
    metadata.insert("key1".into(), "value1".into());
    // Create a node
    let node_d = RegistryNode {
        identity: "node_d".into(),
        endpoints: vec!["tcp://127.0.0.1:8084".into()],
        display_name: "node_d".into(),
        keys: vec![key.to_string()],
        metadata: metadata,
    };
    // Add `node_d` to the registry
    registry_client
        .add_node(&node_d)
        .expect("registry add node failed");
    // Delete `node_d` from the registry
    registry_client
        .delete_node("node_d")
        .expect("registry add node failed");
    // Try to get node from the registry
    let registry_response = registry_client
        .get_node("node_d")
        .expect("Registry get node request failed");
    // Check that None is returned because the node no longer exists in the registry
    assert!(registry_response.is_none());
    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that registry nodes are accurately listed when filtering for specific
/// metadata.
///
/// 1. Create a network with one node
/// 2. List nodes using the registry client
/// 3. Check list length is 1
/// 4. Create two nodes with the same metadata and one with unique metadata
/// 5. Add all three nodes to the registry
/// 6. List all nodes in the registry and check that the length of the returned
///    list is 4
/// 7. List the nodes in the registry with the filter `key1=value1`
/// 8. Check that the two nodes with this metadata are returned in the list
/// 9. List the nodes in the registry with the filter `key2=value2`
/// 10. Check that the one node with this metadata is returned in the list
/// 11. List the nodes in the registry with the filter `key3=value3`
/// 12. Check that an empty list is returned because there are no nodes in the
///     registry with this metadata
/// 13. Shutdown
#[test]
pub fn test_registry_list_nodes_filter() {
    // Start single node network
    let network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(1)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the node
    let node_a = network.node(0).expect("Unable to get node");
    // Get node's registry client
    let registry_client = node_a.registry_client();
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 1
    assert_eq!(registry_list_response.data.len(), 1);
    let key = node_a
        .admin_signer()
        .clone_box()
        .public_key()
        .expect("Unable to get signer's public key");
    // Create two different sets of metadata
    let mut metadata_1 = HashMap::new();
    let mut metadata_2 = HashMap::new();
    metadata_1.insert("key1".into(), "value1".into());
    metadata_2.insert("key2".into(), "value2".into());
    // Create three nodes
    let node_b = RegistryNode {
        identity: "node_b".into(),
        endpoints: vec!["tcp://127.0.0.1:8084".into()],
        display_name: "node_b".into(),
        keys: vec![key.to_string()],
        metadata: metadata_1.clone(),
    };

    let node_c = RegistryNode {
        identity: "node_c".into(),
        endpoints: vec!["tcp://127.0.0.1:8085".into()],
        display_name: "node_c".into(),
        keys: vec![key.to_string()],
        metadata: metadata_1,
    };

    let node_d = RegistryNode {
        identity: "node_d".into(),
        endpoints: vec!["tcp://127.0.0.1:8086".into()],
        display_name: "node_d".into(),
        keys: vec![key.to_string()],
        metadata: metadata_2,
    };
    // Add all three nodes to the registry
    registry_client
        .add_node(&node_b)
        .expect("registry add node failed");
    registry_client
        .add_node(&node_c)
        .expect("registry add node failed");
    registry_client
        .add_node(&node_d)
        .expect("registry add node failed");
    // List all nodes in the registry
    let registry_list_response = registry_client
        .list_nodes(None)
        .expect("Registry get node request failed");
    // Check the length of the returned list is 4
    assert_eq!(registry_list_response.data.len(), 4);
    // List nodes from the registry with filter `key1=value1`
    let registry_list_response = registry_client
        .list_nodes(Some("{\"key1\":[\"=\",\"value1\"]}"))
        .expect("Registry get node request failed");
    // Check that the two nodes with this metadata are returned
    assert_eq!(registry_list_response.data.len(), 2);
    // Check that the id's of the nodes are correct
    assert_eq!(registry_list_response.data[0].identity, "node_b");
    assert_eq!(registry_list_response.data[1].identity, "node_c");
    // List nodes from the registry with filter `key2=value2`
    let registry_list_response = registry_client
        .list_nodes(Some("{\"key2\":[\"=\",\"value2\"]}"))
        .expect("Registry get node request failed");
    // Check that the one node with this metadata is returned
    assert_eq!(registry_list_response.data.len(), 1);
    // Check that the id of the node is correct
    assert_eq!(registry_list_response.data[0].identity, "node_d");
    // List nodes from the registry with filter `key3=value3`
    let registry_list_response = registry_client
        .list_nodes(Some("{\"key3\":[\"=\",\"value3\"]}"))
        .expect("Registry get node request failed");
    // Check that no nodes are returned because there are no nodes with this metadata
    assert_eq!(registry_list_response.data.len(), 0);
    shutdown!(network).expect("Unable to shutdown network");
}
