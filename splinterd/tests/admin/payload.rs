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

//! Provides functionality for building `CircuitManagementPayload`s, used in the admin service
//! integration tests.

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use cylinder::{PublicKey, Signer};
use openssl::hash::{hash, MessageDigest};
use protobuf::{Message, RepeatedField};
use sabre_sdk::protocol::payload::{
    CreateContractActionBuilder, CreateContractRegistryActionBuilder,
    CreateNamespaceRegistryActionBuilder, CreateNamespaceRegistryPermissionActionBuilder,
    ExecuteContractActionBuilder,
};
use splinter::admin::client::ProposalSlice;
use splinter::admin::messages::{
    AuthorizationType, CircuitProposalVote, CreateCircuitBuilder, DurabilityType, PersistenceType,
    RouteType, SplinterNode, SplinterNodeBuilder, SplinterService, SplinterServiceBuilder, Vote,
};
use splinter::error::InternalError;
use splinter::protos::admin::{
    CircuitAbandon, CircuitCreateRequest, CircuitDisbandRequest, CircuitManagementPayload,
    CircuitManagementPayload_Action, CircuitManagementPayload_Header,
};
use transact::protocol::batch::Batch;
use transact::protos::command::{
    AddEvent, BytesEntry, Command, CommandPayload, Command_CommandType, DeleteState, GetState,
    ReturnInternalError, ReturnInvalid, SetState,
};

/// Makes the `CircuitManagementPayload` to create a circuit and returns the bytes of this
/// payload
pub(in crate::admin) fn make_create_circuit_payload(
    circuit_id: &str,
    requester: &str,
    node_info: HashMap<String, (Vec<String>, PublicKey)>,
    signer: &dyn Signer,
    admin_keys: &[String],
    auth_type: AuthorizationType,
) -> Result<Vec<u8>, InternalError> {
    let circuit_request = setup_circuit(circuit_id, node_info, admin_keys, auth_type);
    complete_create_payload(requester, signer, circuit_request)
}

pub(in crate) fn complete_create_payload(
    requester: &str,
    signer: &dyn Signer,
    circuit_request: CircuitCreateRequest,
) -> Result<Vec<u8>, InternalError> {
    let serialized_action = circuit_request.write_to_bytes().map_err(|e| {
        InternalError::from_source_with_message(
            Box::new(e),
            "unable to serialize `CreateCircuitRequest`".to_string(),
        )
    })?;

    // Get the public key to set the `requester` field of the `CircuitManagementPayload` header
    let public_key = signer
        .public_key()
        .map_err(|e| {
            InternalError::from_source_with_message(
                Box::new(e),
                "unable to get signer's public key".to_string(),
            )
        })?
        .into_bytes();
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action).map_err(|e| {
        InternalError::from_source_with_message(
            Box::new(e),
            "unable to hash `CircuitCreateRequest` bytes".to_string(),
        )
    })?;

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
    header.set_requester(public_key);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(requester.to_string());

    let mut payload = CircuitManagementPayload::new();
    payload.set_signature(
        signer
            .sign(&payload.header)
            .map_err(|e| {
                InternalError::from_source_with_message(
                    Box::new(e),
                    "unable to sign `CircuitManagementPayload` header".to_string(),
                )
            })?
            .take_bytes(),
    );
    payload.set_circuit_create_request(circuit_request);
    payload.set_header(Message::write_to_bytes(&header).map_err(|e| {
        InternalError::from_source_with_message(
            Box::new(e),
            "unable to serialize payload header".to_string(),
        )
    })?);

    let bytes = Message::write_to_bytes(&payload).map_err(|e| {
        InternalError::from_source_with_message(
            Box::new(e),
            "unable to serialize `CircuitManagementPayload`".to_string(),
        )
    })?;

    // Return the bytes of the payload
    Ok(bytes)
}

/// Makes the `CircuitProposalVote` payload to either accept or reject the proposal (based on
/// the `accept` argument) and returns the bytes of this payload
pub(in crate) fn make_circuit_proposal_vote_payload(
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
        circuit_hash: proposal.circuit_hash,
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
            .expect("Unable to sign `CircuitManagementPayload` header")
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

/// Makes the `CircuitManagementPayload` to abandon a circuit and returns the bytes of this
/// payload
pub(in crate::admin) fn make_circuit_abandon_payload(
    circuit_id: &str,
    requester_node_id: &str,
    signer: &dyn Signer,
) -> Vec<u8> {
    // Get the public key to create the `CircuitAbandon` and to also set the `requester`
    // field of the `CircuitManagementPayload` header
    let public_key = signer
        .public_key()
        .expect("Unable to get signer's public key")
        .into_bytes();
    let mut circuit_abandon = CircuitAbandon::new();
    circuit_abandon.set_circuit_id(circuit_id.to_string());

    let serialized_action = circuit_abandon
        .write_to_bytes()
        .expect("Unable to serialize `CircuitAbandon`");
    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)
        .expect("Unable to hash `CircuitAbandon` bytes");

    let mut header = CircuitManagementPayload_Header::new();
    header.set_action(CircuitManagementPayload_Action::CIRCUIT_ABANDON);
    header.set_requester(public_key);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(requester_node_id.to_string());

    let mut payload = CircuitManagementPayload::new();
    payload.set_signature(
        signer
            .sign(&payload.header)
            .expect("Unable to sign `CircuitManagementPayload` header")
            .take_bytes(),
    );
    payload.set_circuit_abandon(circuit_abandon);
    payload
        .set_header(Message::write_to_bytes(&header).expect("Unable to serialize payload header"));
    // Return the bytes of the payload
    payload
        .write_to_bytes()
        .expect("Unable to get bytes from `CircuitAbandon` payload")
}

/// Creates the `CircuitCreateRequest` for the `CircuitManagementPayload` to propose a circuit
fn setup_circuit(
    circuit_id: &str,
    node_info: HashMap<String, (Vec<String>, PublicKey)>,
    admin_keys: &[String],
    auth_type: AuthorizationType,
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
        .map(|(node_id, (endpoints, public_key))| {
            let mut builder = SplinterNodeBuilder::new()
                .with_node_id(&node_id)
                .with_endpoints(endpoints);

            if auth_type == AuthorizationType::Challenge {
                builder = builder.with_public_key(public_key.as_slice())
            }

            builder.build().expect("Unable to build SplinterNode")
        })
        .collect();

    let create_circuit_message = CreateCircuitBuilder::new()
        .with_circuit_id(circuit_id)
        .with_roster(&services)
        .with_members(&nodes)
        .with_authorization_type(&auth_type)
        .with_persistence(&PersistenceType::Any)
        .with_durability(&DurabilityType::NoDurability)
        .with_routes(&RouteType::Any)
        .with_circuit_management_type(&format!("test_circuit_{}", &circuit_id))
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

pub(in crate::admin) fn make_upload_contract_batch(
    name: &str,
    version: &str,
    prefix: &str,
    path: &str,
    signer: &dyn Signer,
) -> Result<Batch, InternalError> {
    let contract_path = Path::new(path);
    let contract_file =
        File::open(contract_path).map_err(|err| InternalError::from_source(Box::new(err)))?;
    let mut buf_reader = std::io::BufReader::new(contract_file);
    let mut contract = Vec::new();
    buf_reader
        .read_to_end(&mut contract)
        .map_err(|err| InternalError::from_source(Box::new(err)))?;

    CreateContractActionBuilder::new()
        .with_name(name.into())
        .with_version(version.into())
        .with_inputs(vec![prefix.into()])
        .with_outputs(vec![prefix.into()])
        .with_contract(contract)
        .into_payload_builder()
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_transaction_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_batch_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .build(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))
}

pub(in crate::admin) fn make_namespace_create_batch(
    prefix: &str,
    signer: &dyn Signer,
) -> Result<Batch, InternalError> {
    let owners = vec![signer
        .public_key()
        .expect("Unable to get signer's public key")
        .as_hex()];
    CreateNamespaceRegistryActionBuilder::new()
        .with_namespace(prefix.into())
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

pub(in crate::admin) fn make_namespace_permissions_batch(
    name: &str,
    prefix: &str,
    signer: &dyn Signer,
) -> Result<Batch, InternalError> {
    CreateNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(prefix.into())
        .with_contract_name(name.into())
        .with_read(true)
        .with_write(true)
        .into_payload_builder()
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_transaction_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .into_batch_builder(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))?
        .build(signer)
        .map_err(|err| InternalError::from_source(Box::new(err)))
}

pub(in crate::admin) fn make_command_batch(
    command_type: &str,
    address: String,
    signer: &dyn Signer,
) -> Result<Batch, InternalError> {
    let mut command = Command::new();

    match command_type {
        "set-state" => {
            let mut bytes_entry = BytesEntry::new();
            bytes_entry.set_key(address.clone());
            bytes_entry.set_value("state_value".to_string().as_bytes().to_vec());

            let state_writes = vec![bytes_entry];

            let mut set_state = SetState::new();
            set_state.set_state_writes(RepeatedField::from_vec(state_writes));

            command.set_command_type(Command_CommandType::SET_STATE);
            command.set_set_state(set_state);
        }
        "get-state" => {
            let mut get_state = GetState::new();
            get_state.set_state_keys(RepeatedField::from_vec(vec![address.clone()]));

            command.set_command_type(Command_CommandType::GET_STATE);
            command.set_get_state(get_state);
        }
        "delete-state" => {
            let mut delete_state = DeleteState::new();
            delete_state.set_state_keys(RepeatedField::from_vec(vec![address.clone()]));

            command.set_command_type(Command_CommandType::DELETE_STATE);
            command.set_delete_state(delete_state);
        }
        "add-event" => {
            let mut bytes_entry = BytesEntry::new();
            bytes_entry.set_key("event_key".to_string());
            bytes_entry.set_value("event_value".to_string().as_bytes().to_vec());

            let mut add_event = AddEvent::new();

            add_event.set_event_type("event_type".to_string());
            add_event.set_attributes(RepeatedField::from_vec(vec![bytes_entry]));
            add_event.set_data(format!("data{}", address.clone()).as_bytes().to_vec());

            command.set_command_type(Command_CommandType::ADD_EVENT);
            command.set_add_event(add_event);
        }
        "return-invalid" => {
            let mut return_invalid = ReturnInvalid::new();
            return_invalid
                .set_error_message("'return_invalid' command mock error message".to_string());

            let mut command = Command::new();
            command.set_command_type(Command_CommandType::RETURN_INVALID);
            command.set_return_invalid(return_invalid);
        }
        "return-internal-error" => {
            let mut return_internal_error = ReturnInternalError::new();
            return_internal_error.set_error_message(
                "'return_internal_error' command mock error message".to_string(),
            );

            let mut command = Command::new();
            command.set_command_type(Command_CommandType::RETURN_INTERNAL_ERROR);
            command.set_return_internal_error(return_internal_error);
        }
        command => {
            return Err(InternalError::with_message(format!(
                "command type '{}' does not exist",
                command
            )))
        }
    }

    let mut command_payload = CommandPayload::new();
    command_payload.set_commands(RepeatedField::from_vec(vec![command]));

    let payload_bytes = command_payload
        .write_to_bytes()
        .expect("Unable to get bytes from Command Payload");

    Ok(ExecuteContractActionBuilder::new()
        .with_name(String::from("command"))
        .with_version(String::from("1.0"))
        .with_inputs(vec![address.clone()])
        .with_outputs(vec![address.clone()])
        .with_payload(payload_bytes)
        .into_payload_builder()
        .expect("Unable to create payload builder")
        .into_transaction_builder(signer)
        .expect("Unable to create transaction builder")
        .into_batch_builder(signer)
        .expect("Unable to create batch builder")
        .build(signer)
        .expect("Unable to build txn"))
}
