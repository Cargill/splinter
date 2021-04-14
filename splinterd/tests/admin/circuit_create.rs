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

use splinter::admin::client::event::{EventType, PublicKey};
use splinterd::node::{Node, RestApiVariant};

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
    let node_b_admin_pubkey = admin_pubkey(node_b);
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

    let node_a_event_client = node_a
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");

    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal event from each node
    let proposal_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let proposal_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal rejected
    let rejected_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let rejected_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(
        &EventType::ProposalRejected {
            requester: node_b_admin_pubkey.clone()
        },
        rejected_a_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalRejected {
            requester: node_b_admin_pubkey.clone()
        },
        rejected_b_event.event_type(),
    );
    assert_eq!(rejected_a_event.proposal(), rejected_b_event.proposal());
    let proposals_a = node_a
        .admin_service_client()
        .list_proposals(None, None)
        .expect("Unable to list proposals from first node")
        .data;
    assert!(proposals_a.is_empty());
    let proposals_b = node_b
        .admin_service_client()
        .list_proposals(None, None)
        .expect("Unable to list proposals from second node")
        .data;
    assert!(proposals_b.is_empty());

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
    let node_b_admin_pubkey = admin_pubkey(node_b);
    // Get the third node in the network
    let node_c = network.node(2).expect("Unable to get third node");
    let node_c_admin_pubkey = admin_pubkey(node_c);

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

    let node_a_event_client = node_a
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_c_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");

    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal event from each node
    let proposal_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let proposal_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    let proposal_c_event = node_c_event_client
        .next_event()
        .expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_c_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
    assert_eq!(proposal_a_event.proposal(), proposal_c_event.proposal());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // wait for vote event
    let vote_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let vote_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    let vote_c_event = node_c_event_client
        .next_event()
        .expect("Unable to get next event");

    assert_eq!(
        &EventType::ProposalVote {
            requester: node_b_admin_pubkey.clone()
        },
        vote_a_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalVote {
            requester: node_b_admin_pubkey.clone()
        },
        vote_b_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalVote {
            requester: node_b_admin_pubkey.clone()
        },
        vote_c_event.event_type(),
    );
    assert_eq!(vote_a_event.proposal(), vote_b_event.proposal());
    assert_eq!(vote_a_event.proposal(), vote_c_event.proposal());

    // Wait for proposal rejected
    let rejected_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let rejected_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    let rejected_c_event = node_c_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(
        &EventType::ProposalRejected {
            requester: node_c_admin_pubkey.clone()
        },
        rejected_a_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalRejected {
            requester: node_c_admin_pubkey.clone()
        },
        rejected_b_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalRejected {
            requester: node_c_admin_pubkey.clone()
        },
        rejected_c_event.event_type(),
    );
    assert_eq!(rejected_a_event.proposal(), rejected_b_event.proposal());
    assert_eq!(rejected_a_event.proposal(), rejected_c_event.proposal());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a_event.proposal().clone(),
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        false,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    let proposals_a = node_a
        .admin_service_client()
        .list_proposals(None, None)
        .expect("Unable to list proposals from first node")
        .data;
    assert!(proposals_a.is_empty());
    let proposals_b = node_b
        .admin_service_client()
        .list_proposals(None, None)
        .expect("Unable to list proposals from second node")
        .data;
    assert!(proposals_b.is_empty());
    let proposals_c = node_c
        .admin_service_client()
        .list_proposals(None, None)
        .expect("Unable to list proposals from third node")
        .data;
    assert!(proposals_c.is_empty());

    shutdown!(network).expect("Unable to shutdown network");
}

fn admin_pubkey(node: &Node) -> PublicKey {
    PublicKey(
        node.admin_signer()
            .public_key()
            .unwrap()
            .as_slice()
            .to_vec(),
    )
}
