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

//! Integration tests for the lifecycle of a circuit between multiple nodes. These tests verify
//! the process of creating and then disbanding a circuit between two and three nodes.

use splinter::admin::client::event::{BlockingAdminServiceEventIterator, EventType, PublicKey};
use splinterd::node::{Node, RestApiVariant};

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::payload::{make_circuit_disband_payload, make_circuit_proposal_vote_payload};
use crate::framework::circuit_builder::{CircuitData, ScabbardCircuitBuilderVeil};
use crate::framework::network::Network;

/// Test that a 2-party circuit may be created on a 2-node network. This test then validates the
/// circuit is able to be disbanded. Furthermore, this test validates the disbanded circuit is
/// still accessible to each node and the circuit definition is as expected, after disbanding.
///
/// 1. Create a circuit between 2 nodes
/// 2. Create and submit a `CircuitDisbandRequest` from the first node
/// 3. Wait until the disband proposal is available to the second node, using `list_proposals`
/// 4. Verify the same disband proposal is available on each node
/// 5. Submit the same `CircuitDisbandRequest` from the second step to the second node
/// 6. Validate this duplicate disband proposal is rejected
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 8. Wait until the circuit is no longer available as an active circuit on the first node,
///    using `list_circuits`
/// 9. Validate the circuit is no longer active on every node
/// 10. Validate the disbanded circuit is still available to each node, though disbanded, and that
///    the disbanded circuit is the same for each node
#[test]
pub fn test_2_party_circuit_lifecycle() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Commit the circuit to state
    let node_b_admin_pubkey = admin_pubkey(node_b);

    let CircuitData {
        circuit_id,
        management_type,
        ..
    } = network
        .circuit_builder(&[0, 1])
        .expect("Could not create builder")
        .veil::<ScabbardCircuitBuilderVeil>()
        .add_service_group(&[0, 1])
        .unwrap()
        .build()
        .expect("Could not create circuit");

    // As we've started a new event client, we'll skip just past the circuit ready event
    let mut node_a_events = BlockingAdminServiceEventIterator::new(
        node_a
            .admin_service_event_client(&management_type)
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_b_events = BlockingAdminServiceEventIterator::new(
        node_b
            .admin_service_event_client(&management_type)
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_events.next().expect("Unable to get next event");
    let proposal_b_event = node_b_events.next().expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // Submit a duplicate of the disband `CircuitManagementPayload` to the second node
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(duplicate_res.is_err());

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal accepted
    let accepted_a_event = node_a_events.next().expect("Unable to get next event");
    let accepted_b_event = node_b_events.next().expect("Unable to get next event");
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
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // Wait for circuit ready event.
    let ready_a_event = node_a_events.next().expect("Unable to get next event");
    let ready_b_event = node_b_events.next().expect("Unable to get next event");
    assert_eq!(ready_a_event.event_type(), &EventType::CircuitDisbanded);
    assert_eq!(ready_b_event.event_type(), &EventType::CircuitDisbanded);
    assert_eq!(ready_a_event.proposal(), ready_b_event.proposal());
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // If the circuit no longer appears in the list of active circuits, the circuit
    // has been successfully disbanded.
    let circuits_a = node_a
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from first node")
        .data;
    let circuits_b = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from second node")
        .data;
    assert!(circuits_a.is_empty());
    assert!(circuits_b.is_empty());

    // Validate the disbanded circuit is the same for both nodes
    let disbanded_circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from first node")
        .unwrap();
    let disbanded_circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from second node")
        .unwrap();
    assert_eq!(disbanded_circuit_a, disbanded_circuit_b);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit may be created on a 2-node network. This test then validates a
/// circuit member is able to propose to disband the circuit. This test then validates the disband
/// request is able to be rejected by another circuit member, removing the disband proposal.
///
/// 1. Create a circuit between 2 nodes
/// 2. Create and submit a `CircuitDisbandRequest` from the first node
/// 3. Wait until the disband proposal is available to the second node, using `list_proposals`
/// 4. Verify the same disband proposal is available on each node
/// 5. Create and submit a `CircuitProposalVote` from the second node to reject the disband proposal
/// 6. Wait until the disband proposal is no longer available to the second node,
///    using `list_proposals`
/// 7. Validate the proposal is no longer available on the nodes
/// 8. Validate the active circuit is still available to each node, using `list_circuits` which
///    only returns active circuits
#[test]
#[ignore]
pub fn test_2_party_circuit_disband_proposal_rejected() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let node_b_admin_pubkey = admin_pubkey(node_b);
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b);

    // As we've started a new event client, we'll skip just past the circuit ready event
    let mut node_a_events = BlockingAdminServiceEventIterator::new(
        node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_b_events = BlockingAdminServiceEventIterator::new(
        node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_events.next().expect("Unable to get next event");
    let proposal_b_event = node_b_events.next().expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal rejection
    let rejected_a_event = node_a_events.next().expect("Unable to get next event");
    let rejected_b_event = node_b_events.next().expect("Unable to get next event");
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
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // validate the proposal no longer appears in the list, the proposal has been removed as it was
    // rejected
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
    assert!(proposals_a.is_empty());
    assert!(proposals_b.is_empty());

    // Validate the active circuit is still available to each node
    let active_circuits_a = node_a
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from first node")
        .data;
    assert!(active_circuits_a.len() == 1);
    let active_circuits_b = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from second node")
        .data;
    assert!(active_circuits_b.len() == 1);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be created on a 3-node network. This test then validates the
/// circuit is able to be disbanded.
///
/// 1. Create a circuit between 3 nodes
/// 2. Create and submit a `CircuitDisbandRequest` from the first node
/// 3. Wait until the disband proposal is available to each node, using `list_proposals`
/// 4. Verify the same disband proposal is present on each node
/// 5. Submit the same `CircuitDisbandRequest` to the nodes that did not originally submit the
///    proposal
/// 6. Validate these duplicate proposals are rejected
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 8. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal for each
///    remote node
/// 9. Validate the proposal has been updated and includes the `Vote` submitted in the previous
///    steps for every node
/// 10. Create and submit a `CircuitProposalVote` from the third node to accept the disband proposal
/// 11. Wait until the active circuit is no longer available to the remote nodes, using
///    `list_circuits`
/// 12. Validate the circuit is no longer active on the nodes
/// 13. Validate the disbanded circuit is still available to each node, though disbanded, and that
///    the disbanded circuit is the same for each node
#[test]
#[ignore]
pub fn test_3_party_circuit_lifecycle() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node from the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node from the network
    let node_b = network.node(1).expect("Unable to get second node");
    let node_b_admin_pubkey = admin_pubkey(node_b);
    // Get the third node from the network
    let node_c = network.node(2).expect("Unable to get third node");
    let node_c_admin_pubkey = admin_pubkey(node_b);

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

    // As we've started a new event client, we'll skip just past the circuit ready event
    let mut node_a_events = BlockingAdminServiceEventIterator::new(
        node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_b_events = BlockingAdminServiceEventIterator::new(
        node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_c_events = BlockingAdminServiceEventIterator::new(
        node_c
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_events.next().expect("Unable to get next event");
    let proposal_b_event = node_b_events.next().expect("Unable to get next event");
    let proposal_c_event = node_c_events.next().expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_c_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
    assert_eq!(proposal_a_event.proposal(), proposal_c_event.proposal());
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // Submit a duplicate of the disband `CircuitManagementPayload` to the second node
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(duplicate_res.is_err());
    // Submit a duplicate of the disband `CircuitManagementPayload` to the third node
    let duplicate_res = node_c
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(duplicate_res.is_err());

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // wait for vote event
    let vote_a_event = node_a_events.next().expect("Unable to get next event");
    let vote_b_event = node_b_events.next().expect("Unable to get next event");
    let vote_c_event = node_c_events.next().expect("Unable to get next event");
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
        proposal_c_event.proposal().clone(),
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        true,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal accepted
    let accepted_a_event = node_a_events.next().expect("Unable to get next event");
    let accepted_b_event = node_b_events.next().expect("Unable to get next event");
    let accepted_c_event = node_c_events.next().expect("Unable to get next event");
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
    let ready_a_event = node_a_events.next().expect("Unable to get next event");
    let ready_b_event = node_b_events.next().expect("Unable to get next event");
    let ready_c_event = node_c_events.next().expect("Unable to get next event");
    assert_eq!(ready_a_event.event_type(), &EventType::CircuitDisbanded);
    assert_eq!(ready_b_event.event_type(), &EventType::CircuitDisbanded);
    assert_eq!(ready_c_event.event_type(), &EventType::CircuitDisbanded);
    assert_eq!(ready_a_event.proposal(), ready_b_event.proposal());
    assert_eq!(ready_a_event.proposal(), ready_c_event.proposal());

    // Validate the disbanded circuit is available and the same for each node
    let disbanded_circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from first node")
        .unwrap();
    let disbanded_circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from second node")
        .unwrap();
    let disbanded_circuit_c = node_c
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from third node")
        .unwrap();
    assert_eq!(disbanded_circuit_a, disbanded_circuit_b);
    assert_eq!(disbanded_circuit_b, disbanded_circuit_c);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be created on a 3-node network. This test then validates the
/// circuit is able to be disbanded.
///
/// 1. Create a circuit between 3 nodes
/// 2. Create and submit a `CircuitDisbandRequest` from the first node
/// 3. Wait until the disband proposal is available to each node, using `list_proposals`
/// 4. Verify the same disband proposal is present on each node
/// 5. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 6. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal for each
///    remote node
/// 7. Validate the proposal has been updated and includes the `Vote` submitted in the previous
///    steps for every node
/// 8. Create and submit a `CircuitProposalVote` from the third node to reject the disband proposal
/// 9. Wait until the disband proposal is no longer available to the remote nodes, using
///    `list_proposals`
/// 10. Validate the disband proposal is no longer available for every node
/// 11. Validate the circuit is still active for each node, using `list_circuits` which only returns
///     active circuits
#[test]
#[ignore]
pub fn test_3_party_circuit_lifecycle_proposal_rejected() {
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
    let node_c_admin_pubkey = admin_pubkey(node_b);

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

    // As we've started a new event client, we'll skip just past the circuit ready event
    let mut node_a_events = BlockingAdminServiceEventIterator::new(
        node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_b_events = BlockingAdminServiceEventIterator::new(
        node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.
    let mut node_c_events = BlockingAdminServiceEventIterator::new(
        node_c
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id))
            .expect("Unable to get event client"),
    )
    .skip_while(|evt| evt.event_type() != &EventType::CircuitReady)
    .skip(1); // skip the ready event itself.

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(res.is_ok());

    // Wait for the proposal event from each node.
    let proposal_a_event = node_a_events.next().expect("Unable to get next event");
    let proposal_b_event = node_b_events.next().expect("Unable to get next event");
    let proposal_c_event = node_b_events.next().expect("Unable to get next event");

    assert_eq!(&EventType::ProposalSubmitted, proposal_a_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_b_event.event_type());
    assert_eq!(&EventType::ProposalSubmitted, proposal_c_event.event_type());
    assert_eq!(proposal_a_event.proposal(), proposal_b_event.proposal());
    assert_eq!(proposal_a_event.proposal(), proposal_c_event.proposal());
    assert_eq!(&proposal_a_event.proposal().proposal_type, "Disband");

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // wait for vote event
    let vote_a_event = node_a_events.next().expect("Unable to get next event");
    let vote_b_event = node_b_events.next().expect("Unable to get next event");
    let vote_c_event = node_c_events.next().expect("Unable to get next event");
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
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_c_event.proposal().clone(),
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        false,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for proposal accepted
    let rejected_a_event = node_a_events.next().expect("Unable to get next event");
    let rejected_b_event = node_b_events.next().expect("Unable to get next event");
    let rejected_c_event = node_c_events.next().expect("Unable to get next event");
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

    // Validate the active circuit is still available to each node
    let active_circuits_a = node_a
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from first node")
        .data;
    assert!(active_circuits_a.len() == 1);

    let active_circuits_b = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from second node")
        .data;
    assert!(active_circuits_b.len() == 1);

    let active_circuits_c = node_c
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from third node")
        .data;
    assert!(active_circuits_c.len() == 1);

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
