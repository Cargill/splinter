// Copyright 2020 Cargill Incorporated
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

use cylinder::Signer;
use openssl::hash::{hash, MessageDigest};
use protobuf::Message;
use splinter::admin::messages::CreateCircuit;
#[cfg(feature = "circuit-abandon")]
use splinter::protos::admin::CircuitAbandon;
use splinter::protos::admin::{
    CircuitCreateRequest, CircuitDisbandRequest, CircuitManagementPayload,
    CircuitManagementPayload_Action as Action, CircuitManagementPayload_Header as Header,
    CircuitProposalVote, CircuitProposalVote_Vote, CircuitPurgeRequest,
};

use crate::error::CliError;

#[cfg(feature = "circuit-abandon")]
use super::AbandonedCircuit;
use super::{CircuitDisband, CircuitPurge};
use super::{CircuitVote, Vote};

/// A circuit action that has a type and can be converted into a protobuf-serializable struct.
pub trait CircuitAction<M: Message> {
    fn into_proto(self) -> Result<M, CliError>;

    fn action_type(&self) -> Action;
}

/// Applies a circuit payload action to the given CircuitManagementPayload.
pub trait ApplyToEnvelope {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload);
}

/// Makes a signed, circuit management payload to be submitted to the Splinter REST API.
pub fn make_signed_payload<M, A>(
    requester_node: &str,
    signer: Box<dyn Signer>,
    action: A,
) -> Result<Vec<u8>, CliError>
where
    M: Message + ApplyToEnvelope,
    A: CircuitAction<M>,
{
    let action_type = action.action_type();
    let action_proto = action.into_proto()?;
    let serialized_action = action_proto
        .write_to_bytes()
        .map_err(|err| CliError::ActionError(format!("Failed to serialize action: {}", err)))?;

    let hashed_bytes = hash(MessageDigest::sha512(), &serialized_action)?;

    let public_key = signer
        .public_key()
        .map_err(|err| {
            CliError::ActionError(format!(
                "Failed to get public key from secp256k1 private key: {}",
                err
            ))
        })?
        .into_bytes();

    let mut header = Header::new();
    header.set_action(action_type);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester(public_key);
    header.set_requester_node_id(requester_node.into());
    let header_bytes = header.write_to_bytes().map_err(|err| {
        CliError::ActionError(format!("Failed to serialize payload header: {}", err))
    })?;

    let header_signature = signer
        .sign(&header_bytes)
        .map_err(|err| CliError::ActionError(format!("Failed to sign payload header: {}", err)))?;

    let mut circuit_management_payload = CircuitManagementPayload::new();
    circuit_management_payload.set_header(header_bytes);
    circuit_management_payload.set_signature(header_signature.take_bytes());
    action_proto.apply(&mut circuit_management_payload);
    let payload_bytes = circuit_management_payload
        .write_to_bytes()
        .map_err(|err| CliError::ActionError(format!("Failed to serialize payload: {}", err)))?;
    Ok(payload_bytes)
}

// Conversions for explicit actions and their associated types.

impl CircuitAction<CircuitCreateRequest> for CreateCircuit {
    fn action_type(&self) -> Action {
        Action::CIRCUIT_CREATE_REQUEST
    }

    fn into_proto(self) -> Result<CircuitCreateRequest, CliError> {
        CreateCircuit::into_proto(self).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to convert circuit create request to protobuf: {}",
                err
            ))
        })
    }
}

impl ApplyToEnvelope for CircuitCreateRequest {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload) {
        circuit_management_payload.set_circuit_create_request(self);
    }
}

impl CircuitAction<CircuitProposalVote> for CircuitVote {
    fn action_type(&self) -> Action {
        Action::CIRCUIT_PROPOSAL_VOTE
    }

    fn into_proto(self) -> Result<CircuitProposalVote, CliError> {
        let mut vote = CircuitProposalVote::new();
        vote.set_vote(match self.vote {
            Vote::Accept => CircuitProposalVote_Vote::ACCEPT,
            Vote::Reject => CircuitProposalVote_Vote::REJECT,
        });
        vote.set_circuit_id(self.circuit_id);
        vote.set_circuit_hash(self.circuit_hash);

        Ok(vote)
    }
}

impl ApplyToEnvelope for CircuitProposalVote {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload) {
        circuit_management_payload.set_circuit_proposal_vote(self);
    }
}

impl CircuitAction<CircuitDisbandRequest> for CircuitDisband {
    fn action_type(&self) -> Action {
        Action::CIRCUIT_DISBAND_REQUEST
    }

    fn into_proto(self) -> Result<CircuitDisbandRequest, CliError> {
        let mut disband_request = CircuitDisbandRequest::new();
        disband_request.set_circuit_id(self.circuit_id);
        Ok(disband_request)
    }
}

impl ApplyToEnvelope for CircuitDisbandRequest {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload) {
        circuit_management_payload.set_circuit_disband_request(self);
    }
}

impl CircuitAction<CircuitPurgeRequest> for CircuitPurge {
    fn action_type(&self) -> Action {
        Action::CIRCUIT_PURGE_REQUEST
    }

    fn into_proto(self) -> Result<CircuitPurgeRequest, CliError> {
        let mut purge_request = CircuitPurgeRequest::new();
        purge_request.set_circuit_id(self.circuit_id);
        Ok(purge_request)
    }
}

impl ApplyToEnvelope for CircuitPurgeRequest {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload) {
        circuit_management_payload.set_circuit_purge_request(self);
    }
}

#[cfg(feature = "circuit-abandon")]
impl CircuitAction<CircuitAbandon> for AbandonedCircuit {
    fn action_type(&self) -> Action {
        Action::CIRCUIT_ABANDON
    }

    fn into_proto(self) -> Result<CircuitAbandon, CliError> {
        let mut abandon = CircuitAbandon::new();
        abandon.set_circuit_id(self.circuit_id);
        Ok(abandon)
    }
}

#[cfg(feature = "circuit-abandon")]
impl ApplyToEnvelope for CircuitAbandon {
    fn apply(self, circuit_management_payload: &mut CircuitManagementPayload) {
        circuit_management_payload.set_circuit_abandon(self);
    }
}
