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

//! Integration tests for the creation of a circuit between multiple nodes.

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;

use splinter::admin::client::event::{EventType, PublicKey};
use splinter::admin::messages::AuthorizationType;
use splinter::peer::{PeerAuthorizationToken, PeerManagerNotification, PeerTokenPair};
use splinterd::node::{Node, RestApiVariant};

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::payload::{make_circuit_proposal_vote_payload, make_create_circuit_payload};
use crate::framework::network::Network;
use crate::framework::timeout::timeout;

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

    commit_2_party_circuit(circuit_id, node_a, node_b, AuthorizationType::Trust);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit may be created on a 2-node network using challenge authorization.
///
/// 1. Start a two node network and get both nodes.
/// 2. Use commit_2_party_circuit to verify that a circuit can be created between the two nodes
///    using challenge authorization.
/// 3. Shutdown the network
#[test]
pub fn test_2_party_circuit_creation_challenge_authorization() {
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

    commit_2_party_circuit(circuit_id, node_a, node_b, AuthorizationType::Challenge);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit may be created on a 2-node network using challenge authorization
/// after the nodes have already connected via endpoints.
///
/// 1. Add node_b as an unidentified peer on node_a
/// 2. Wait for node_a to get connection notification of node_b.
/// 3. Use commit_2_party_circuit to verify that a circuit can be created between the two nodes
///    using challenge authorization.
/// 4. Shutdown the network.
#[test]
pub fn test_2_party_circuit_creation_challenge_authorization_unidentified_peer() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");

    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    let peer_connector = node_a.peer_connector();
    let (tx, notification_rx): (mpsc::Sender<TestEnum>, mpsc::Receiver<TestEnum>) = mpsc::channel();
    peer_connector
        .subscribe_sender(tx)
        .expect("Unable to get subscriber");

    let _peer_ref = peer_connector
        .add_unidentified_peer(
            node_b.network_endpoints()[0].to_string(),
            PeerAuthorizationToken::from_public_key(
                node_a
                    .signers()
                    .get(0)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key")
                    .as_slice(),
            ),
        )
        .expect("Unable to request connection to peer by endpoint");

    // timeout after 60 seconds
    let timeout = Duration::from_secs(60);
    let notification = notification_rx
        .recv_timeout(timeout)
        .expect("Unable to get new notifications");
    assert_eq!(
        notification,
        TestEnum::Notification(PeerManagerNotification::Connected {
            peer: PeerTokenPair::new(
                PeerAuthorizationToken::from_public_key(
                    node_b
                        .signers()
                        .get(0)
                        .expect("node does not have enough signers configured")
                        .public_key()
                        .expect("Unable to get first node's public key")
                        .as_slice(),
                ),
                PeerAuthorizationToken::from_public_key(
                    node_a
                        .signers()
                        .get(0)
                        .expect("node does not have enough signers configured")
                        .public_key()
                        .expect("Unable to get first node's public key")
                        .as_slice(),
                )
            )
        })
    );

    let circuit_id = "ABCDE-01234";

    commit_2_party_circuit(circuit_id, node_a, node_b, AuthorizationType::Challenge);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that two 2-party circuit may be created on a 2-node network using challenge authorization,
/// where the second circuit has a different key for the first node defined in the circuit.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait for the `ProposalSubmitted` event from each node's event client
/// 3. Verify the same proposal is available on each node
/// 4. Submit the same `CircuitCreateRequest` created in the first step from the second node
/// 5. Validate the duplicate proposal submitted in the previous step results in an error
/// 6. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 7. Wait for the `CircuitReady` event from each node's event client
/// 8. Verify the same circuit is available to each node
/// 9. Repeat steps 1-8 for a different circuit that sets the member public key for the first node
///    to the second configured key.
#[test]
pub fn test_2_party_circuit_creation_challenge_authorization_different_key() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .set_num_of_keys(2)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    let first_circuit_id = "ABCDE-01234";

    commit_2_party_circuit(
        first_circuit_id,
        node_a,
        node_b,
        AuthorizationType::Challenge,
    );

    let second_circuit_id = "ABCDE-56789";
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    // Create the list of node details needed to build the `CircuitCreateRequest`
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                // get the second signer (not the normal key in the first position)
                node_a
                    .signers()
                    .get(1)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .signers()
                    .get(0)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();

    let node_b_admin_pubkey = admin_pubkey(node_b);

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &second_circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &second_circuit_id), None)
        .expect("Unable to get event client");

    let circuit_payload_bytes = make_create_circuit_payload(
        &second_circuit_id,
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
        AuthorizationType::Challenge,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone())
        .expect("Unable to submit admin payload to admin service");

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
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");

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
        .fetch_circuit(&second_circuit_id)
        .expect("Unable to get circuit from node_a")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&second_circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that two 2-party circuit may be created on a 2-node network using challenge authorization,
/// where the second circuit has a different key for the both nodes defined in the circuit.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait for the `ProposalSubmitted` event from each node's event client
/// 3. Verify the same proposal is available on each node
/// 4. Submit the same `CircuitCreateRequest` created in the first step from the second node
/// 5. Validate the duplicate proposal submitted in the previous step results in an error
/// 6. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 7. Wait for the `CircuitReady` event from each node's event client
/// 8. Verify the same circuit is available to each node
/// 9. Repeat steps 1-8 for a different circuit that sets the member's public keys to the nodes
///    second configured key.
#[test]
pub fn test_2_party_circuit_creation_challenge_authorization_different_keys() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .set_num_of_keys(2)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    let first_circuit_id = "ABCDE-01234";

    commit_2_party_circuit(
        first_circuit_id,
        node_a,
        node_b,
        AuthorizationType::Challenge,
    );

    let second_circuit_id = "ABCDE-56789";
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    // Create the list of node details needed to build the `CircuitCreateRequest`
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                // get the second signer (not the admin signer which is in the first position)
                node_a
                    .signers()
                    .get(1)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .signers()
                    .get(1)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();

    let node_b_admin_pubkey = admin_pubkey(node_b);

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &second_circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &second_circuit_id), None)
        .expect("Unable to get event client");

    let circuit_payload_bytes = make_create_circuit_payload(
        &second_circuit_id,
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
        AuthorizationType::Challenge,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes.clone())
        .expect("Unable to submit admin payload to admin service");

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
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");

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
        .fetch_circuit(&second_circuit_id)
        .expect("Unable to get circuit from node_a")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&second_circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);

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
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .ok();

    timeout(Duration::from_secs(300), || {
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
        commit_3_party_circuit(circuit_id, node_a, node_b, node_c, AuthorizationType::Trust);

        shutdown!(network).expect("Unable to shutdown network");
    })
}

/// Test that a 3-party circuit may be created on a 3-node network using challenge authorization.
///
/// 1. Create a 3 node network and get each node.
/// 2. Use commit_3_party_circuit to verify that a circuit can be created between the three nodes
///    using challenge authorization.
/// 3. Shutdown the network.
#[test]
pub fn test_3_party_circuit_creation_challenge_authorization() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .ok();

    timeout(Duration::from_secs(300), || {
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
        commit_3_party_circuit(
            circuit_id,
            node_a,
            node_b,
            node_c,
            AuthorizationType::Challenge,
        );

        shutdown!(network).expect("Unable to shutdown network");
    })
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
            (
                node_a.network_endpoints().to_vec(),
                node_a
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get seconds node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
    let circuit_id = "ABCDE-01234";

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");
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
        AuthorizationType::Trust,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes)
        .expect("Unable to submit admin payload to admin service");

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
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");

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
pub fn test_3_party_circuit_creation_proposal_rejected() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .ok();

    timeout(Duration::from_secs(300), || {
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
                (
                    node_a.network_endpoints().to_vec(),
                    node_a
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get first node's public key"),
                ),
            ),
            (
                node_b.node_id().to_string(),
                (
                    node_b.network_endpoints().to_vec(),
                    node_b
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get seconds node's public key"),
                ),
            ),
            (
                node_c.node_id().to_string(),
                (
                    node_c.network_endpoints().to_vec(),
                    node_c
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get third node's public key"),
                ),
            ),
        ]
        .into_iter()
        .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
        let circuit_id = "ABCDE-01234";

        let node_a_event_client = node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_b_event_client = node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_c_event_client = node_c
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");

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
            AuthorizationType::Trust,
        )
        .expect("Unable to generate circuit request");
        // Submit the `CircuitManagementPayload` to the first node
        node_a
            .admin_service_client()
            .submit_admin_payload(circuit_payload_bytes)
            .expect("Unable to submit admin payload to admin service");

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
        node_b
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");

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
        // Uses `false` for the `accept` argument to create a vote to reject the proposal
        let vote_payload_bytes = make_circuit_proposal_vote_payload(
            proposal_a_event.proposal().clone(),
            node_c.node_id(),
            &*node_c.admin_signer().clone_box(),
            false,
        );
        node_c
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");

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
    })
}

/// Test that a 2-party circuit proposal may be submitted and committed to both nodes, while the
/// nodes are stopped throughout the process.
///
/// 1. Collect  the node information needed to create the `CircuitCreateRequest`
/// 2. Stop the second node in the network
/// 3. Create and submit a `CircuitCreateRequest` from the first node
/// 4. Restart the second node in the network
/// 5. Wait for the `ProposalSubmitted` event from each node's event client, validate the
///    corresponding proposal
/// 6. Stop the first node in the network
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 8. Restart the first node in the network
/// 9. Wait for the `ProposalAccepted` event from each node's event client, validate the
///    corresponding proposal
/// 10. Wait for the `CircuitReady` event from each node's event client, validate the
///     corresponding proposal
/// 11. Verify the circuit is now available to each node, using `fetch_circuit`
#[test]
#[ignore]
pub fn test_2_party_circuit_creation_stop() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let mut node_a = network.node(0).expect("Unable to get node_a");
    // Get the second node from the network
    let mut node_b = network.node(1).expect("Unable to get node_b");
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                node_a
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get seconds node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
    let circuit_id = "ABCDE-01234";
    // Stop the second node in the network
    network = network.stop(1).expect("Unable to stop second node");
    node_a = network.node(0).expect("Unable to get first node");
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
        AuthorizationType::Trust,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the second node in the network
    network = network.start(1).expect("Unable to start second node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");

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
    // Stop the first node in the network
    network = network.stop(0).expect("Unable to stop first node");
    node_b = network.node(1).expect("Unable to get second node");
    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the first node in the network
    network = network.start(0).expect("Unable to start first node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    let node_b_admin_pubkey = admin_pubkey(node_b);
    let node_a_event_client = node_a
        .admin_service_event_client(
            &format!("test_circuit_{}", &circuit_id),
            Some(*proposal_a_event.event_id()),
        )
        .expect("Unable to get event client");

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
        .expect("Unable to get circuit from node_a")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit proposal using challenge authorization may be submitted and
/// committed to both nodes, while the nodes are stopped throughout the process.
///
/// 1. Collect  the node information needed to create the `CircuitCreateRequest`
/// 2. Stop the second node in the network
/// 3. Create and submit a `CircuitCreateRequest` from the first node
/// 4. Restart the second node in the network
/// 5. Wait for the `ProposalSubmitted` event from each node's event client, validate the
///    corresponding proposal
/// 6. Stop the first node in the network
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 8. Restart the first node in the network
/// 9. Wait for the `ProposalAccepted` event from each node's event client, validate the
///    corresponding proposal
/// 10. Wait for the `CircuitReady` event from each node's event client, validate the
///     corresponding proposal
/// 11. Verify the circuit is now available to each node, using `fetch_circuit`
#[test]
#[ignore]
pub fn test_2_party_circuit_creation_stop_challenge_authorization() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let mut node_a = network.node(0).expect("Unable to get node_a");
    // Get the second node from the network
    let mut node_b = network.node(1).expect("Unable to get node_b");
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                node_a
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get seconds node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
    let circuit_id = "ABCDE-01234";
    // Stop the second node in the network
    network = network.stop(1).expect("Unable to stop second node");
    node_a = network.node(0).expect("Unable to get first node");
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
        AuthorizationType::Challenge,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the second node in the network
    network = network.start(1).expect("Unable to start second node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");

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
    // Stop the first node in the network
    network = network.stop(0).expect("Unable to stop first node");
    node_b = network.node(1).expect("Unable to get second node");
    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the first node in the network
    network = network.start(0).expect("Unable to start first node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    let node_b_admin_pubkey = admin_pubkey(node_b);
    let node_a_event_client = node_a
        .admin_service_event_client(
            &format!("test_circuit_{}", &circuit_id),
            Some(*proposal_a_event.event_id()),
        )
        .expect("Unable to get event client");

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
        .expect("Unable to get circuit from node_a")
        .expect("Circuit was not found");
    let circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to get circuit from node_b")
        .expect("Circuit was not found");

    assert_eq!(circuit_a, circuit_b);

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit proposal may be submitted and committed to both nodes, while the
/// nodes are stopped throughout the process.
///
/// 1. Collect  the node information needed to create the `CircuitCreateRequest`
/// 2. Stop the second node in the network
/// 3. Create and submit a `CircuitCreateRequest` from the first node
/// 4. Restart the second node in the network
/// 5. Wait for the `ProposalSubmitted` event from each node's event client, validate the
///    corresponding proposal
/// 6. Stop the first node in the network
/// 7. Create and submit a `CircuitProposalVote` from the second node to reject the proposal
/// 8. Restart the first node in the network
/// 9. Wait for the `ProposalRejected` event from each node's event client, validate the
///    corresponding proposal
/// 10. Verify the circuit proposal is not available to any node
#[test]
#[ignore]
pub fn test_2_party_circuit_proposal_rejected_stop() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let mut node_a = network.node(0).expect("Unable to get node_a");
    // Get the second node from the network
    let mut node_b = network.node(1).expect("Unable to get node_b");
    let node_info = vec![
        (
            node_a.node_id().to_string(),
            (
                node_a.network_endpoints().to_vec(),
                node_a
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get first node's public key"),
            ),
        ),
        (
            node_b.node_id().to_string(),
            (
                node_b.network_endpoints().to_vec(),
                node_b
                    .admin_signer()
                    .clone()
                    .public_key()
                    .expect("Unable to get seconds node's public key"),
            ),
        ),
    ]
    .into_iter()
    .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
    let circuit_id = "ABCDE-01234";
    // Stop the second node in the network
    network = network.stop(1).expect("Unable to stop second node");
    node_a = network.node(0).expect("Unable to get first node");
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
        AuthorizationType::Trust,
    )
    .expect("Unable to generate circuit request");
    // Submit the `CircuitManagementPayload` to the first node
    node_a
        .admin_service_client()
        .submit_admin_payload(circuit_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the second node in the network
    network = network.start(1).expect("Unable to start second node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");

    let node_a_event_client = node_a
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");
    let node_b_event_client = node_b
        .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
        .expect("Unable to get event client");

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
    // Stop the first node in the network
    network = network.stop(0).expect("Unable to stop first node");
    node_b = network.node(1).expect("Unable to get second node");
    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b_event.proposal().clone(),
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes)
        .expect("Unable to submit admin payload to admin service");
    // Restart the first node in the network
    network = network.start(0).expect("Unable to start first node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    let node_b_admin_pubkey = admin_pubkey(node_b);
    let node_a_event_client = node_a
        .admin_service_event_client(
            &format!("test_circuit_{}", &circuit_id),
            Some(*proposal_a_event.event_id()),
        )
        .expect("Unable to get event client");

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

/// Test that a 3-party circuit proposal may be submitted and committed to all nodes, while the
/// nodes are stopped throughout the process.
///
/// 1. Collect  the node information needed to create the `CircuitCreateRequest`
/// 2. Stop the second node in the network
/// 3. Create and submit a `CircuitCreateRequest` from the first node
/// 4. Restart the second node in the network
/// 5. Wait for the `ProposalSubmitted` event from each node's event client, validate the
///    corresponding proposal
/// 6. Stop the third node in the network
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 8. Restart the third node in the network
/// 9. Wait for the `CircuitProposalVote` event from each node's event client, validate the
///    corresponding proposal
/// 10. Stop the first node in the network
/// 11. Create and submit a `CircuitProposalVote` from the third node to accept the proposal
/// 12. Restart the first node in the network
/// 13. Wait for the `ProposalAccepted` event from each node's event client, validate the
///     corresponding proposal
/// 14. Wait for the `CircuitReady` event from each node's event client, validate the
///     corresponding proposal
/// 15. Verify the circuit is now available to each node, using `fetch_circuit`
#[test]
#[ignore]
pub fn test_3_party_circuit_creation_stop() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .ok();

    timeout(Duration::from_secs(300), || {
        // Start a 3-node network
        let mut network = Network::new()
            .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
            .add_nodes_with_defaults(3)
            .expect("Unable to start 3-node ActixWeb1 network");
        // Get the first node in the network
        let mut node_a = network.node(0).expect("Unable to get first node");
        // Get the second node in the network
        let mut node_b = network.node(1).expect("Unable to get second node");
        // Get the third node in the network
        let mut node_c = network.node(2).expect("Unable to get third node");
        let circuit_id = "ABCDE-01234";
        let node_info = vec![
            (
                node_a.node_id().to_string(),
                (
                    node_a.network_endpoints().to_vec(),
                    node_a
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get first node's public key"),
                ),
            ),
            (
                node_b.node_id().to_string(),
                (
                    node_b.network_endpoints().to_vec(),
                    node_b
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get seconds node's public key"),
                ),
            ),
            (
                node_c.node_id().to_string(),
                (
                    node_c.network_endpoints().to_vec(),
                    node_c
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get third node's public key"),
                ),
            ),
        ]
        .into_iter()
        .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
        // Stop the second node in the network
        network = network.stop(1).expect("Unable to stop second node");
        node_a = network.node(0).expect("Unable to get first node");
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
            AuthorizationType::Trust,
        )
        .expect("Unable to generate circuit request");
        // Submit the `CircuitManagementPayload` to the first node
        node_a
            .admin_service_client()
            .submit_admin_payload(circuit_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        // Restart the second node in the network
        network = network.start(1).expect("Unable to start second node");
        node_a = network.node(0).expect("Unable to get first node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");

        let node_a_event_client = node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_b_event_client = node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_c_event_client = node_c
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
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

        // Stop the third node in the network
        network = network.stop(2).expect("Unable to stop third node");
        node_b = network.node(1).expect("Unable to get second node");
        // Create the `CircuitProposalVote` to be sent to a node
        // Uses `true` for the `accept` argument to create a vote to accept the proposal
        let vote_payload_bytes = make_circuit_proposal_vote_payload(
            proposal_b_event.proposal().clone(),
            node_b.node_id(),
            &*node_b.admin_signer().clone_box(),
            true,
        );
        node_b
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        // Restart the third node in the network
        network = network.start(2).expect("Unable to start third node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");
        let node_b_admin_pubkey = admin_pubkey(node_b);
        let node_c_event_client = node_c
            .admin_service_event_client(
                &format!("test_circuit_{}", &circuit_id),
                Some(*proposal_c_event.event_id()),
            )
            .expect("Unable to get event client");

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
        // Stop the first node in the network
        network = network.stop(0).expect("Unable to stop first node");
        node_c = network.node(2).expect("Unable to get third node");
        // Create the `CircuitProposalVote` to be sent to a node
        // Uses `true` for the `accept` argument to create a vote to accept the proposal
        let vote_payload_bytes = make_circuit_proposal_vote_payload(
            vote_c_event.proposal().clone(),
            node_c.node_id(),
            &*node_c.admin_signer().clone_box(),
            true,
        );
        node_c
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        //Restart the first node in the network
        network = network.start(0).expect("Unable to start third node");
        node_a = network.node(0).expect("Unable to get first node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");
        let node_c_admin_pubkey = admin_pubkey(node_c);
        let node_a_event_client = node_a
            .admin_service_event_client(
                &format!("test_circuit_{}", &circuit_id),
                Some(*vote_a_event.event_id()),
            )
            .expect("Unable to get event client");

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

        shutdown!(network).expect("Unable to shutdown network");
    })
}

/// Test that a 3-party circuit proposal may be submitted and then rejected and removed, while the
/// nodes are stopped throughout the process.
///
/// 1. Collect  the node information needed to create the `CircuitCreateRequest`
/// 2. Stop the second node in the network
/// 3. Create and submit a `CircuitCreateRequest` from the first node
/// 4. Restart the second node in the network
/// 5. Wait for the `ProposalSubmitted` event from each node's event client, validate the
///    corresponding proposal
/// 6. Stop the third node in the network
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 8. Restart the third node in the network
/// 9. Wait for the `CircuitProposalVote` event from each node's event client, validate the
///    corresponding proposal
/// 10. Stop the first node in the network
/// 11. Create and submit a `CircuitProposalVote` from the third node to accept the proposal
/// 12. Restart the first node in the network
/// 13. Wait for the `ProposalRejected` event from each node's event client, validate the
///     corresponding proposal
/// 14. Verify the circuit proposal is no longer available to any nodes
#[test]
#[ignore]
pub fn test_3_party_circuit_proposal_rejected_stop() {
    tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::TRACE)
        .try_init()
        .ok();

    timeout(Duration::from_secs(300), || {
        // Start a 3-node network
        let mut network = Network::new()
            .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
            .add_nodes_with_defaults(3)
            .expect("Unable to start 3-node ActixWeb1 network");
        // Get the first node in the network
        let mut node_a = network.node(0).expect("Unable to get first node");
        // Get the second node in the network
        let mut node_b = network.node(1).expect("Unable to get second node");
        // Get the third node in the network
        let mut node_c = network.node(2).expect("Unable to get third node");
        let circuit_id = "ABCDE-01234";
        let node_info = vec![
            (
                node_a.node_id().to_string(),
                (
                    node_a.network_endpoints().to_vec(),
                    node_a
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get first node's public key"),
                ),
            ),
            (
                node_b.node_id().to_string(),
                (
                    node_b.network_endpoints().to_vec(),
                    node_b
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get seconds node's public key"),
                ),
            ),
            (
                node_c.node_id().to_string(),
                (
                    node_c.network_endpoints().to_vec(),
                    node_c
                        .admin_signer()
                        .clone()
                        .public_key()
                        .expect("Unable to get third node's public key"),
                ),
            ),
        ]
        .into_iter()
        .collect::<HashMap<String, (Vec<String>, cylinder::PublicKey)>>();
        // Stop the second node in the network
        network = network.stop(1).expect("Unable to stop second node");
        node_a = network.node(0).expect("Unable to get first node");
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
            AuthorizationType::Trust,
        )
        .expect("Unable to generate circuit request");
        // Submit the `CircuitManagementPayload` to the first node
        node_a
            .admin_service_client()
            .submit_admin_payload(circuit_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        // Restart the second node in the network
        network = network.start(1).expect("Unable to start second node");
        node_a = network.node(0).expect("Unable to get first node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");

        let node_a_event_client = node_a
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_b_event_client = node_b
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
        let node_c_event_client = node_c
            .admin_service_event_client(&format!("test_circuit_{}", &circuit_id), None)
            .expect("Unable to get event client");
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

        // Stop the third node in the network
        network = network.stop(2).expect("Unable to stop third node");
        node_b = network.node(1).expect("Unable to get second node");
        // Create the `CircuitProposalVote` to be sent to a node
        // Uses `true` for the `accept` argument to create a vote to accept the proposal
        let vote_payload_bytes = make_circuit_proposal_vote_payload(
            proposal_a_event.proposal().clone(),
            node_b.node_id(),
            &*node_b.admin_signer().clone_box(),
            true,
        );
        node_b
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        // Restart the third node in the network
        network = network.start(2).expect("Unable to start third node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");
        let node_b_admin_pubkey = admin_pubkey(node_b);
        let node_c_event_client = node_c
            .admin_service_event_client(
                &format!("test_circuit_{}", &circuit_id),
                Some(*proposal_c_event.event_id()),
            )
            .expect("Unable to get event client");

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
        // Stop the first node in the network
        network = network.stop(0).expect("Unable to stop first node");
        node_c = network.node(2).expect("Unable to get third node");
        // Create the `CircuitProposalVote` to be sent to a node
        // Uses `false` for the `accept` argument to create a vote to reject the proposal
        let vote_payload_bytes = make_circuit_proposal_vote_payload(
            vote_c_event.proposal().clone(),
            node_c.node_id(),
            &*node_c.admin_signer().clone_box(),
            true,
        );
        node_c
            .admin_service_client()
            .submit_admin_payload(vote_payload_bytes)
            .expect("Unable to submit admin payload to admin service");
        //Restart the first node in the network
        network = network.start(0).expect("Unable to start third node");
        node_a = network.node(0).expect("Unable to get first node");
        node_b = network.node(1).expect("Unable to get second node");
        node_c = network.node(2).expect("Unable to get third node");
        let node_c_admin_pubkey = admin_pubkey(node_c);
        let node_a_event_client = node_a
            .admin_service_event_client(
                &format!("test_circuit_{}", &circuit_id),
                Some(*vote_a_event.event_id()),
            )
            .expect("Unable to get event client");

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
    })
}

/// This test is designed to tickle issues where a two connections exist for a peer and one
/// must be removed.
///
/// 1. Request connection to unidentified peer from node_b to node_a. Do not wait for notification
///    of peer connection
/// 2. Immediately propose a circuit. This creates a good chance that two connection will be
///    created, where one needs to be closed
/// 3. Wait for the circuit to be created sucessfully
#[test]
pub fn test_2_party_circuit_duplicate_connection() {
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

    let peer_connector_b = node_b.peer_connector();
    let (tx, _notification_rx): (mpsc::Sender<TestEnum>, mpsc::Receiver<TestEnum>) =
        mpsc::channel();
    peer_connector_b
        .subscribe_sender(tx)
        .expect("Unable to get subscriber");

    let _peer_ref = peer_connector_b
        .add_unidentified_peer(
            node_a.network_endpoints()[0].to_string(),
            PeerAuthorizationToken::from_public_key(
                node_b
                    .signers()
                    .get(0)
                    .expect("node does not have enough signers configured")
                    .public_key()
                    .expect("Unable to get first node's public key")
                    .as_slice(),
            ),
        )
        .expect("Unable to request connection to peer by endpoint");

    commit_2_party_circuit(circuit_id, node_a, node_b, AuthorizationType::Challenge);
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

#[derive(PartialEq, Debug)]
enum TestEnum {
    Notification(PeerManagerNotification),
}
/// Converts `PeerManagerNotification` into `Test_Enum::Notification(PeerManagerNotification)`
impl From<PeerManagerNotification> for TestEnum {
    fn from(notification: PeerManagerNotification) -> Self {
        TestEnum::Notification(notification)
    }
}
