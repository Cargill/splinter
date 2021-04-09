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

use splinterd::node::RestApiVariant;

use crate::framework::network::Network;

/// Test that nodes can be stopped and restarted successfully.
///
/// 1. Create a three node network
/// 2. Stop the first node and check that it has been stopped and can no longer be
///    retrieved from the network
/// 3. Check that the second and third nodes are running by getting the registry clients
///    from them and using them
/// 4. Stop the second node and check that it has been stopped and can no longer be
///    retrieved from the network
/// 5. Check that the third node is still running by getting the registry client
///    from it and using it
/// 6. Stop the third node and check that it has been stopped and can no longer be
///    retrieved from the network
/// 7. Check that none of the nodes are running by ensuring none of them can be
///    retrieved from the network
/// 8. Start the first node and check that it can now be retrieved from the network
/// 9. Check that the first node is running by getting its registry client and using it
/// 10. Check that the second and thrid nodes are still stopped
/// 11. Start the second node and check that it can now be retrieved from the network
/// 12. Check that the second node is running by getting its registry client and using it
/// 13. Check that the third node is still stopped
/// 14. Start the third node and check that it can now be retrieved from the network
/// 15. Check that the third node is running by getting its registry client and using it
/// 16. Shutdown
#[test]
pub fn test_stop_start_nodes_multiple() {
    // create network with three nodes
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start three node ActixWeb1 network");

    // Stop the first node
    network = network.stop(0).expect("Unable to stop first node");

    // Check that the first node is stopped and can not be retrieved from the network
    assert!(network.node(0).is_err());
    // Check that the second and third nodes are still running and can be retrieved from
    // the network
    let node_1 = network.node(1).expect("Unable to retrieve second node");
    let node_2 = network.node(2).expect("Unable to retrieve third node");
    // Test that the second and third nodes are running by using the registry client
    let registry_response = node_1
        .registry_client()
        .get_node(node_1.node_id())
        .expect("Unable to get node from the registry");
    assert_eq!(node_1.node_id(), registry_response.unwrap().identity);
    let registry_response = node_2
        .registry_client()
        .list_nodes(None)
        .expect("Unable to list nodes from registry");
    assert_eq!(3, registry_response.data.len());

    // Stop the second node
    network = network.stop(1).expect("Unable to stop the second node");

    // Check that the second node is stopped and can not be retrieved from the network
    assert!(network.node(1).is_err());
    // Check that the first node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(0).is_err());
    // Check that the third nodes is still running and can be retrieved from the network
    let node_2 = network.node(2).expect("Unable to retrieve third node");
    // Test that the third node is running by using the registry client
    let registry_response = node_2
        .registry_client()
        .get_node(node_2.node_id())
        .expect("Unable to get node from the registry");
    assert_eq!(
        node_2.network_endpoints(),
        registry_response.unwrap().endpoints
    );

    // Stop the third node
    network = network.stop(2).expect("Unable to stop the third node");

    // Check that the third node is stopped and can not be retrieved from the network
    assert!(network.node(2).is_err());
    // Check that the second node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(1).is_err());
    // Check that the first node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(0).is_err());

    // Start the first node
    network = network.start(0).expect("Unable to start the first node");

    // Check that the first node has been started and can be retrieved from the network
    let node_0 = network.node(0).expect("Unable to retieve first node");
    // Check that the first node is running by using the registry client
    let registry_response = node_0
        .registry_client()
        .get_node(node_0.node_id())
        .expect("Unable to get node from the registry");
    assert_eq!(
        node_0.network_endpoints(),
        registry_response.unwrap().endpoints
    );

    // Check that the second node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(1).is_err());
    // Check that the third node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(2).is_err());

    // Start the second node
    network = network.start(1).expect("Unable to start the first node");

    // Check that the second node has been started and can be retrieved from the network
    let node_1 = network.node(1).expect("Unable to retieve second node");
    // Check that the second node is running by using the registry client
    let registry_response = node_1
        .registry_client()
        .get_node(node_1.node_id())
        .expect("Unable to get node from the registry");
    assert_eq!(
        node_1.network_endpoints(),
        registry_response.unwrap().endpoints
    );

    // Check that the third node is still stopped and can not be retrieved from
    // the network
    assert!(network.node(2).is_err());

    // Start the third node
    network = network.start(2).expect("Unable to start the third node");

    // Check that the third node has been started and can be retrieved from the network
    let node_2 = network.node(2).expect("Unable to retieve third node");
    // Check that the third node's is active by using the registry client
    let registry_response = node_2
        .registry_client()
        .get_node(node_2.node_id())
        .expect("Unable to get node from the registry");
    assert_eq!(
        node_2.network_endpoints(),
        registry_response.unwrap().endpoints
    );

    shutdown!(network).expect("Unable to shutdown network");
}
