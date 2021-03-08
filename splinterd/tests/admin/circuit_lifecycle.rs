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

//! Integration tests for creating a circuit between multiple nodes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use cylinder::{secp256k1::Secp256k1Context, Context, Signer};
use openssl::hash::{hash, MessageDigest};
use protobuf::Message;

use crate::framework::network::Network;
use splinter::admin::client::{AdminServiceClient, ProposalSlice};
use splinter::admin::messages::{
    AuthorizationType, CircuitProposalVote, CreateCircuitBuilder, DurabilityType, PersistenceType,
    RouteType, SplinterNode, SplinterNodeBuilder, SplinterService, SplinterServiceBuilder, Vote,
};
use splinter::protos::admin::{
    CircuitCreateRequest, CircuitManagementPayload, CircuitManagementPayload_Action,
    CircuitManagementPayload_Header,
};
use splinterd::node::RestApiVariant;

/// Makes the `CircuitManagementPayload` to create a circuit and returns the bytes of this
/// payload
fn make_create_circuit_payload(
    circuit_id: &str,
    requester: &str,
    node_info: HashMap<String, String>,
    signer: Box<dyn Signer>,
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
    signer: Box<dyn Signer>,
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

/// Creates the `CircuitCreateRequest` for the `CircuitManagementPayload` to propose a circuit
fn setup_circuit(
    circuit_id: &str,
    node_info: HashMap<String, String>,
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
        let service_id = format!("sc0{}", idx);
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
        .map(|(node_id, endpoint)| {
            SplinterNodeBuilder::new()
                .with_node_id(node_id)
                .with_endpoints(vec![endpoint.to_string()].as_ref())
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
        .build()
        .expect("Unable to build `CreateCircuit`");
    create_circuit_message
        .into_proto()
        .expect("Unable to get proto from `CreateCircuit`")
}

/// Commit a 2-party circuit on a network that is already running
fn commit_2_party_circuit(
    circuit_id: &str,
    node_a_client: &Box<dyn AdminServiceClient>,
    node_a_signer: Box<dyn Signer>,
    node_b_client: &Box<dyn AdminServiceClient>,
    node_b_signer: Box<dyn Signer>,
    node_info: HashMap<String, String>,
) {
    let circuit_payload_bytes =
        make_create_circuit_payload(&circuit_id, "node_a", node_info.clone(), node_a_signer);
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a_client.submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the second node
    let proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_b")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first proposal in this list as we've already validated the list is
            // not empty
            proposal_b = proposals.get(0).unwrap().clone();
            break;
        }
    }

    // Validate the same proposal is available to the first node
    let proposal_a = node_a_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_a")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_b", node_b_signer, true);
    let res = node_b_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be committed for the second node
    let circuit_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let circuits = node_b_client
            .list_circuits(None)
            .expect("Unable to list circuits from node_b")
            .data;
        if !circuits.is_empty() {
            // Unwrap the first element in this list as we've already validated that the list
            // is not empty
            circuit_b = circuits.get(0).unwrap().clone();
            break;
        }
    }

    // Validate the circuit is available to the first node
    let circuit_a = node_a_client
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from node_a")
        .unwrap();
    assert_eq!(circuit_a, circuit_b);
}

/// Commit a 3-party circuit on a network that is already running
fn commit_3_party_circuit(
    circuit_id: &str,
    node_a_client: &Box<dyn AdminServiceClient>,
    node_a_signer: Box<dyn Signer>,
    node_b_client: &Box<dyn AdminServiceClient>,
    node_b_signer: Box<dyn Signer>,
    node_c_client: &Box<dyn AdminServiceClient>,
    node_c_signer: Box<dyn Signer>,
    node_info: HashMap<String, String>,
) {
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes =
        make_create_circuit_payload(&circuit_id, "node_a", node_info.clone(), node_a_signer);
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a_client.submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the remote nodes
    let proposal_b;
    let proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals_b = node_b_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_b")
            .data;
        let proposals_c = node_c_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_c")
            .data;
        if !(proposals_b.is_empty() && proposals_c.is_empty()) {
            // Unwrap the first elements in each list as we've already validated that both of
            // the lists are not empty
            proposal_b = proposals_b.get(0).unwrap().clone();
            proposal_c = proposals_c.get(0).unwrap().clone();
            break;
        }
    }
    // Validate the same proposal is available to the first node
    let proposal_a = node_a_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_a")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);
    assert_eq!(proposal_b, proposal_c);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_b", node_b_signer, true);
    let res = node_b_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the vote from this node to appear on the proposal for the remote nodes
    let mut proposal_a;
    let mut proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal vote in time");
        }
        // The proposal should already be available to each of these nodes, so we are able to
        // unwrap the result of the `fetch_proposal` call
        proposal_a = node_a_client
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_a")
            .unwrap();
        proposal_c = node_c_client
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_c")
            .unwrap();
        if proposal_a.votes.len() == 2 && proposal_c.votes.len() == 2 {
            break;
        }
    }
    // Validate the extra vote records are also available for the voting node
    let proposal_b = node_b_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_b")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);
    assert_eq!(proposal_b, proposal_c);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_c", node_c_signer, true);
    let res = node_c_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be committed for the other nodes
    let circuit_a;
    let circuit_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let circuits_a = node_a_client
            .list_circuits(None)
            .expect("Unable to list circuits from node_a")
            .data;
        let circuits_b = node_b_client
            .list_circuits(None)
            .expect("Unable to list circuits from node_b")
            .data;
        if !(circuits_a.is_empty() && circuits_b.is_empty()) {
            // Unwrap the first element in each list as we've already validated each of the
            // lists are not empty
            circuit_a = circuits_a.get(0).unwrap().clone();
            circuit_b = circuits_b.get(0).unwrap().clone();
            break;
        }
    }

    // Validate the circuit is available to the first node
    let circuit_c = node_c_client
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from node_c")
        .unwrap();
    assert_eq!(circuit_a, circuit_b);
    assert_eq!(circuit_b, circuit_c);
}

/// Test that a 2-party circuit may be created on a 2-node network.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to the second node, using `list_proposals`
/// 3. Verify the same proposal is available on each node
/// 4. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 5. Wait until the circuit is available on the first node, using `list_circuits`
/// 6. Verify the same circuit is available to each node
#[test]
#[ignore]
pub fn test_2_party_circuit_creation() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node_actixWeb1 network");
    // Create a context in order to produce private keys for the nodes
    let context = Secp256k1Context::new();
    // Collect the node information to be used to populate the payloads
    let mut node_info = HashMap::new();
    // Get the node and node's client for the first node
    let node_a = network.node(0).expect("Unable to get node_a");
    let node_a_client = node_a.admin_service_client();
    let node_a_signer = context.new_signer(context.new_random_private_key());
    // Get the node and node's client for the second node
    let node_b = network.node(1).expect("Unable to get node_b");
    let node_b_client = node_b.admin_service_client();
    let node_b_signer = context.new_signer(context.new_random_private_key());
    // Using `node_b` here for the second node as a placeholder
    node_info.insert(
        "node_b".to_string(),
        format!("http://localhost:{}", node_b.rest_api_port()),
    );
    let circuit_id = "ABCDE-01234";

    commit_2_party_circuit(
        circuit_id,
        &node_a_client,
        node_a_signer,
        &node_b_client,
        node_b_signer,
        node_info,
    );

    shutdown!(network).unwrap();
}

/// Test that a 3-party circuit may be created on a 3-node network.
///
/// 1. Create and submit a `CircuitCreateRequest` from the first node
/// 2. Wait until the proposal is available to one of the other nodes, using `list_proposals`
/// 3. Verify the same proposal is available on every node
/// 4. Create and submit a `CircuitProposalVote` from the second node to accept the proposal
/// 5. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal
/// 6. Validate the proposal has also been updated and includes the `Vote` submitted in the
///    previous steps for every node
/// 7. Create and submit a `CircuitProposalVote` from the third node to accept the proposal
/// 8. Wait until the circuit becomes available for one of the other nodes, using `list_circuits`
/// 9. Validate the circuit is available to every node
#[test]
#[ignore]
pub fn test_3_party_circuit_creation() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node_actixWeb1 network");
    // Create a context in order to produce private keys for the nodes
    let context = Secp256k1Context::new();
    // Collect the node information to be used to populate the payloads
    let mut node_info = HashMap::new();
    // Get the node and node's client for the first node
    let node_a = network.node(0).expect("Unable to get node_a");
    let node_a_client = node_a.admin_service_client();
    let node_a_signer = context.new_signer(context.new_random_private_key());
    // Using `node_a` here for the first node as a placeholder
    node_info.insert(
        "node_a".to_string(),
        format!("http://localhost:{}", node_a.rest_api_port()),
    );
    // Get the node and node's client for the second node
    let node_b = network.node(1).expect("Unable to get node_b");
    let node_b_client = node_b.admin_service_client();
    let node_b_signer = context.new_signer(context.new_random_private_key());
    // Using `node_b` here for the second node as a placeholder
    node_info.insert(
        "node_b".to_string(),
        format!("http://localhost:{}", node_b.rest_api_port()),
    );
    // Get the node and node's client for the third node
    let node_c = network.node(2).expect("Unable to get node_c");
    let node_c_client = node_c.admin_service_client();
    let node_c_signer = context.new_signer(context.new_random_private_key());
    // Using `node_c` here for the third node as a placeholder
    node_info.insert(
        "node_c".to_string(),
        format!("http://localhost:{}", node_c.rest_api_port()),
    );
    let circuit_id = "ABCDE-01234";

    commit_3_party_circuit(
        circuit_id,
        &node_a_client,
        node_a_signer,
        &node_b_client,
        node_b_signer,
        &node_c_client,
        node_c_signer,
        node_info,
    );

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
/// 5. Wait until the proposal is not available on the first node, using `list_proposals`
/// 6. Verify the proposal does not exist on the second node
#[test]
#[ignore]
pub fn test_2_party_circuit_creation_proposal_rejected() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node_actixWeb1 network");
    // Create a context in order to produce private keys for the nodes
    let context = Secp256k1Context::new();
    // Collect the node information to be used to populate the payloads
    let mut node_info = HashMap::new();
    // Get the node and node's client for the first node
    let node_a = network.node(0).expect("Unable to get node_a");
    let node_a_client = node_a.admin_service_client();
    let node_a_signer = context.new_signer(context.new_random_private_key());
    // Using `node_a` here for the first node as a placeholder
    node_info.insert(
        "node_a".to_string(),
        format!("http://localhost:{}", node_a.rest_api_port()),
    );
    // Get the node and node's client for the second node
    let node_b = network.node(1).expect("Unable to get node_b");
    let node_b_client = node_b.admin_service_client();
    let node_b_signer = context.new_signer(context.new_random_private_key());
    // Using `node_b` here for the second node as a placeholder
    node_info.insert(
        "node_b".to_string(),
        format!("http://localhost:{}", node_b.rest_api_port()),
    );
    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes =
        make_create_circuit_payload(&circuit_id, "node_a", node_info.clone(), node_a_signer);
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a_client.submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the second node
    let proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_b")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first item in the list as we've already validated this list is not empty
            proposal_b = proposals.get(0).unwrap().clone();
            break;
        }
    }
    // Validate the same proposal is available to the first node
    let proposal_a = node_a_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_a")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_b", node_b_signer, false);
    let res = node_b_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed for the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect removed proposal in time");
        }
        if node_a_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_a")
            .data
            .is_empty()
        {
            break;
        }
    }
    // Validate the proposal has been removed for the second node
    let proposals_slice_b = node_b_client
        .list_proposals(None, None)
        .expect("Unable to list proposals from node_b");
    assert!(proposals_slice_b.data.is_empty());

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
///    `fetch_proposal`
/// 9. Validate the proposal is no longer available for the node that voted to reject the proposal
#[test]
#[ignore]
pub fn test_3_party_circuit_creation_proposal_rejected() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node_actixWeb1 network");
    // Create a context in order to produce private keys for the nodes
    let context = Secp256k1Context::new();
    // Collect the node information to be used to populate the payloads
    let mut node_info = HashMap::new();
    // Get the node and node's client for the first node
    let node_a = network.node(0).expect("Unable to get node_a");
    let node_a_client = node_a.admin_service_client();
    let node_a_signer = context.new_signer(context.new_random_private_key());
    // Using `node_a` here for the first node as a placeholder
    node_info.insert(
        "node_a".to_string(),
        format!("http://localhost:{}", node_a.rest_api_port()),
    );
    // Get the node and node's client for the second node
    let node_b = network.node(1).expect("Unable to get node_b");
    let node_b_client = node_b.admin_service_client();
    let node_b_signer = context.new_signer(context.new_random_private_key());
    // Using `node_b` here for the second node as a placeholder
    node_info.insert(
        "node_b".to_string(),
        format!("http://localhost:{}", node_b.rest_api_port()),
    );
    // Get the node and node's client for the third node
    let node_c = network.node(2).expect("Unable to get node_c");
    let node_c_client = node_c.admin_service_client();
    let node_c_signer = context.new_signer(context.new_random_private_key());
    // Using `node_c` here for the third node as a placeholder
    node_info.insert(
        "node_c".to_string(),
        format!("http://localhost:{}", node_c.rest_api_port()),
    );
    let circuit_id = "ABCDE-01234";
    // Create the `CircuitManagementPayload` to be sent to a node
    let circuit_payload_bytes =
        make_create_circuit_payload(&circuit_id, "node_a", node_info.clone(), node_a_signer);
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a_client.submit_admin_payload(circuit_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be committed for the remote nodes
    let proposal_b;
    let proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals_b = node_b_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_b")
            .data;
        let proposals_c = node_c_client
            .list_proposals(None, None)
            .expect("Unable to list proposals from node_c")
            .data;
        if !(proposals_b.is_empty() && proposals_c.is_empty()) {
            // Unwrap the first element in each list as we've already validated the lists are not
            // empty
            proposal_b = proposals_b.get(0).unwrap().clone();
            proposal_c = proposals_c.get(0).unwrap().clone();
            break;
        }
    }
    // Validate the same proposal is available to the first node
    let proposal_a = node_a_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_a")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);
    assert_eq!(proposal_b, proposal_c);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_b", node_b_signer, true);
    let res = node_b_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the vote from this node to appear on the proposal for the remote nodes
    let mut proposal_a;
    let mut proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal vote in time");
        }
        // The proposal should already be available to each of these nodes, so we are able to
        // unwrap the result of the `fetch_proposal` call
        proposal_a = node_a_client
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_a")
            .unwrap();
        proposal_c = node_c_client
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_c")
            .unwrap();
        if proposal_a.votes.len() == 2 && proposal_c.votes.len() == 2 {
            break;
        }
    }
    // Validate the extra vote records are also available for the voting node
    let proposal_b = node_b_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_b")
        .unwrap();
    assert_eq!(proposal_a, proposal_b);
    assert_eq!(proposal_b, proposal_c);

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes =
        make_circuit_proposal_vote_payload(proposal_a, "node_c", node_c_signer, false);
    let res = node_c_client.submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed for the other nodes
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect removed proposal in time");
        }
        if node_a_client
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from node_a")
            .is_none()
            && node_b_client
                .fetch_proposal(&circuit_id)
                .expect("Unable to fetch proposal from node_b")
                .is_none()
        {
            break;
        }
    }
    let removed_proposal = node_c_client
        .fetch_proposal(&circuit_id)
        .expect("Unable to fetch proposal from node_c");
    assert!(removed_proposal.is_none());

    shutdown!(network).expect("Unable to shutdown network");
}
