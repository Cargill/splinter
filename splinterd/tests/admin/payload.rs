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

//! Provides functionality for building `CircuitManagmentPayload`s, used in the admin service
//! integration tests.

use std::collections::HashMap;

use cylinder::Signer;
use openssl::hash::{hash, MessageDigest};
use protobuf::Message;

use sabre_sdk::protocol::payload::CreateContractRegistryActionBuilder;
use splinter::admin::client::ProposalSlice;
use splinter::admin::messages::{
    AuthorizationType, CircuitProposalVote, CreateCircuitBuilder, DurabilityType, PersistenceType,
    RouteType, SplinterNode, SplinterNodeBuilder, SplinterService, SplinterServiceBuilder, Vote,
};
use splinter::error::InternalError;
use splinter::protos::admin::{
    CircuitCreateRequest, CircuitDisbandRequest, CircuitManagementPayload,
    CircuitManagementPayload_Action, CircuitManagementPayload_Header,
};
use transact::protocol::batch::Batch;

/// Makes the `CircuitManagementPayload` to create a circuit and returns the bytes of this
/// payload
pub(in crate::admin) fn make_create_circuit_payload(
    circuit_id: &str,
    requester: &str,
    node_info: HashMap<String, Vec<String>>,
    signer: &dyn Signer,
    admin_keys: &[String],
) -> Vec<u8> {
    // Get the public key to set the `requester` field of the `CircuitManagementPayload` header
    let public_key = signer
        .public_key()
        .expect("Unable to get signer's public key")
        .into_bytes();
    let circuit_request = setup_circuit(circuit_id, node_info, admin_keys);
    let serialized_action = circuit_request
        .write_to_bytes()
        .expect("Unable to serialize `CircuitCreateRequest`");
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)
        .expect("Unable to hash `CircuitCreateRequest` bytes");

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
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
    payload.set_circuit_create_request(circuit_request);
    payload
        .set_header(Message::write_to_bytes(&header).expect("Unable to serialize payload header"));

    // Return the bytes of the payload
    Message::write_to_bytes(&payload).expect("Unable to serialize `CircuitManagmentPayload`")
}

/// Makes the `CircuitProposalVote` payload to either accept or reject the proposal (based on
/// the `accept` argument) and returns the bytes of this payload
pub(in crate::admin) fn make_circuit_proposal_vote_payload(
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
pub(in crate::admin) fn make_circuit_disband_payload(
    circuit_id: &str,
    requester: &str,
    signer: &dyn Signer,
) -> Vec<u8> {
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
    admin_keys: &[String],
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
                        (
                            "admin_keys".to_string(),
                            format!("{:?}", admin_keys.to_vec()),
                        ),
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

/// Create the bytes of a `CreateContractRegistryAction` batch
pub(in crate::admin) fn make_create_contract_registry_batch(
    name: &str,
    signer: &dyn Signer,
) -> Result<Batch, InternalError> {
    let owners = vec![signer
        .public_key()
        .expect("Unable to get signer's public key")
        .as_hex()];
    CreateContractRegistryActionBuilder::new()
        .with_name(name.into())
        .with_owners(owners)
        .into_payload_builder()
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_transaction_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_batch_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .build(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))
}
