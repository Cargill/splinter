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

use std::convert::TryFrom;
use std::convert::TryInto;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{Builder, JoinHandle};
use std::time::Duration;

use protobuf::{Message, RepeatedField};

use crate::consensus::two_phase::TwoPhaseEngine;
use crate::consensus::{
    error::{ConsensusSendError, ProposalManagerError},
    ConsensusMessage, ConsensusNetworkSender, PeerId, Proposal, ProposalId, ProposalManager,
    ProposalUpdate,
};
use crate::consensus::{ConsensusEngine, StartupState};
use crate::hex::to_hex;
use crate::protos::admin::{AdminMessage, AdminMessage_Type, ProposedCircuit};
use crate::protos::two_phase::RequiredVerifiers;
use crate::service::ServiceError;

use super::error::AdminConsensusManagerError;
use super::shared::AdminServiceShared;
use super::{admin_service_id, sha256};

/// Component used by the service to manage and interact with consensus
pub struct AdminConsensusManager {
    consensus_msg_tx: Sender<ConsensusMessage>,
    proposal_update_tx: Sender<ProposalUpdate>,
    thread_handle: JoinHandle<()>,
}

impl AdminConsensusManager {
    /// Create the proposal manager, network sender, and channels used to communicate with
    /// consensus, and start consensus in a separate thread.
    pub fn new(
        service_id: String,
        shared: Arc<Mutex<AdminServiceShared>>,
        // The coordinator timeout for the two-phase commit consensus engine
        coordinator_timeout: Duration,
    ) -> Result<Self, AdminConsensusManagerError> {
        let (consensus_msg_tx, consensus_msg_rx) = channel();
        let (proposal_update_tx, proposal_update_rx) = channel();

        let proposal_manager =
            AdminProposalManager::new(proposal_update_tx.clone(), shared.clone());
        let consensus_network_sender = AdminConsensusNetworkSender::new(service_id.clone(), shared);
        let startup_state = StartupState {
            id: service_id.as_bytes().into(),
            peer_ids: vec![],
            last_proposal: None,
        };

        let thread_handle = Builder::new()
            .name(format!("consensus-{}", service_id))
            .spawn(move || {
                let mut two_phase_engine = TwoPhaseEngine::new(coordinator_timeout);
                if let Err(err) = two_phase_engine.run(
                    consensus_msg_rx,
                    proposal_update_rx,
                    Box::new(consensus_network_sender),
                    Box::new(proposal_manager),
                    startup_state,
                ) {
                    error!("two phase consensus exited with an error: {}", err)
                };
            })
            .map_err(|err| AdminConsensusManagerError(Box::new(err)))?;

        Ok(AdminConsensusManager {
            consensus_msg_tx,
            proposal_update_tx,
            thread_handle,
        })
    }

    /// Consumes self and shuts down the consensus thread.
    pub fn shutdown(self) -> Result<(), AdminConsensusManagerError> {
        self.send_update(ProposalUpdate::Shutdown)?;

        self.thread_handle
            .join()
            .unwrap_or_else(|err| error!("consensus thread failed: {:?}", err));

        Ok(())
    }

    pub fn handle_message(&self, message_bytes: &[u8]) -> Result<(), AdminConsensusManagerError> {
        let consensus_message = ConsensusMessage::try_from(message_bytes)
            .map_err(|err| AdminConsensusManagerError(Box::new(err)))?;

        self.consensus_msg_tx
            .send(consensus_message)
            .map_err(|err| AdminConsensusManagerError(Box::new(err)))?;

        Ok(())
    }

    pub fn send_update(&self, update: ProposalUpdate) -> Result<(), AdminConsensusManagerError> {
        self.proposal_update_tx
            .send(update)
            .map_err(|err| AdminConsensusManagerError(Box::new(err)))
    }

    pub fn proposal_update_sender(&self) -> Sender<ProposalUpdate> {
        self.proposal_update_tx.clone()
    }
}

pub struct AdminProposalManager {
    proposal_update_sender: Sender<ProposalUpdate>,
    shared: Arc<Mutex<AdminServiceShared>>,
}

impl AdminProposalManager {
    pub fn new(
        proposal_update_sender: Sender<ProposalUpdate>,
        shared: Arc<Mutex<AdminServiceShared>>,
    ) -> Self {
        AdminProposalManager {
            proposal_update_sender,
            shared,
        }
    }
}

impl ProposalManager for AdminProposalManager {
    // Ignoring previous proposal ID because this service and two phase
    // consensus don't care about it. The consensus data field is set to a 2PC-specific
    // message that's generated by the proposal manager to tell consensus who the required
    // verifiers are.
    fn create_proposal(
        &self,
        _previous_proposal_id: Option<ProposalId>,
        _consensus_data: Vec<u8>,
    ) -> Result<(), ProposalManagerError> {
        let network_sender = self
            .shared
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?
            .network_sender()
            .as_ref()
            .cloned()
            .ok_or(ServiceError::NotStarted)?;

        let mut shared = self
            .shared
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?;
        if let Some(circuit_payload) = shared.pop_pending_circuit_payload() {
            let (expected_hash, circuit_proposal) = shared
                .propose_change(circuit_payload.clone())
                .map_err(|err| ProposalManagerError::Internal(Box::new(err)))?;

            // Cheating a bit here by not setting the ID properly (isn't a hash of previous_id,
            // proposal_height, and summary), but none of this really matters with 2-phase
            // consensus. The ID is the hash of the circuit management playload. This example will
            // not work with forking consensus, because it does not track previously accepted
            // proposals.
            let mut proposal = Proposal {
                id: sha256(&circuit_payload)
                    .map_err(|err| ProposalManagerError::Internal(Box::new(err)))?
                    .as_bytes()
                    .into(),
                summary: expected_hash.as_bytes().into(),
                ..Default::default()
            };

            let mut required_verifiers = RequiredVerifiers::new();
            let mut verifiers = vec![];
            let members = circuit_proposal.get_circuit_proposal().get_members();
            for member in members {
                verifiers.push(admin_service_id(member.get_node_id()).as_bytes().to_vec());
            }
            required_verifiers.set_verifiers(RepeatedField::from_vec(verifiers));
            let required_verifiers_bytes = required_verifiers
                .write_to_bytes()
                .map_err(|err| ProposalManagerError::Internal(Box::new(err)))?;
            proposal.consensus_data = required_verifiers_bytes.clone();

            shared.add_pending_consensus_proposal(
                proposal.id.clone(),
                (proposal.clone(), circuit_payload.clone()),
            );

            // Send the proposal to the other services
            let mut proposed_circuit = ProposedCircuit::new();
            proposed_circuit.set_circuit_payload(circuit_payload);
            proposed_circuit.set_expected_hash(expected_hash.as_bytes().into());
            proposed_circuit.set_required_verifiers(required_verifiers_bytes);
            let mut msg = AdminMessage::new();
            msg.set_message_type(AdminMessage_Type::PROPOSED_CIRCUIT);
            msg.set_proposed_circuit(proposed_circuit);

            let envelope_bytes = msg.write_to_bytes().unwrap();
            for member in members {
                if member.get_node_id() != shared.node_id() {
                    network_sender
                        .send(&admin_service_id(member.get_node_id()), &envelope_bytes)
                        .unwrap();
                }
            }

            self.proposal_update_sender
                .send(ProposalUpdate::ProposalCreated(Some(proposal)))?;
        } else {
            self.proposal_update_sender
                .send(ProposalUpdate::ProposalCreated(None))?;
        }

        Ok(())
    }

    fn check_proposal(&self, id: &ProposalId) -> Result<(), ProposalManagerError> {
        let mut shared = self
            .shared
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?;

        let (proposal, circuit_payload) = shared
            .pending_consensus_proposals(id)
            .ok_or_else(|| ProposalManagerError::UnknownProposal(id.clone()))?
            .clone();

        let (hash, _) = shared
            .propose_change(circuit_payload)
            .map_err(|err| ProposalManagerError::Internal(Box::new(err)))?;

        // check if hash is the expected hash stored in summary
        if hash.as_bytes().to_vec() != proposal.summary {
            warn!(
                "Hash mismatch: expected {} but was {}",
                to_hex(&proposal.summary),
                to_hex(hash.as_bytes())
            );

            self.proposal_update_sender
                .send(ProposalUpdate::ProposalInvalid(id.clone()))?;
        } else {
            self.proposal_update_sender
                .send(ProposalUpdate::ProposalValid(id.clone()))?;
        }

        Ok(())
    }

    fn accept_proposal(
        &self,
        id: &ProposalId,
        _consensus_data: Option<Vec<u8>>,
    ) -> Result<(), ProposalManagerError> {
        let mut shared = self
            .shared
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?;

        match shared.pending_consensus_proposals(id) {
            Some((proposal, _)) if &proposal.id == id => match shared.commit() {
                Ok(_) => {
                    shared.remove_pending_consensus_proposals(id);
                    info!("Committed proposal {}", id);
                }
                Err(err) => {
                    self.proposal_update_sender
                        .send(ProposalUpdate::ProposalAcceptFailed(
                            id.clone(),
                            format!("failed to commit proposal: {}", err),
                        ))?
                }
            },
            _ => self
                .proposal_update_sender
                .send(ProposalUpdate::ProposalAcceptFailed(
                    id.clone(),
                    "not pending proposal".into(),
                ))?,
        }

        Ok(())
    }

    fn reject_proposal(&self, id: &ProposalId) -> Result<(), ProposalManagerError> {
        let mut shared = self
            .shared
            .lock()
            .map_err(|_| ServiceError::PoisonedLock("the admin state lock was poisoned".into()))?;

        shared
            .remove_pending_consensus_proposals(id)
            .ok_or_else(|| ProposalManagerError::UnknownProposal(id.clone()))?;

        shared
            .rollback()
            .map_err(|err| ProposalManagerError::Internal(Box::new(err)))?;

        info!("Rolled back proposal {}", id);

        Ok(())
    }
}

pub struct AdminConsensusNetworkSender {
    service_id: String,
    state: Arc<Mutex<AdminServiceShared>>,
}

impl AdminConsensusNetworkSender {
    pub fn new(service_id: String, state: Arc<Mutex<AdminServiceShared>>) -> Self {
        AdminConsensusNetworkSender { service_id, state }
    }
}

impl ConsensusNetworkSender for AdminConsensusNetworkSender {
    fn send_to(&self, peer_id: &PeerId, message: Vec<u8>) -> Result<(), ConsensusSendError> {
        let peer_id_string = String::from_utf8(peer_id.clone().into())
            .map_err(|err| ConsensusSendError::Internal(Box::new(err)))?;

        let consensus_message = ConsensusMessage::new(message, self.service_id.as_bytes().into());
        let mut msg = AdminMessage::new();
        msg.set_message_type(AdminMessage_Type::CONSENSUS_MESSAGE);
        msg.set_consensus_message(consensus_message.try_into()?);

        let shared = self.state.lock().map_err(|_| {
            ConsensusSendError::Internal(Box::new(ServiceError::PoisonedLock(
                "the admin state lock was poisoned".into(),
            )))
        })?;

        let network_sender = shared
            .network_sender()
            .clone()
            .ok_or(ConsensusSendError::NotReady)?;

        network_sender
            .send(&peer_id_string, msg.write_to_bytes()?.as_slice())
            .map_err(|err| ConsensusSendError::Internal(Box::new(err)))?;

        Ok(())
    }

    fn broadcast(&self, message: Vec<u8>) -> Result<(), ConsensusSendError> {
        let consensus_message = ConsensusMessage::new(message, self.service_id.as_bytes().into());
        let mut msg = AdminMessage::new();
        msg.set_message_type(AdminMessage_Type::CONSENSUS_MESSAGE);
        msg.set_consensus_message(consensus_message.try_into()?);

        let shared = self.state.lock().map_err(|_| {
            ConsensusSendError::Internal(Box::new(ServiceError::PoisonedLock(
                "the admin state lock was poisoned".into(),
            )))
        })?;

        let network_sender = shared
            .network_sender()
            .clone()
            .ok_or(ConsensusSendError::NotReady)?;

        // Since there are not a fixed set of peers to send messages too, use the set of verifiers
        // in the current_consensus_verifiers which comes from the pending_changes
        for verifier in shared.current_consensus_verifiers() {
            {
                // don't send a message back to this service
                if verifier != &admin_service_id(shared.node_id()) {
                    network_sender
                        .send(verifier, msg.write_to_bytes()?.as_slice())
                        .map_err(|err| ConsensusSendError::Internal(Box::new(err)))?;
                }
            }
        }

        Ok(())
    }
}
