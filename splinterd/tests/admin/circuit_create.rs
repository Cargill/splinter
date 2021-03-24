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

//! Integration tests for the creation of a circuit between multiple nodes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use splinterd::node::RestApiVariant;

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::payload::{make_circuit_proposal_vote_payload, make_create_circuit_payload};
use crate::framework::network::Network;

/// Test that a 2-party circuit may be created on a 2-node network.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to the second node, using `list_proposals`
/// 3. Verify the same proposal is available on each node
/// 4. Submit the same `CircuitCreateRequest` created in the first step from the second node
/// 5. Validate the duplicate proposal submitted in the previous step results in an error
/// 6. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 7. Wait until the circuit is available on the first node, using `list_circuits`
/// 8. Verify the same circuit is available to each node
#[test]
pub fn test_2_party_circuit_creation() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    let circuit_id = "ABCDE-01234";

    commit_2_party_circuit(circuit_id, node_a, node_b);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be created on a 3-node network.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to one of the other nodes, using `list_proposals`
/// 3. Verify the same proposal is available on every node
/// 4. Submit the same `CircuitManagmentPayload` created in the first step from the nodes that did
///    not submit the original `CircuitCreateRequest`.
/// 5. Validate the duplicate proposals submitted in the previous step each result in an error
/// 6. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 7. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal
/// 8. Validate the proposal has also been updated and includes the `Vote` submitted in the
///    previous steps for every node
/// 9. Create and submit a `CircuitProposalVote` from the third node to accept the proposal
/// 10. Wait until the circuit becomes available for one of the other nodes, using `list_circuits`
/// 11. Validate the circuit is available to every node
#[test]
pub fn test_3_party_circuit_creation() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Get the third node in the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit proposal may be submitted and committed to both nodes. This
/// test then validates the proposal is removed for the nodes when a proposed member votes to
/// reject the proposal.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to the second node, using `list_proposals`
/// 3. Verify the same proposal is available on each node
/// 4. Create and submit a `CircuitProposalVote` from the second node to reject the proposal
/// 5. Wait until the proposal is not available on the nodes, using `list_proposals`
/// 6. Verify the proposal does not exist on each node
#[test]
#[ignore]
pub fn test_2_party_circuit_creation_proposal_rejected() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get node_a");
    // Get the second node from the network
    let node_b = network.node(1).expect("Unable to get node_b");
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
    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
        &vec![node_a
            .admin_signer()
            .public_key()
            .expect("Unable to get first node's public key")
            .as_hex()],
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
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
            .expect("Unable to list proposals from second node")
            .data;

        if !proposals.is_empty() {
            // Unwrap the first item in the list as we've already validated this list is not empty
            proposal_b = proposals.get(0).unwrap().clone();
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
        break proposal_a;
    };

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed for the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect removed proposal in time");
        }
        let proposals_a = node_a
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from first node")
            .data;
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;

        if proposals_a.is_empty() && proposals_b.is_empty() {
            break;
        }
    }

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be proposed on a 3-node network. This test then validates the
/// proposal is removed if any of the proposed members votes to reject the proposal.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to one of the other nodes, using `list_proposals`
/// 3. Verify the same proposal is available on every node
/// 4. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 5. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal
/// 6. Validate the proposal has also been updated and includes the `Vote` submitted in the
///    previous steps for every node
/// 7. Create and submit a `CircuitProposalVote` from the third node to reject the proposal
/// 8. Wait until the proposal is no longer available to the other remote nodes, using
///    `list_proposals`
/// 9. Validate the proposal is no longer available for the node that voted to reject the proposal
#[test]
#[ignore]
pub fn test_3_party_circuit_creation_proposal_rejected() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Get the third node in the network
    let node_c = network.node(2).expect("Unable to get third node");
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
    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
        &vec![node_a
            .admin_signer()
            .public_key()
            .expect("Unable to get first node's public key")
            .as_hex()],
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
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
            // Unwrap the first element in each list as we've already validated the lists are not
            // empty
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
            Some(proposal) => proposal,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_b, proposal_c);
        break proposal_a;
    };

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
            // Validate the same proposal is available to each node
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
        } else {
            continue;
        }
    }

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        false,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed for the nodes
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect removed proposal in time");
        }
        let proposals_a = node_a
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from first node")
            .data;
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
        if proposals_a.is_empty() && proposals_b.is_empty() && proposals_c.is_empty() {
            break;
        } else {
            continue;
        }
    }

    shutdown!(network).expect("Unable to shutdown network");
}
