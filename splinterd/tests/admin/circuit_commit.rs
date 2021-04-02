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

use splinter::admin::client::event::{EventType, PublicKey};
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

    let node_b_admin_pubkey = admin_pubkey(node_b);

    let node_a_event_client = node_a
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");

    let circuit_payload_bytes = make_create_circuit_payload(
        &circuit_id,
        node_a.node_id(),
        node_info,
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone());
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let proposal_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());

    // Submit the same `CircuitManagmentPayload` to create the circuit to the second node
    // to validate this duplicate proposal is rejected
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes);
    assert!(
        duplicate_res.is_err(),
        "node {} erroneously accepted a duplicate proposal",
        node_b.node_id()
    );

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

    // Wait for proposal accepted
    let accepted_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let accepted_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(
        &EventType::ProposalAccepted {
            requester: node_b_admin_pubkey.clone()
        },
        accepted_a_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalAccepted {
            requester: node_b_admin_pubkey.clone()
        },
        accepted_b_event.event_type(),
    );
    assert_eq!(accepted_a_event.proposal(), accepted_b_event.proposal());

    // Wait for circuit ready.
    let ready_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let ready_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(ready_a_event.event_type(), &EventType::CircuitReady);
    assert_eq!(ready_b_event.event_type(), &EventType::CircuitReady);
    assert_eq!(ready_a_event.proposal(), ready_b_event.proposal());

    let circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);
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

    let node_b_admin_pubkey = admin_pubkey(node_b);
    let node_c_admin_pubkey = admin_pubkey(node_c);

    let node_a_event_client = node_a
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");
    let node_c_event_client = node_b
        .admin_service_event_client("test_circuit")
        .expect("Unable to get event client");

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
        .submit_admin_payload(circuit_payload_bytes.clone());
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

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a_event.proposal().clone(),
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        true,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal accepted
    let accepted_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let accepted_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    let accepted_c_event = node_c_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(
        &EventType::ProposalAccepted {
            requester: node_c_admin_pubkey.clone()
        },
        accepted_a_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalAccepted {
            requester: node_c_admin_pubkey.clone()
        },
        accepted_b_event.event_type(),
    );
    assert_eq!(
        &EventType::ProposalAccepted {
            requester: node_c_admin_pubkey.clone()
        },
        accepted_c_event.event_type(),
    );
    assert_eq!(accepted_a_event.proposal(), accepted_b_event.proposal());
    assert_eq!(accepted_a_event.proposal(), accepted_c_event.proposal());

    // Wait for circuit ready.
    let ready_a_event = node_a_event_client
        .next_event()
        .expect("Unable to get next event");
    let ready_b_event = node_b_event_client
        .next_event()
        .expect("Unable to get next event");
    let ready_c_event = node_c_event_client
        .next_event()
        .expect("Unable to get next event");
    assert_eq!(ready_a_event.event_type(), &EventType::CircuitReady);
    assert_eq!(ready_b_event.event_type(), &EventType::CircuitReady);
    assert_eq!(ready_c_event.event_type(), &EventType::CircuitReady);
    assert_eq!(ready_a_event.proposal(), ready_b_event.proposal());
    assert_eq!(ready_a_event.proposal(), ready_c_event.proposal());

    // Validate the circuit is available to the first node
    let circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from first node")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from second node")
        .expect("Circuit was not found");
    let circuit_c = node_c
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from third node")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);
    assert_eq!(circuit_b, circuit_c);
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
