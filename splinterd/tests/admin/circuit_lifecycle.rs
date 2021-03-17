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

//! Integration tests for the lifecycle of a circuit between multiple nodes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use cylinder::Signer;
use openssl::hash::{hash, MessageDigest};
use protobuf::Message;

use crate::framework::network::Network;
use splinter::admin::client::ProposalSlice;
use splinter::admin::messages::{
    AuthorizationType, CircuitProposalVote, CreateCircuitBuilder, DurabilityType, PersistenceType,
    RouteType, SplinterNode, SplinterNodeBuilder, SplinterService, SplinterServiceBuilder, Vote,
};
use splinter::protos::admin::{
    CircuitCreateRequest, CircuitDisbandRequest, CircuitManagementPayload,
    CircuitManagementPayload_Action, CircuitManagementPayload_Header,
};
use splinterd::node::{Node, RestApiVariant};

/// Makes the `CircuitManagementPayload` to create a circuit and returns the bytes of this
/// payload
fn make_create_circuit_payload(
    circuit_id: &str,
    requester: &str,
    node_info: HashMap<String, Vec<String>>,
    signer: &dyn Signer,
) -> Vec<u8> {
    // Get the public key to create the `CircuitCreateRequest` and to also set the `requester`
    // field of the `CircuitManagementPayload` header
    let public_key = signer
        .public_key()
        .expect("Unable to get signer's public key");
    let circuit_request = setup_circuit(circuit_id, node_info, &public_key.as_hex());
    let serialized_action = circuit_request
        .write_to_bytes()
        .expect("Unable to serialize `CircuitCreateRequest`");
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)
        .expect("Unable to hash `CircuitCreateRequest` bytes");

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
    header.set_requester(public_key.into_bytes());
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(requester.to_string());

    let mut payload = CircuitManagementPayload::new();
    payload.set_signature(
        signer
            .sign(&payload.header)
            .expect("Unable to sign `CircuitManagementPayload` header")
            .take_bytes(),
    );
    payload.set_circuit_create_request(circuit_request);
    payload
        .set_header(Message::write_to_bytes(&header).expect("Unable to serialize payload header"));

    // Return the bytes of the payload
    Message::write_to_bytes(&payload).expect("Unable to serialize `CircuitManagmentPayload`")
}

/// Makes the `CircuitProposalVote` payload to either accept or reject the proposal (based on
/// the `accept` argument) and returns the bytes of this payload
fn make_circuit_proposal_vote_payload(
    proposal: ProposalSlice,
    requester: &str,
    signer: &dyn Signer,
    accept: bool,
) -> Vec<u8> {
    // Get the public key necessary to set the `requester` field of the payload's header
    let public_key = signer
        .public_key()
        .expect("Unable to get signer's public key")
        .into_bytes();
    let vote = if accept { Vote::Accept } else { Vote::Reject };

    let vote_proto = CircuitProposalVote {
        circuit_id: proposal.circuit_id.to_string(),
        circuit_hash: proposal.circuit_hash.to_string(),
        vote,
    }
    .into_proto();

    let serialized_action = vote_proto
        .write_to_bytes()
        .expect("Unable to serialize `CircuitProposalVote`");
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)
        .expect("Unable to hash `CircuitProposalVote` bytes");

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE);
    header.set_requester(public_key);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(requester.to_string());

    let mut payload = CircuitManagementPayload::new();
    payload.set_signature(
        signer
            .sign(&payload.header)
            .expect("Unable to sign `CircuitManagementPayload` header")
            .take_bytes(),
    );
    payload.set_circuit_proposal_vote(vote_proto);
    payload
        .set_header(Message::write_to_bytes(&header).expect("Unable to serialize payload header"));
    // Return the bytes of the payload
    payload
        .write_to_bytes()
        .expect("Unable to get bytes from CircuitProposalVote payload")
}

/// Makes the `CircuitManagementPayload` to disband a circuit and returns the bytes of this
/// payload
fn make_circuit_disband_payload(circuit_id: &str, requester: &str, signer: &dyn Signer) -> Vec<u8> {
    let public_key = signer
        .public_key()
        .expect("Unable to get signer's public key")
        .into_bytes();
    let mut disband_request = CircuitDisbandRequest::new();
    disband_request.set_circuit_id(circuit_id.to_string());

    let serialized_action = disband_request
        .write_to_bytes()
        .expect("Unable to serialize `CircuitDisbandRequest`");
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)
        .expect("Unable to hash `CircuitDisbandRequest` bytes");

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST);
    header.set_requester(public_key);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(requester.to_string());

    let mut payload = CircuitManagementPayload::new();
    payload.set_signature(
        signer
            .sign(&payload.header)
            .expect("Unable to sign `CircuitManagmentPayload` header")
            .take_bytes(),
    );
    payload.set_circuit_disband_request(disband_request);
    payload
        .set_header(Message::write_to_bytes(&header).expect("Unable to serialize payload header"));
    // Return the bytes of the payload
    payload
        .write_to_bytes()
        .expect("Unable to get bytes from `CircuitDisbandRequest` payload")
}

/// Creates the `CircuitCreateRequest` for the `CircuitManagementPayload` to propose a circuit
fn setup_circuit(
    circuit_id: &str,
    node_info: HashMap<String, Vec<String>>,
    public_key: &str,
) -> CircuitCreateRequest {
    // The services require the service IDs from its peer services, which will be generated
    // after the node information is iterated over and the `SplinterServiceBuilder` is created
    // (with the generated service ID). Afterwards, the peer services may be added to the
    // service builders. Maps the service builder to the service ID, in order to iterate back
    // over the other services to collect the service ids.
    let mut service_builders: Vec<(String, SplinterServiceBuilder)> = vec![];
    let mut service_ids: Vec<String> = vec![];
    for (idx, node_id) in node_info.keys().enumerate() {
        let service_id = format!("sc{:0>2}", idx);
        service_ids.push(service_id.clone());
        let builder = SplinterServiceBuilder::new()
            .with_service_id(service_id.as_ref())
            .with_service_type("scabbard")
            .with_allowed_nodes(vec![node_id.to_string()].as_ref());
        service_builders.push((service_id, builder));
    }
    let services: Vec<SplinterService> = service_builders
        .into_iter()
        .map(|(service_id, builder)| {
            let peer_services = service_ids
                .iter()
                .filter(|peer_service_id| peer_service_id != &&service_id)
                .collect::<Vec<&String>>();
            builder
                .with_arguments(
                    vec![
                        ("peer_services".to_string(), format!("{:?}", peer_services)),
                        ("admin_keys".to_string(), format!("{:?}", vec![public_key])),
                    ]
                    .as_ref(),
                )
                .build()
                .expect("Unable to build SplinterService")
        })
        .collect::<Vec<SplinterService>>();

    let nodes: Vec<SplinterNode> = node_info
        .iter()
        .map(|(node_id, endpoints)| {
            SplinterNodeBuilder::new()
                .with_node_id(&node_id)
                .with_endpoints(endpoints)
                .build()
                .expect("Unable to build SplinterNode")
        })
        .collect();

    let create_circuit_message = CreateCircuitBuilder::new()
        .with_circuit_id(circuit_id)
        .with_roster(&services)
        .with_members(&nodes)
        .with_authorization_type(&AuthorizationType::Trust)
        .with_persistence(&PersistenceType::Any)
        .with_durability(&DurabilityType::NoDurability)
        .with_routes(&RouteType::Any)
        .with_circuit_management_type("test_circuit")
        .with_application_metadata(b"test_data")
        .with_comments("test circuit")
        .with_display_name("test_circuit")
        .with_circuit_version(2)
        .build()
        .expect("Unable to build `CreateCircuit`");
    create_circuit_message
        .into_proto()
        .expect("Unable to get proto from `CreateCircuit`")
}

/// Commit a 2-party circuit on a network that is already running
/// This function also validates that a duplicate proposal of the circuit being created is
/// rejected when submitted.
fn commit_2_party_circuit(circuit_id: &str, node_a: &Node, node_b: &Node) {
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
fn commit_3_party_circuit(circuit_id: &str, node_a: &Node, node_b: &Node, node_c: &Node) {
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
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b);

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

    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first proposal in this list as we've already validated the list is
            // not empty
            proposal_b = proposals.get(0).unwrap().clone();
        } else {
            continue;
        }

        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal) => proposal,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Submit a duplicate of the disband `CircuitManagementPayload` to the second node
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(duplicate_res.is_err());

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be removed from the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
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
        if circuits_a.is_empty() && circuits_b.is_empty() {
            break;
        } else {
            continue;
        }
    }

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
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b);

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

    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first proposal in this list as we've already validated the list is
            // not empty
            proposal_b = proposals.get(0).unwrap().clone();
        } else {
            continue;
        }

        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal_a) => proposal_a,
            None => continue,
        };

        assert_eq!(proposal_b, proposal_a);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed from the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        // If the proposal no longer appears in the list, the proposal has been removed as it was
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
        if proposals_a.is_empty() && proposals_b.is_empty() {
            break;
        }
    }

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
    // Get the third node from the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

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

    // Wait for the disband proposal to be committed for the second and third node
    let mut proposal_b;
    let mut proposal_c;
    let start = Instant::now();
    loop {
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
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

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
        proposal_b,
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
            panic!("Failed to detect proposal in time");
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
        if proposal_a.votes.is_empty() && proposal_b.votes.is_empty() && proposal_c.votes.is_empty()
        {
            continue;
        } else {
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
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

    // Wait for the circuit to be removed for the other nodes
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
            .expect("Unable to list circuits from second node")
            .data;
        let circuits_c = node_c
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from third node")
            .data;

        if circuits_a.is_empty() && circuits_b.is_empty() && circuits_c.is_empty() {
            break;
        }
    }
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
    // Get the third node in the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

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

    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let mut proposal_c;
    let start = Instant::now();
    loop {
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
            Some(proposal) => proposal,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_b, proposal_c);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
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
            panic!("Failed to detect proposal in time");
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
        if proposal_a.votes.is_empty() && proposal_b.votes.is_empty() && proposal_c.votes.is_empty()
        {
            continue;
        } else {
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
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

    // Wait for the disband proposal to be removed for the other nodes
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
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
        }
    }

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
