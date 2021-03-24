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

//! Provides the functionality of committing a 2- to 3-party circuit on a running network.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use splinterd::node::Node;

use crate::admin::payload::{make_circuit_proposal_vote_payload, make_create_circuit_payload};

/// Commit a 2-party circuit on a network that is already running
/// This function also validates that a duplicate proposal of the circuit being created is
/// rejected when submitted.
pub(in crate::admin) fn commit_2_party_circuit(circuit_id: &str, node_a: &Node, node_b: &Node) {
    // Create the list of node details needed to build the `CircuitCreateRequest`
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            node_a.network_endpoints().to_vec(),
        ),
        (
            node_b.node_id().to_string(),
            node_b.network_endpoints().to_vec(),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, Vec<String>>>();

    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
        &vec![
            node_a
                .admin_signer()
                .public_key()
                .expect("Unable to get first node's public key")
                .as_hex(),
            node_b
                .admin_signer()
                .public_key()
                .expect("Unable to get second node's public key")
                .as_hex(),
        ],
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone());
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the second node
    let mut proposal_b;
    let start = Instant::now();
    let proposal_a = loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_b")
            .data;

        if !proposals.is_empty() {
            // Unwrap the first element in this list as we've already validated that the list
            // is not empty
            proposal_b = proposals.get(0).unwrap().clone();
        } else {
            continue;
        }

        // Validate the same proposal is available to the first node
        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_a")
        {
            Some(proposal_a) => proposal_a,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        break proposal_a;
    };

    // Submit the same `CircuitManagmentPayload` to create the circuit to the second node
    // to validate this duplicate proposal is rejected
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
    assert!(duplicate_res.is_err());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be committed for the second node
    let mut circuit_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let circuits = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from node_b")
            .data;

        if !circuits.is_empty() {
            // Unwrap the first element in this list as we've already validated that the list
            // is not empty
            circuit_b = circuits.get(0).unwrap().clone();
        } else {
            continue;
        }

        // Validate the circuit is available to the first node
        let circuit_a = match node_a
            .admin_service_client()
            .fetch_circuit(&circuit_id)
            .expect("Unable to list circuits from node_b")
        {
            Some(circuit) => circuit,
            None => continue,
        };

        assert_eq!(circuit_a, circuit_b);
        break;
    }
}

/// Commit a 3-party circuit on a network that is already running
/// This function also validates that any duplicate proposal of the circuit being created is
/// rejected when submitted.
pub(in crate::admin) fn commit_3_party_circuit(
    circuit_id: &str,
    node_a: &Node,
    node_b: &Node,
    node_c: &Node,
) {
    // Create the list of node details needed to build the `CircuitCreateRequest`
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            node_a.network_endpoints().to_vec(),
        ),
        (
            node_b.node_id().to_string(),
            node_b.network_endpoints().to_vec(),
        ),
        (
            node_c.node_id().to_string(),
            node_c.network_endpoints().to_vec(),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, Vec<String>>>();

    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
        &vec![
            node_a
                .admin_signer()
                .public_key()
                .expect("Unable to get first node's public key")
                .as_hex(),
            node_b
                .admin_signer()
                .public_key()
                .expect("Unable to get second node's public key")
                .as_hex(),
            node_c
                .admin_signer()
                .public_key()
                .expect("Unable to get third node's public key")
                .as_hex(),
        ],
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone());
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the remote nodes
    let mut proposal_b;
    let mut proposal_c;
    let start = Instant::now();
    let proposal_a = loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        let proposals_c = node_c
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from third node")
            .data;
        if !proposals_b.is_empty() && !proposals_c.is_empty() {
            // Unwrap the first elements in each list as we've already validated that both of
            // the lists are not empty
            proposal_b = proposals_b.get(0).unwrap().clone();
            proposal_c = proposals_c.get(0).unwrap().clone();
        } else {
            continue;
        }

        // Validate the same proposal is available to the first node
        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal_a) => proposal_a,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_b, proposal_c);
        break proposal_a;
    };

    // Submit the same `CircuitManagmentPayload` to create the circuit to the second node
    // to validate this duplicate proposal is rejected
    let duplicate_res_b = node_b
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone());
    assert!(duplicate_res_b.is_err());
    // Submit the same `CircuitManagmentPayload` to create the circuit to the third node
    // to validate this duplicate proposal is rejected
    let duplicate_res_c = node_c
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
    assert!(duplicate_res_c.is_err());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the vote from this node to appear on the proposal for the remote nodes
    let mut proposal_a;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal vote in time");
        }
        // The proposal should already be available to each of these nodes, so we are able to
        // unwrap the result of the `fetch_proposal` call
        proposal_a = node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
            .unwrap();
        let proposal_b = node_b
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from second node")
            .unwrap();
        let proposal_c = node_c
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from third node")
            .unwrap();

        if proposal_a.votes.len() == 1 && proposal_b.votes.len() == 1 && proposal_c.votes.len() == 1
        {
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
        } else {
            continue;
        }
    }

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        true,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be committed for the other nodes
    let mut circuit_a;
    let mut circuit_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let circuits_a = node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from first node")
            .data;
        let circuits_b = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from third node")
            .data;
        if !circuits_a.is_empty() && !circuits_b.is_empty() {
            // Unwrap the first element in each list as we've already validated each of the
            // lists are not empty
            circuit_a = circuits_a.get(0).unwrap().clone();
            circuit_b = circuits_b.get(0).unwrap().clone();
        } else {
            continue;
        }

        // Validate the circuit is available to the first node
        let circuit_c = match node_c
            .admin_service_client()
            .fetch_circuit(&circuit_id)
            .expect("Unable to fetch circuit from third node")
        {
            Some(circuit) => circuit,
            None => continue,
        };

        assert_eq!(circuit_a, circuit_b);
        assert_eq!(circuit_b, circuit_c);
        break;
    }
}
