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

//! Admin service integration tests.

mod biome;
mod circuit_abandon;
mod circuit_commit;
mod circuit_create;
mod circuit_disband;
mod circuit_list;
mod node_lifecycle;
pub(super) mod payload;
mod registry;
mod scabbard_service;

use scabbard::client::ServiceId;
use splinterd::node::Node;

// Helper function to generate the `ServiceId` for the provided Node on the circuit specified by
// the `circuit_id` argument. The generic definition of this function allows for this function to
// be used for any Node that is apart of the circuit specified.
pub(super) fn get_node_service_id(circuit_id: &str, node: &Node) -> ServiceId {
    // Retrieve the node's associated `service_id` from the circuit just committed
    let circuit = node
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit")
        .unwrap();
    // Create the `ServiceId` struct based on the node's associated `service_id` and the
    // committed `circuit_id`
    let node_service = &circuit
        .roster
        .iter()
        .find(|service_slice| &service_slice.node_id == node.node_id())
        .expect("Circuit committed without service for node")
        .service_id;
    format!("{}::{}", &circuit_id, node_service)
        .parse::<ServiceId>()
        .expect("Unable to parse `ServiceId`")
}
