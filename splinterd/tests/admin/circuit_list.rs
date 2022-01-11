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

//! A framework for running a network of Splinter nodes in a single process, usually for
//! integration testing purposes.

use splinterd::node::RestApiVariant;

use crate::framework::network::Network;

/// Creates a single node network and confirms that the admin service's REST API is available
/// by listing circuits (which will be empty).
fn single_node_network(rest_api_variant: RestApiVariant) {
    let mut network = Network::new()
        .with_default_rest_api_variant(rest_api_variant)
        .add_nodes_with_defaults(1)
        .unwrap();

    let client = network.node(0).unwrap().admin_service_client();

    // make a call to the port
    let list_slice = client.list_circuits(None).unwrap();
    assert_eq!(list_slice.data, vec![]);
    assert_eq!(0, list_slice.paging.total);

    shutdown!(network).unwrap();
}

/// Creates a multi-node network and confirms that the admin service's REST API is available
/// by listing circuits (which will be empty).
fn multi_node_network(rest_api_variant: RestApiVariant) {
    let mut network = Network::new()
        .with_default_rest_api_variant(rest_api_variant)
        .add_nodes_with_defaults(3)
        .unwrap();

    let client_0 = network.node(0).unwrap().admin_service_client();
    // make a call to the first node's port
    let list_slice = client_0
        .list_circuits(None)
        .expect("Unable to list circuits from node 0");
    assert_eq!(list_slice.data, vec![]);

    let client_1 = network.node(1).unwrap().admin_service_client();
    // make a call to the second node's port
    let list_slice = client_1
        .list_circuits(None)
        .expect("Unable to list circuits from node 1");
    assert_eq!(list_slice.data, vec![]);

    let client_2 = network.node(2).unwrap().admin_service_client();
    // make a call to the last node's port
    let list_slice = client_2
        .list_circuits(None)
        .expect("Unable to list circuits from node 2");
    assert_eq!(list_slice.data, vec![]);

    shutdown!(network).unwrap();
}

/// Executes the single node network test with Actix Web 1.
#[test]
fn single_node_network_actix_web_1() {
    single_node_network(RestApiVariant::ActixWeb1);
}

/// Executes the single node network test with Actix Web 3.
#[test]
#[ignore]
fn single_node_network_actix_web_3() {
    single_node_network(RestApiVariant::ActixWeb3);
}

/// Executes the multi node network test with Actix Web 1.
#[test]
fn multi_node_network_actix_web_1() {
    multi_node_network(RestApiVariant::ActixWeb1);
}

/// Executes the multi node network test with Actix Web 3.
#[test]
#[ignore]
fn multi_node_network_actix_web_3() {
    multi_node_network(RestApiVariant::ActixWeb3);
}
