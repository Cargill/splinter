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

use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::{TryFrom, TryInto};
use std::iter::ExactSizeIterator;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use cylinder::{PublicKey, Signature, Verifier as SignatureVerifier};
use protobuf::{Message, RepeatedField};

use crate::admin::store::{
    AdminServiceStore, Circuit as StoreCircuit, CircuitBuilder as StoreCircuitBuilder,
    CircuitPredicate, CircuitProposal as StoreProposal, CircuitStatus as StoreCircuitStatus,
    ProposalType, ProposedCircuit, Service as StoreService, Vote, VoteRecordBuilder,
};
use crate::admin::token::{PeerAuthorizationTokenReader, PeerNode};
use crate::admin::CIRCUIT_PROTOCOL_VERSION;
use crate::circuit::routing::{self, RoutingTableWriter};
use crate::consensus::{Proposal, ProposalId, ProposalUpdate};
use crate::error::InternalError;
use crate::hex::parse_hex;
use crate::hex::to_hex;
use crate::keys::KeyPermissionManager;
use crate::orchestrator::{ServiceDefinition, ServiceOrchestrator};
use crate::peer::{PeerAuthorizationToken, PeerManagerConnector, PeerRef, PeerTokenPair};
use crate::protos::admin::{
    AbandonedCircuit, AdminMessage, AdminMessage_Type, Circuit, CircuitManagementPayload,
    CircuitManagementPayload_Action, CircuitManagementPayload_Header, CircuitProposal,
    CircuitProposalVote, CircuitProposalVote_Vote, CircuitProposal_ProposalType,
    Circuit_AuthorizationType, Circuit_CircuitStatus, Circuit_DurabilityType,
    Circuit_PersistenceType, Circuit_RouteType, MemberReady, RemovedProposal,
    ServiceProtocolVersionRequest, SplinterNode, SplinterService,
};
use crate::public_key;
use crate::service::error::ServiceError;
use crate::service::validation::ServiceArgValidator;

use crate::service::ServiceNetworkSender;

use super::error::{AdminSharedError, MarshallingError};
use super::messages;
use super::subscriber::SubscriberMap;
use super::{admin_service_id, sha256, AdminKeyVerifier, AdminServiceEventSubscriber, Events};
use super::{ADMIN_SERVICE_PROTOCOL_MIN, ADMIN_SERVICE_PROTOCOL_VERSION};

static VOTER_ROLE: &str = "voter";
static PROPOSER_ROLE: &str = "proposer";
const ADMIN_SERVICE_PUBLIC_KEY_PREFIX: &str = "public_key";
const DEFAULT_HOLD_PEER_SECS: u64 = 10;

pub enum PayloadType {
    Circuit(CircuitManagementPayload),
    Consensus(ProposalId, (Proposal, CircuitManagementPayload)),
}

#[derive(PartialEq, Clone, Copy)]
pub enum AdminServiceStatus {
    NotRunning,
    Running,
    ShuttingDown,
    Shutdown,
}

pub struct PendingPayload {
    pub unpeered_ids: Vec<PeerTokenPair>,
    pub missing_protocol_ids: Vec<PeerNode>,
    pub payload_type: PayloadType,
    pub message_sender: String,
    pub members: Vec<PeerTokenPair>,
}

#[derive(Clone, Debug)]
pub struct PeerNodePair {
    pub peer_node: PeerNode,
    pub local_peer_token: PeerAuthorizationToken,
}

enum CircuitProposalStatus {
    Accepted,
    Rejected,
    Pending,
}

struct CircuitProposalContext {
    pub circuit_proposal: CircuitProposal,
    pub action: CircuitManagementPayload_Action,
    pub signer_public_key: Vec<u8>,
}

struct UninitializedCircuit {
    pub circuit: Option<CircuitProposal>,
    pub ready_members: HashSet<String>,
}

pub struct AdminServiceShared {
    // the node id of the connected splinter node
    node_id: String,
    // the list of circuit that have been committed to splinter state but whose services haven't
    // been initialized or stopped, depending on the proposal type
    uninitialized_circuits: HashMap<String, UninitializedCircuit>,
    orchestrator: Arc<Mutex<ServiceOrchestrator>>,
    // map of service arg validators, by service type
    service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
    // peer connector used to connect to new members listed in a circuit
    peer_connector: PeerManagerConnector,
    // PeerRef Map, peer_id to PeerRef, these PeerRef should be dropped when the peer is no longer
    // needed
    peer_refs: HashMap<PeerTokenPair, Vec<PeerRef>>,
    // network sender is used to communicate with other services on the splinter network
    network_sender: Option<Box<dyn ServiceNetworkSender>>,
    // the CircuitManagementPayloads that are waiting for members to be peered
    unpeered_payloads: Vec<PendingPayload>,
    // the CircuitManagementPayloads that require the peers' admin services to negotiate a protocol
    // version
    pending_protocol_payloads: Vec<PendingPayload>,
    // the agreed upon protocol version between another admin service, map of peer token for the
    // service id to version protocol
    service_protocols: HashMap<PeerTokenPair, u32>,
    // CircuitManagmentPayloads that still need to go through consensus
    pending_circuit_payloads: VecDeque<CircuitManagementPayload>,
    // The pending consensus proposals
    pending_consensus_proposals: HashMap<ProposalId, (Proposal, CircuitManagementPayload)>,
    // the pending changes for the current proposal
    pending_changes: Option<CircuitProposalContext>,
    // the verifiers that should be broadcasted for the pending change
    current_consensus_verifiers: Vec<PeerTokenPair>,
    // Admin Service Event Subscribers
    event_subscribers: SubscriberMap,
    // AdminServiceStore
    admin_store: Box<dyn AdminServiceStore>,
    // signature verifier
    signature_verifier: Box<dyn SignatureVerifier>,
    key_verifier: Box<dyn AdminKeyVerifier>,
    key_permission_manager: Box<dyn KeyPermissionManager>,
    proposal_sender: Option<Sender<ProposalUpdate>>,

    admin_service_status: AdminServiceStatus,
    routing_table_writer: Box<dyn RoutingTableWriter>,
    // Mailbox of AdminServiceEvent values
    event_store: Box<dyn AdminServiceStore>,
    public_keys: Vec<public_key::PublicKey>,
    token_to_peer: HashMap<PeerTokenPair, PeerNodePair>,
    // Temporarily hold on to peers that should be removed. This helps avoid dropping messages
    // when removing a proposal.
    peers_to_be_removed: Vec<(Instant, Vec<PeerTokenPair>)>,
}

impl AdminServiceShared {
    #![allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: String,
        orchestrator: Arc<Mutex<ServiceOrchestrator>>,
        service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
        peer_connector: PeerManagerConnector,
        admin_store: Box<dyn AdminServiceStore>,
        signature_verifier: Box<dyn SignatureVerifier>,
        key_verifier: Box<dyn AdminKeyVerifier>,
        key_permission_manager: Box<dyn KeyPermissionManager>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
        admin_service_event_store: Box<dyn AdminServiceStore>,
        public_keys: Vec<public_key::PublicKey>,
    ) -> Self {
        AdminServiceShared {
            node_id,
            network_sender: None,
            uninitialized_circuits: Default::default(),
            orchestrator,
            service_arg_validators,
            peer_connector,
            peer_refs: HashMap::new(),
            unpeered_payloads: Vec::new(),
            pending_protocol_payloads: Vec::new(),
            service_protocols: HashMap::new(),
            pending_circuit_payloads: VecDeque::new(),
            pending_consensus_proposals: HashMap::new(),
            pending_changes: None,
            current_consensus_verifiers: Vec::new(),
            event_subscribers: SubscriberMap::new(),
            admin_store,
            signature_verifier,
            key_verifier,
            key_permission_manager,
            proposal_sender: None,
            admin_service_status: AdminServiceStatus::NotRunning,
            routing_table_writer,
            event_store: admin_service_event_store,
            public_keys,
            token_to_peer: HashMap::new(),
            peers_to_be_removed: Vec::new(),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn is_local_node(&self, peer_id: &PeerAuthorizationToken) -> bool {
        match peer_id {
            PeerAuthorizationToken::Trust { peer_id } => peer_id == self.node_id(),
            PeerAuthorizationToken::Challenge { public_key } => {
                self.public_keys.contains(public_key)
            }
        }
    }

    // The local admin service will always be connected using Trust
    fn is_local_admin_service(&self, peer_id: &PeerTokenPair) -> bool {
        match peer_id.peer_id() {
            PeerAuthorizationToken::Trust { .. } => peer_id
                .peer_id()
                .has_peer_id(&admin_service_id(self.node_id())),
            PeerAuthorizationToken::Challenge { .. } => false,
        }
    }

    pub fn set_token_to_peer(&mut self, token_to_peer: HashMap<PeerTokenPair, PeerNodePair>) {
        self.token_to_peer = token_to_peer;
    }

    pub fn token_to_peer(&self) -> &HashMap<PeerTokenPair, PeerNodePair> {
        &self.token_to_peer
    }

    pub fn network_sender(&self) -> &Option<Box<dyn ServiceNetworkSender>> {
        &self.network_sender
    }

    pub fn set_network_sender(&mut self, network_sender: Option<Box<dyn ServiceNetworkSender>>) {
        self.network_sender = network_sender;
    }

    pub fn set_proposal_sender(&mut self, proposal_sender: Option<Sender<ProposalUpdate>>) {
        self.proposal_sender = proposal_sender;
    }

    pub fn pop_pending_circuit_payload(&mut self) -> Option<CircuitManagementPayload> {
        self.pending_circuit_payloads.pop_front()
    }

    pub fn routing_table_writer(&self) -> Box<dyn RoutingTableWriter> {
        self.routing_table_writer.clone()
    }

    pub fn pending_consensus_proposals(
        &self,
        id: &ProposalId,
    ) -> Option<&(Proposal, CircuitManagementPayload)> {
        self.pending_consensus_proposals.get(id)
    }

    pub fn remove_pending_consensus_proposals(
        &mut self,
        id: &ProposalId,
    ) -> Option<(Proposal, CircuitManagementPayload)> {
        self.pending_consensus_proposals.remove(id)
    }

    pub fn add_pending_consensus_proposal(
        &mut self,
        id: ProposalId,
        proposal: (Proposal, CircuitManagementPayload),
    ) {
        self.pending_consensus_proposals.insert(id, proposal);
    }

    pub fn current_consensus_verifiers(&self) -> &Vec<PeerTokenPair> {
        &self.current_consensus_verifiers
    }

    pub fn add_peer_ref(&mut self, peer_ref: PeerRef) {
        if let Some(peer_ref_vec) = self.peer_refs.get_mut(peer_ref.peer_id()) {
            peer_ref_vec.push(peer_ref);
        } else {
            self.peer_refs
                .insert(peer_ref.peer_id().clone(), vec![peer_ref]);
        }
    }

    pub fn add_peer_refs(&mut self, peer_refs: Vec<PeerRef>) {
        for peer_ref in peer_refs {
            self.add_peer_ref(peer_ref);
        }
    }

    pub fn remove_peer_refs(&mut self, peer_ids: Vec<PeerTokenPair>) {
        for peer_id in peer_ids {
            if let Some(mut peer_ref_vec) = self.peer_refs.remove(&peer_id) {
                peer_ref_vec.pop();
                if !peer_ref_vec.is_empty() {
                    self.peer_refs.insert(peer_id, peer_ref_vec);
                } else {
                    // If we have no other peer refs for this peer, the connection will be closed.
                    // On reconnection, the peer must go through protocol agreement again
                    self.service_protocols.remove(&peer_id);
                }
            }
        }
    }

    /// Remove the peers who have been held onto for the default holding time. In some cases
    /// peers should not be removed when a proposal is removed because it can cause messages to be
    /// dropped. Instead, the will be dropped after 10 seconds or when the cleanup_held_peer_refs
    /// function is called.
    pub fn cleanup_held_peer_refs(&mut self) {
        let peers_to_be_removed = std::mem::take(&mut self.peers_to_be_removed);
        let (to_clean, pending) = peers_to_be_removed
            .into_iter()
            .partition(|(instant, _)| instant.elapsed().as_secs() > DEFAULT_HOLD_PEER_SECS);

        self.peers_to_be_removed = pending;

        for (_, peers) in to_clean {
            self.remove_peer_refs(peers);
        }
    }

    pub fn change_status(&mut self) {
        match self.admin_service_status {
            AdminServiceStatus::NotRunning => {
                self.admin_service_status = AdminServiceStatus::Running
            }
            AdminServiceStatus::Running => {
                self.admin_service_status = AdminServiceStatus::ShuttingDown
            }
            AdminServiceStatus::ShuttingDown => {
                self.admin_service_status = AdminServiceStatus::Shutdown
            }
            AdminServiceStatus::Shutdown => (),
        }
    }

    pub fn admin_service_status(&self) -> AdminServiceStatus {
        self.admin_service_status
    }

    pub fn commit(&mut self) -> Result<(), AdminSharedError> {
        match self.pending_changes.take() {
            Some(circuit_proposal_context) => {
                let circuit_proposal = circuit_proposal_context.circuit_proposal;
                let action = circuit_proposal_context.action;
                let circuit_id = circuit_proposal.get_circuit_id();
                let mgmt_type = circuit_proposal
                    .get_circuit_proposal()
                    .circuit_management_type
                    .clone();

                match self.check_approved(&circuit_proposal) {
                    CircuitProposalStatus::Accepted => {
                        let status = circuit_proposal.get_circuit_proposal().get_circuit_status();
                        // Verifying if the circuit proposal is associated with a disband request.
                        // If the status is set to `DISBANDED`, the proposal is associated with
                        // a disband request. Otherwise, the admin service should continue with
                        // committing a new circuit proposal. For 0.4 compatibility, this is the
                        // default action as these proposals will not have the `circuit_status`
                        // field set.
                        if status == Circuit_CircuitStatus::DISBANDED {
                            let store_circuit =
                                StoreCircuit::try_from(circuit_proposal.get_circuit_proposal())
                                    .map_err(|err| {
                                        AdminSharedError::SplinterStateError(format!(
                                            "Unable to convert proto Circuit to store Circuit: {}",
                                            err.to_string()
                                        ))
                                    })?;

                            if store_circuit.circuit_status() != &StoreCircuitStatus::Disbanded {
                                return Err(AdminSharedError::SplinterStateError(format!(
                                    "Circuit should be disbanded: {}",
                                    circuit_id
                                )));
                            }
                            // Updating the corresponding `active` circuit from the admin store
                            // and then removing the corresponding `CircuitProposal` from the
                            // disband request
                            self.admin_store
                                .update_circuit(store_circuit.clone())
                                .map_err(|_| {
                                    AdminSharedError::SplinterStateError(format!(
                                        "Unable to update circuit {}",
                                        circuit_id
                                    ))
                                })
                                .and_then(|_| self.remove_proposal(store_circuit.circuit_id()))?;

                            self.update_metrics()?;

                            // send message about circuit disband proposal being accepted
                            let circuit_proposal_proto =
                                messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                    .map_err(AdminSharedError::InvalidMessageFormat)?;
                            let event = messages::AdminServiceEvent::ProposalAccepted((
                                circuit_proposal_proto,
                                circuit_proposal_context.signer_public_key,
                            ));
                            self.send_event(&mgmt_type, event);
                            // send MEMBER_READY message to all other members' admin
                            // services
                            if let Some(ref network_sender) = self.network_sender {
                                let mut member_ready = MemberReady::new();
                                member_ready.set_circuit_id(circuit_id.to_string());
                                member_ready.set_member_node_id(self.node_id.clone());
                                let mut msg = AdminMessage::new();
                                msg.set_message_type(AdminMessage_Type::MEMBER_READY);
                                msg.set_member_ready(member_ready);

                                let envelope_bytes =
                                    msg.write_to_bytes().map_err(MarshallingError::from)?;
                                for token in store_circuit
                                    .list_tokens(&self.node_id)
                                    .map_err(|_| {
                                        AdminSharedError::SplinterStateError(format!(
                                            "Unable to get member peer tokens from {}",
                                            circuit_id
                                        ))
                                    })?
                                    .iter()
                                {
                                    if !self.is_local_node(token.peer_id()) {
                                        network_sender.send(
                                            &admin_service_id(&token.id_as_string()),
                                            &envelope_bytes,
                                        )?;
                                    }
                                }
                            }
                        } else {
                            // commit new circuit
                            self.admin_store.upgrade_proposal_to_circuit(circuit_id)?;

                            self.update_metrics()?;

                            let circuit =
                                self.admin_store.get_circuit(circuit_id)?.ok_or_else(|| {
                                    AdminSharedError::SplinterStateError(format!(
                                        "Unable to get circuit that was just set: {}",
                                        circuit_id
                                    ))
                                })?;

                            let routing_circuit = routing::Circuit::new(
                                circuit.circuit_id().to_string(),
                                circuit
                                    .roster()
                                    .iter()
                                    .map(|service| {
                                        routing::Service::new(
                                            service.service_id().to_string(),
                                            service.service_type().to_string(),
                                            service.node_id().to_string(),
                                            service.arguments().to_vec(),
                                        )
                                    })
                                    .collect(),
                                circuit
                                    .members()
                                    .iter()
                                    .map(|node| node.node_id().to_string())
                                    .collect(),
                                circuit.authorization_type().into(),
                            );

                            let routing_members = circuit_proposal
                                .get_circuit_proposal()
                                .get_members()
                                .iter()
                                .map(|node| {
                                    routing::CircuitNode::new(
                                        node.get_node_id().to_string(),
                                        node.get_endpoints().to_vec(),
                                        if node.get_public_key().is_empty() {
                                            None
                                        } else {
                                            Some(public_key::PublicKey::from_bytes(
                                                node.get_public_key().to_vec(),
                                            ))
                                        },
                                    )
                                })
                                .collect::<Vec<routing::CircuitNode>>();

                            self.routing_table_writer
                                .add_circuit(
                                    circuit.circuit_id().to_string(),
                                    routing_circuit,
                                    routing_members,
                                )
                                .map_err(|_| {
                                    AdminSharedError::SplinterStateError(format!(
                                        "Unable to add new circuit to routing table: {}",
                                        circuit_id
                                    ))
                                })?;

                            // send message about circuit acceptance
                            let circuit_proposal_proto =
                                messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                    .map_err(AdminSharedError::InvalidMessageFormat)?;
                            let event = messages::AdminServiceEvent::ProposalAccepted((
                                circuit_proposal_proto,
                                circuit_proposal_context.signer_public_key,
                            ));
                            self.send_event(&mgmt_type, event);

                            // send MEMBER_READY message to all other members' admin services
                            if let Some(ref network_sender) = self.network_sender {
                                let mut member_ready = MemberReady::new();
                                member_ready.set_circuit_id(circuit_id.to_string());
                                member_ready.set_member_node_id(self.node_id.clone());
                                let mut msg = AdminMessage::new();
                                msg.set_message_type(AdminMessage_Type::MEMBER_READY);
                                msg.set_member_ready(member_ready);

                                let envelope_bytes =
                                    msg.write_to_bytes().map_err(MarshallingError::from)?;

                                for token in circuit
                                    .list_tokens(&self.node_id)
                                    .map_err(|_| {
                                        AdminSharedError::SplinterStateError(format!(
                                            "Unable to get member peer tokens from {}",
                                            circuit.circuit_id()
                                        ))
                                    })?
                                    .iter()
                                {
                                    if !self.is_local_node(token.peer_id()) {
                                        network_sender.send(
                                            &admin_service_id(&token.id_as_string()),
                                            &envelope_bytes,
                                        )?;
                                    }
                                }
                            }
                        }
                        // add circuit as pending further service handling
                        self.add_uninitialized_circuit(circuit_proposal.clone())?;

                        Ok(())
                    }
                    CircuitProposalStatus::Pending => {
                        match action {
                            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST => {
                                self.add_proposal(circuit_proposal.clone())?;
                                self.update_metrics()?;
                                // notify registered application authorization handlers of the
                                // committed circuit proposal
                                let event = messages::AdminServiceEvent::ProposalSubmitted(
                                    messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                        .map_err(AdminSharedError::InvalidMessageFormat)?,
                                );
                                self.send_event(&mgmt_type, event);

                                info!(
                                    "committed changes for new circuit proposal to create circuit \
                                     {}",
                                    circuit_id
                                );
                                Ok(())
                            }

                            CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE => {
                                self.update_proposal(circuit_proposal.clone())?;
                                self.update_metrics()?;
                                // notify registered application authorization handlers of the
                                // committed circuit proposal
                                let circuit_proposal_proto =
                                    messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                        .map_err(AdminSharedError::InvalidMessageFormat)?;
                                let event = messages::AdminServiceEvent::ProposalVote((
                                    circuit_proposal_proto,
                                    circuit_proposal_context.signer_public_key,
                                ));
                                self.send_event(&mgmt_type, event);

                                info!("committed vote for circuit proposal {}", circuit_id);
                                Ok(())
                            }
                            CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST => {
                                self.add_proposal(circuit_proposal.clone())?;
                                self.update_metrics()?;
                                // notify registered application authorization handlers of the
                                // committed disband circuit proposal
                                let event = messages::AdminServiceEvent::ProposalSubmitted(
                                    messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                        .map_err(AdminSharedError::InvalidMessageFormat)?,
                                );
                                self.send_event(&mgmt_type, event);

                                info!(
                                    "committed changes for new circuit proposal to disband \
                                       circuit {}",
                                    circuit_id
                                );
                                Ok(())
                            }
                            _ => Err(AdminSharedError::UnknownAction(format!(
                                "Received unknown action: {:?}",
                                action
                            ))),
                        }
                    }
                    CircuitProposalStatus::Rejected => {
                        // remove circuit
                        let proposal = self.remove_proposal(circuit_id)?;
                        self.update_metrics()?;
                        if let Some(proposal) = proposal {
                            self.peers_to_be_removed.push((
                                Instant::now(),
                                proposal
                                    .circuit()
                                    .list_tokens(&self.node_id)
                                    .map_err(|err| {
                                        AdminSharedError::SplinterStateError(format!(
                                            "Unable to remove peer refs for proposal {}: {}",
                                            proposal.circuit_id(),
                                            err
                                        ))
                                    })?,
                            ));
                        }
                        let circuit_proposal_proto =
                            messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                .map_err(AdminSharedError::InvalidMessageFormat)?;
                        let event = messages::AdminServiceEvent::ProposalRejected((
                            circuit_proposal_proto,
                            circuit_proposal_context.signer_public_key,
                        ));
                        self.send_event(&mgmt_type, event);

                        info!("circuit proposal for {} has been rejected", circuit_id);
                        Ok(())
                    }
                }
            }
            None => Err(AdminSharedError::NoPendingChanges),
        }
    }

    pub fn rollback(&mut self) -> Result<(), AdminSharedError> {
        match self.pending_changes.take() {
            Some(circuit_proposal_context) => info!(
                "discarded change for {}",
                circuit_proposal_context.circuit_proposal.get_circuit_id()
            ),
            None => debug!("no changes to rollback"),
        }

        Ok(())
    }

    pub fn propose_change(
        &mut self,
        mut circuit_payload: CircuitManagementPayload,
    ) -> Result<(String, CircuitProposal), AdminSharedError> {
        self.cleanup_held_peer_refs();
        let header = Message::parse_from_bytes(circuit_payload.get_header())
            .map_err(MarshallingError::from)?;
        self.validate_circuit_management_payload(&circuit_payload, &header)?;
        self.verify_signature(&circuit_payload).map_err(|_| {
            AdminSharedError::ValidationFailed(String::from("Unable to verify signature"))
        })?;
        match header.get_action() {
            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST => {
                let mut create_request = circuit_payload.take_circuit_create_request();
                let proposed_circuit = create_request.take_circuit();
                let mut verifiers = vec![];
                let mut protocol = ADMIN_SERVICE_PROTOCOL_VERSION;

                let local_required_auth = proposed_circuit
                    .get_node_token(&self.node_id)
                    .map_err(|err| {
                        AdminSharedError::ValidationFailed(format!(
                            "Unable to get local nodes token: {}",
                            err
                        ))
                    })?
                    .ok_or_else(|| {
                        AdminSharedError::ValidationFailed(
                            "Circuit does not have the local node".to_string(),
                        )
                    })?;

                for member in proposed_circuit.list_nodes().map_err(|_| {
                    AdminSharedError::SplinterStateError(format!(
                        "Unable to get tokens for proposal: {}",
                        proposed_circuit.get_circuit_id()
                    ))
                })? {
                    verifiers.push(member.admin_service.clone());
                    // Figure out what protocol version should be used for this proposal
                    if let Some(protocol_version) = self.service_protocols.get(&PeerTokenPair::new(
                        member.token.clone(),
                        local_required_auth.clone(),
                    )) {
                        if protocol_version < &protocol {
                            protocol = *protocol_version
                        }
                    }
                }

                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();

                self.validate_create_circuit(
                    &proposed_circuit,
                    signer_public_key,
                    requester_node_id,
                    protocol,
                )
                .map_err(|err| {
                    match proposed_circuit.list_tokens(&self.node_id) {
                        Ok(tokens) => self.remove_peer_refs(tokens),
                        Err(err) => {
                            error!(
                                "Unable to remove peer refs for proposal {}: {}",
                                proposed_circuit.get_circuit_id(),
                                err
                            );
                        }
                    };

                    err
                })?;
                debug!("proposing {}", proposed_circuit.get_circuit_id());

                let mut circuit_proposal = CircuitProposal::new();
                circuit_proposal.set_proposal_type(CircuitProposal_ProposalType::CREATE);
                circuit_proposal.set_circuit_id(proposed_circuit.get_circuit_id().into());
                circuit_proposal.set_circuit_hash(sha256(&proposed_circuit)?);
                circuit_proposal.set_circuit_proposal(proposed_circuit.clone());
                circuit_proposal.set_requester(header.get_requester().to_vec());
                circuit_proposal.set_requester_node_id(header.get_requester_node_id().to_string());

                let expected_hash = sha256(&circuit_proposal)?;
                self.pending_changes = Some(CircuitProposalContext {
                    circuit_proposal: circuit_proposal.clone(),
                    signer_public_key: header.get_requester().to_vec(),
                    action: CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST,
                });
                self.current_consensus_verifiers =
                    proposed_circuit.list_tokens(&self.node_id).map_err(|_| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to get tokens for proposal: {}",
                            proposed_circuit.get_circuit_id()
                        ))
                    })?;

                Ok((expected_hash, circuit_proposal))
            }
            CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE => {
                let proposal_vote = circuit_payload.get_circuit_proposal_vote();
                // validate vote proposal
                // check that the circuit proposal exists
                let mut circuit_proposal = self
                    .get_proposal(proposal_vote.get_circuit_id())
                    .map_err(|err| {
                        AdminSharedError::ValidationFailed(format!(
                            "error occurred when trying to get proposal {}",
                            err
                        ))
                    })?
                    .ok_or_else(|| {
                        AdminSharedError::ValidationFailed(format!(
                            "Received vote for a proposal that does not exist: circuit id {}",
                            proposal_vote.circuit_id
                        ))
                    })?;

                let mut verifiers = vec![];
                for member in circuit_proposal.circuit().members() {
                    verifiers.push(admin_service_id(member.node_id()));
                }
                let signer_public_key = header.get_requester();

                self.validate_circuit_vote(
                    proposal_vote,
                    signer_public_key,
                    &circuit_proposal,
                    header.get_requester_node_id(),
                )
                .map_err(|err| {
                    if circuit_proposal.proposal_type() == &ProposalType::Create {
                        match circuit_proposal.circuit().list_tokens(&self.node_id) {
                            Ok(tokens) => self.remove_peer_refs(tokens),
                            Err(err) => {
                                error!(
                                    "Unable to remove peer refs for proposal {}: {}",
                                    circuit_proposal.circuit_id(),
                                    err
                                );
                            }
                        };
                    }
                    err
                })?;

                // add vote to circuit_proposal
                let vote = match proposal_vote.get_vote() {
                    CircuitProposalVote_Vote::ACCEPT => Vote::Accept,
                    CircuitProposalVote_Vote::REJECT => Vote::Reject,
                    CircuitProposalVote_Vote::UNSET_VOTE => {
                        return Err(AdminSharedError::ValidationFailed(
                            "Vote is unset".to_string(),
                        ));
                    }
                };

                let vote_record = VoteRecordBuilder::new()
                    .with_public_key(&public_key::PublicKey::from_bytes(
                        signer_public_key.to_vec(),
                    ))
                    .with_vote(&vote)
                    .with_voter_node_id(header.get_requester_node_id())
                    .build()
                    .map_err(|err| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to build vote record: {}",
                            err
                        ))
                    })?;

                let mut votes = circuit_proposal.votes().to_vec();
                votes.push(vote_record);
                circuit_proposal = circuit_proposal
                    .builder()
                    .with_votes(&votes)
                    .build()
                    .map_err(|err| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to build circuit proposal: {}",
                            err
                        ))
                    })?;

                self.current_consensus_verifiers = circuit_proposal
                    .circuit()
                    .list_tokens(&self.node_id)
                    .map_err(|_| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to get tokens for proposal: {}",
                            circuit_proposal.circuit_id()
                        ))
                    })?;

                let proto_circuit_proposal = circuit_proposal.into_proto();

                let expected_hash = sha256(&proto_circuit_proposal)?;
                self.pending_changes = Some(CircuitProposalContext {
                    circuit_proposal: proto_circuit_proposal.clone(),
                    signer_public_key: header.get_requester().to_vec(),
                    action: CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE,
                });

                Ok((expected_hash, proto_circuit_proposal))
            }
            CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST => {
                debug!("Circuit disband request being processed");
                let circuit_id = circuit_payload
                    .get_circuit_disband_request()
                    .get_circuit_id();

                // Validate the specified circuit exists
                self.admin_store.get_circuit(circuit_id)
                    .map_err(|err| {
                        AdminSharedError::ValidationFailed(format!(
                            "error occurred when trying to get circuit {}",
                            err
                        ))
                    })?
                    .ok_or_else(|| {
                        AdminSharedError::ValidationFailed(format!(
                            "Received disband request for a circuit that does not exist: circuit id {}",
                            circuit_id
                        ))
                    })?;

                // Creating the proposal to disband this circuit
                let circuit_proposal = self.make_disband_request_circuit_proposal(
                    circuit_id,
                    header.get_requester(),
                    header.get_requester_node_id(),
                )?;

                let local_required_auth = circuit_proposal
                    .get_circuit_proposal()
                    .get_node_token(&self.node_id)
                    .map_err(|err| {
                        AdminSharedError::ValidationFailed(format!(
                            "Unable to get local nodes token: {}",
                            err
                        ))
                    })?
                    .ok_or_else(|| {
                        AdminSharedError::ValidationFailed(
                            "Circuit does not have the local node".to_string(),
                        )
                    })?;

                let mut verifiers = vec![];
                let mut protocol = ADMIN_SERVICE_PROTOCOL_VERSION;
                for member in circuit_proposal
                    .get_circuit_proposal()
                    .list_nodes()
                    .map_err(|_| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to get tokens for proposal: {}",
                            circuit_proposal.get_circuit_id()
                        ))
                    })?
                {
                    verifiers.push(member.admin_service.clone());
                    // Figure out what protocol version should be used for this proposal
                    if let Some(protocol_version) = self.service_protocols.get(&PeerTokenPair::new(
                        member.token.clone(),
                        local_required_auth.clone(),
                    )) {
                        if protocol_version < &protocol {
                            protocol = *protocol_version
                        }
                    }
                }
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();

                self.validate_disband_circuit(
                    circuit_proposal.get_circuit_proposal(),
                    signer_public_key,
                    requester_node_id,
                    protocol,
                )?;

                let expected_hash = sha256(&circuit_proposal)?;
                self.pending_changes = Some(CircuitProposalContext {
                    circuit_proposal: circuit_proposal.clone(),
                    signer_public_key: header.get_requester().to_vec(),
                    action: CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST,
                });
                self.current_consensus_verifiers = circuit_proposal
                    .get_circuit_proposal()
                    .list_tokens(&self.node_id)
                    .map_err(|_| {
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to get tokens for proposal: {}",
                            circuit_proposal.get_circuit_id()
                        ))
                    })?;

                Ok((expected_hash, circuit_proposal))
            }
            CircuitManagementPayload_Action::ACTION_UNSET => Err(
                AdminSharedError::ValidationFailed("Action must be set".to_string()),
            ),
            unknown_action => Err(AdminSharedError::ValidationFailed(format!(
                "Unable to handle {:?}",
                unknown_action
            ))),
        }
    }

    pub fn has_proposal(&self, circuit_id: &str) -> Result<bool, AdminSharedError> {
        Ok(self.admin_store.get_proposal(circuit_id)?.is_some())
    }

    /// Propose a new circuit
    ///
    /// This operation will propose a new circuit to all the member nodes of the circuit.  If there
    /// is no peer connection, a connection to the peer will also be established.
    pub fn propose_circuit(
        &mut self,
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        debug!(
            "received circuit proposal for {}",
            payload
                .get_circuit_create_request()
                .get_circuit()
                .get_circuit_id()
        );
        let proposed_circuit =
            ProposedCircuit::from_proto(payload.get_circuit_create_request().get_circuit().clone())
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

        let local_required_auth = proposed_circuit
            .get_node_token(&self.node_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get local nodes token: {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    "Circuit does not have the local node".to_string(),
                )))
            })?;

        if !self.is_local_node(&local_required_auth) {
            return Err(ServiceError::UnableToHandleMessage(Box::new(
                AdminSharedError::ValidationFailed(format!(
                    "Circuit contains unsupported token for local node: {}",
                    local_required_auth
                )),
            )));
        }

        let members = proposed_circuit.list_nodes().map_err(|err| {
            ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                format!("Unable to get peer tokens for members: {}", err),
            )))
        })?;

        self.check_connected_peers_payload_create(
            &members,
            payload,
            message_sender,
            local_required_auth,
        )
    }

    pub fn propose_vote(
        &mut self,
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        debug!(
            "received circuit vote for {}",
            payload.get_circuit_proposal_vote().get_circuit_id()
        );
        let circuit_id = payload.get_circuit_proposal_vote().get_circuit_id();
        let proposal = self
            .get_proposal(circuit_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("error occurred when trying to get proposal {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!(
                        "Received vote for a proposal that does not exist: circuit id {}",
                        circuit_id
                    ),
                )))
            })?;

        let local_required_auth = proposal
            .circuit()
            .get_node_token(&self.node_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get local nodes token: {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    "Circuit does not have the local node".to_string(),
                )))
            })?;

        let members = proposal.circuit().list_nodes().map_err(|err| {
            ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                format!("Unable to get peer tokens for members: {}", err),
            )))
        })?;

        self.check_connected_peers_payload_vote(
            &members,
            local_required_auth,
            payload,
            message_sender,
        )
    }

    /// Once a local `CircuitDisbandRequest` has been validated, the admin service may now proceed
    /// to communicating with the remote circuit members to propose the disband change.
    pub fn propose_disband(
        &mut self,
        payload: CircuitManagementPayload,
        requester: &[u8],
        requester_node_id: &str,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        debug!(
            "received circuit disband request {}",
            payload.get_circuit_disband_request().get_circuit_id()
        );
        let circuit_id = payload.get_circuit_disband_request().get_circuit_id();
        let circuit_proposal = self
            .make_disband_request_circuit_proposal(circuit_id, requester, requester_node_id)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

        let local_required_auth = circuit_proposal
            .get_circuit_proposal()
            .get_node_token(&self.node_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get local nodes token: {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    "Circuit does not have the local node".to_string(),
                )))
            })?;

        let members = circuit_proposal
            .get_circuit_proposal()
            .list_nodes()
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get peer tokens for members: {}", err),
                )))
            })?;

        self.check_connected_peers_payload_disband(
            &members,
            local_required_auth,
            payload,
            message_sender,
        )
    }

    pub fn update_metrics(&self) -> Result<(), AdminSharedError> {
        // initialize circuit and proposal metrics
        gauge!(
            "splinter.admin.circuits.active",
            self.admin_store.count_circuits(&[])? as f64
        );
        gauge!(
            "splinter.admin.proposals",
            self.admin_store.count_proposals(&[])? as f64
        );
        Ok(())
    }

    /// Attempts to purge a circuit and the associated internal Splinter services
    fn purge_circuit(&mut self, circuit_id: &str) -> Result<(), ServiceError> {
        // Verifying the circuit is able to be purged
        let stored_circuit = self
            .admin_store
            .get_circuit(circuit_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!("error occurred when trying to get circuit {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!(
                        "Received purged request for a circuit that does not exist: circuit id {}",
                        circuit_id
                    ),
                )))
            })?;

        self.purge_services(circuit_id, stored_circuit.roster())
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

        if let Some(circuit) = self
            .remove_circuit(circuit_id)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?
        {
            debug!("Purged circuit {}", circuit.circuit_id());
            Ok(())
        } else {
            Err(ServiceError::UnableToHandleMessage(Box::new(
                AdminSharedError::SplinterStateError(format!(
                    "unable to purge circuit {}",
                    circuit_id
                )),
            )))
        }
    }

    /// Locally abandon a circuit. The circuit to be abandoned is first fetched from the admin
    /// store, to validate this circuit is available to be abandoned. Then, an `ABANDONED_CIRCUIT`
    /// message is sent to the remote circuit members. Finally, the circuit is abandoned by
    /// stopping the associated services, the peer refs associated with this circuit are removed,
    /// the circuit is removed from the local routing table, and the circuit's `circuit_status` is
    /// updated to `Abandoned`.
    fn abandon_circuit(&mut self, circuit_id: &str) -> Result<(), ServiceError> {
        // Verifying the circuit is able to be abandoned
        let stored_circuit = self
            .admin_store
            .get_circuit(circuit_id)
            .map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!("error occurred when trying to get circuit {}", err),
                )))
            })?
            .ok_or_else(|| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!(
                        "Received abandon request for a circuit that does not exist: circuit id {}",
                        circuit_id
                    ),
                )))
            })?;

        // send ABANDONED_CIRCUIT message to all other members' admin services
        if let Some(ref network_sender) = self.network_sender {
            let mut abandoned_circuit = AbandonedCircuit::new();
            abandoned_circuit.set_circuit_id(circuit_id.to_string());
            abandoned_circuit.set_member_node_id(self.node_id.clone());
            let mut msg = AdminMessage::new();
            msg.set_message_type(AdminMessage_Type::ABANDONED_CIRCUIT);
            msg.set_abandoned_circuit(abandoned_circuit);

            let envelope_bytes = msg.write_to_bytes().map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(MarshallingError::ProtobufError(err)))
            })?;

            for token in stored_circuit
                .list_tokens(&self.node_id)
                .map_err(|_| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to get member peer tokens from {}",
                            circuit_id
                        )),
                    ))
                })?
                .iter()
            {
                if !self.is_local_node(token.peer_id()) {
                    network_sender
                        .send(&admin_service_id(&token.id_as_string()), &envelope_bytes)?;
                }
            }
        }

        let (abandoned_proto_circuit, abandoned_store_circuit) = self
            .make_abandoned_circuit(&stored_circuit)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
        // Updating the corresponding `active` circuit from the admin store to have the
        // `Abandoned` `circuit_status`
        self.admin_store
            .update_circuit(abandoned_store_circuit)
            .map_err(|_| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!("Unable to update circuit {}", circuit_id),
                )))
            })?;

        gauge!(
            "splinter.admin.circuits.active",
            self.admin_store.count_circuits(&[]).map_err(|_| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    String::from("Unable to get count of circuits"),
                )))
            })? as f64
        );

        // The circuit is able to be abandoned, so we will proceed with removing the circuit's
        // networking functionality for this node.
        // Circuit has been abandoned: all associated services will be shut
        // down, the circuit removed from the routing table, and peer refs
        // for this circuit will be removed.
        self.stop_services(&abandoned_proto_circuit)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
        // Removing the circuit from the routing table
        self.routing_table_writer
            .remove_circuit(stored_circuit.circuit_id())
            .map_err(|_| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                    format!(
                        "Unable to remove circuit from routing table: {}",
                        circuit_id
                    ),
                )))
            })?;
        // Removing the circuit's peer refs
        self.remove_peer_refs(stored_circuit.list_tokens(&self.node_id).map_err(|err| {
            ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::SplinterStateError(
                format!(
                    "Unable to remove peer refs for circuit: {}: {}",
                    circuit_id, err
                ),
            )))
        })?);

        Ok(())
    }

    /// Locally remove a Circuit Proposal that has been committed. A message is sent to the
    /// circuit proposal members that the proposal is being removed locally. Once the proposal
    /// has been removed from the admin store, the peer refs created for this proposal are also
    /// removed.
    fn request_proposal_removal(&mut self, circuit_id: &str) -> Result<(), ServiceError> {
        if let Some(proposal) = self
            .get_proposal(circuit_id)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?
        {
            // send REMOVED_PROPOSAL message to all other members' admin services
            if let Some(ref network_sender) = self.network_sender {
                let mut removed_proposal = RemovedProposal::new();
                removed_proposal.set_circuit_id(circuit_id.to_string());
                let mut msg = AdminMessage::new();
                msg.set_message_type(AdminMessage_Type::REMOVED_PROPOSAL);
                msg.set_removed_proposal(removed_proposal);

                let envelope_bytes = msg.write_to_bytes().map_err(|err| {
                    ServiceError::UnableToHandleMessage(Box::new(MarshallingError::ProtobufError(
                        err,
                    )))
                })?;

                for token in proposal
                    .circuit()
                    .list_tokens(&self.node_id)
                    .map_err(|_| {
                        ServiceError::UnableToHandleMessage(Box::new(
                            AdminSharedError::SplinterStateError(format!(
                                "Unable to get member peer tokens from {}",
                                circuit_id
                            )),
                        ))
                    })?
                    .iter()
                {
                    if !self.is_local_node(token.peer_id()) {
                        network_sender
                            .send(&admin_service_id(&token.id_as_string()), &envelope_bytes)?;
                    }
                }
            }

            // Remove the proposal itself
            self.remove_proposal(circuit_id)
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?
                .ok_or_else(|| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to remove proposal for circuit {}, proposal does not exist",
                            &circuit_id
                        )),
                    ))
                })?;
            // Update the metrics because the proposal has been removed for this node
            self.update_metrics()
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

            self.remove_peer_refs(proposal.circuit().list_tokens(&self.node_id).map_err(
                |err| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::SplinterStateError(format!(
                            "Unable to remove peer refs for proposal: {}: {}",
                            circuit_id, err
                        )),
                    ))
                },
            )?);
            Ok(())
        } else {
            Err(ServiceError::UnableToHandleMessage(Box::new(
                AdminSharedError::SplinterStateError(format!(
                    "Unable to remove proposal for circuit {}, proposal does not exist",
                    &circuit_id
                )),
            )))
        }
    }

    pub fn send_protocol_request(
        &mut self,
        token: &PeerTokenPair,
        admin_service: &str,
    ) -> Result<(), ServiceError> {
        if self.service_protocols.get(token).is_none() {
            // we will always have the network sender at this point
            if let Some(ref mut network_sender) = self.network_sender {
                debug!("Sending service protocol request to {}", admin_service);
                let mut request = ServiceProtocolVersionRequest::new();
                request.set_protocol_min(ADMIN_SERVICE_PROTOCOL_MIN);
                request.set_protocol_max(ADMIN_SERVICE_PROTOCOL_VERSION);
                let mut msg = AdminMessage::new();
                msg.set_message_type(AdminMessage_Type::SERVICE_PROTOCOL_VERSION_REQUEST);
                msg.set_protocol_request(request);

                let envelope_bytes = msg.write_to_bytes()?;
                // need to set the sender to our local auth that is being used
                network_sender.send_with_sender(
                    &admin_service_id(&token.id_as_string()),
                    &envelope_bytes,
                    &admin_service_id(
                        &PeerTokenPair::new(token.local_id().clone(), token.peer_id().clone())
                            .id_as_string(),
                    ),
                )?
            }
        } else {
            debug!("Already agreed on protocol version with {}", admin_service);
        }
        Ok(())
    }

    fn check_connected_peers_payload_vote(
        &mut self,
        members: &[PeerNode],
        local_required_auth: PeerAuthorizationToken,
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_members = vec![];
        for node in members {
            let peer_token_pair =
                PeerTokenPair::new(node.token.clone(), local_required_auth.clone());
            if !self.is_local_node(&node.token)
                && self.service_protocols.get(&peer_token_pair).is_none()
            {
                self.send_protocol_request(&peer_token_pair, &node.admin_service)?;
                missing_protocol_ids.push(node.clone())
            }
            pending_members.push(peer_token_pair);
        }

        if missing_protocol_ids.is_empty() {
            self.pending_circuit_payloads.push_back(payload);
        } else {
            debug!(
                "Members {:?} added; awaiting service protocol agreement before proceeding",
                &missing_protocol_ids
            );
            self.pending_protocol_payloads.push(PendingPayload {
                unpeered_ids: vec![],
                missing_protocol_ids,
                payload_type: PayloadType::Circuit(payload),
                members: pending_members,
                message_sender,
            });
        }

        Ok(())
    }

    fn check_connected_peers_payload_create(
        &mut self,
        members: &[PeerNode],
        payload: CircuitManagementPayload,
        message_sender: String,
        local_required_auth: PeerAuthorizationToken,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_peers = vec![];
        let mut pending_members = vec![];
        let mut added_peers: Vec<PeerTokenPair> = vec![];
        for node in members {
            let peer_token_pair =
                PeerTokenPair::new(node.token.clone(), local_required_auth.clone());
            if !self.is_local_node(&node.token) {
                debug!("Referencing node {:?}", &node.token);
                let peer_ref = self
                    .peer_connector
                    .add_peer_ref(
                        node.token.clone(),
                        node.endpoints.to_vec(),
                        local_required_auth.clone(),
                    )
                    .map_err(|err| {
                        // remove all peer refs added for this proposal
                        self.remove_peer_refs(added_peers.to_vec());

                        ServiceError::UnableToHandleMessage(Box::new(err))
                    })?;

                self.add_peer_ref(peer_ref);
                added_peers.push(peer_token_pair.clone());

                // if we have a protocol the connection exists for the peer already
                if self.service_protocols.get(&peer_token_pair).is_none() {
                    pending_peers.push(peer_token_pair.clone());
                    missing_protocol_ids.push(node.clone())
                }
            }
            pending_members.push(peer_token_pair.clone());
            self.token_to_peer.insert(
                peer_token_pair,
                PeerNodePair {
                    peer_node: node.clone(),
                    local_peer_token: local_required_auth.clone(),
                },
            );
        }

        if missing_protocol_ids.is_empty() {
            self.pending_circuit_payloads.push_back(payload);
        } else {
            debug!(
                "Members {:?} added; awaiting peering and service protocol agreement before \
                proceeding",
                &missing_protocol_ids
            );
            self.unpeered_payloads.push(PendingPayload {
                unpeered_ids: pending_peers,
                missing_protocol_ids,
                payload_type: PayloadType::Circuit(payload),
                members: pending_members,
                message_sender,
            });
        }

        Ok(())
    }

    /// Verify all members of the circuit to be disbanded are using a valid protocol version.
    /// If all circuit members have agreed on a protocol version, the disband payload is moved into
    /// the `pending_circuit_payloads` list for further processing. Otherwise, this payload is
    /// added to the `pending_protocol_payloads` list to await all nodes' protocol agreement.
    fn check_connected_peers_payload_disband(
        &mut self,
        members: &[PeerNode],
        local_required_auth: PeerAuthorizationToken,
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_members = vec![];
        for node in members {
            let peer_token_pair =
                PeerTokenPair::new(node.token.clone(), local_required_auth.clone());
            if !self.is_local_node(&node.token)
                && self.service_protocols.get(&peer_token_pair).is_none()
            {
                self.send_protocol_request(&peer_token_pair, &node.admin_service)?;
                missing_protocol_ids.push(node.clone())
            }
            pending_members.push(peer_token_pair.clone());
        }

        if missing_protocol_ids.is_empty() {
            self.pending_circuit_payloads.push_back(payload);
        } else {
            debug!(
                "Members {:?} added; awaiting service protocol agreement before proceeding",
                &missing_protocol_ids
            );
            self.pending_protocol_payloads.push(PendingPayload {
                unpeered_ids: vec![],
                missing_protocol_ids,
                payload_type: PayloadType::Circuit(payload),
                members: pending_members,
                message_sender,
            });
        }

        Ok(())
    }

    pub fn submit(&mut self, payload: CircuitManagementPayload) -> Result<(), ServiceError> {
        debug!("Payload submitted: {:?}", payload);

        let header = Message::parse_from_bytes(payload.get_header())?;
        self.validate_circuit_management_payload(&payload, &header)
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
        self.verify_signature(&payload)?;

        match header.get_action() {
            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST => {
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();
                self.validate_create_circuit(
                    payload.get_circuit_create_request().get_circuit(),
                    signer_public_key,
                    requester_node_id,
                    ADMIN_SERVICE_PROTOCOL_VERSION,
                )
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.propose_circuit(payload, "local".to_string())
            }
            CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE => {
                let proposal_vote = payload.get_circuit_proposal_vote();

                // validate vote proposal
                // check that the circuit proposal exists
                let circuit_proposal = self
                    .get_proposal(proposal_vote.get_circuit_id())
                    .map_err(|err| {
                        ServiceError::UnableToHandleMessage(Box::new(
                            AdminSharedError::ValidationFailed(format!(
                                "error occurred when trying to get proposal {}",
                                err
                            )),
                        ))
                    })?
                    .ok_or_else(|| {
                        ServiceError::UnableToHandleMessage(Box::new(
                            AdminSharedError::ValidationFailed(format!(
                                "Received vote for a proposal that does not exist: circuit id {}",
                                proposal_vote.circuit_id
                            )),
                        ))
                    })?;

                let signer_public_key = header.get_requester();
                self.validate_circuit_vote(
                    proposal_vote,
                    signer_public_key,
                    &circuit_proposal,
                    header.get_requester_node_id(),
                )
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.propose_vote(payload, "local".to_string())
            }
            CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST => {
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();
                let circuit_id = payload.get_circuit_disband_request().get_circuit_id();
                let circuit_proposal = self
                    .make_disband_request_circuit_proposal(
                        circuit_id,
                        signer_public_key,
                        requester_node_id,
                    )
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.validate_disband_circuit(
                    circuit_proposal.get_circuit_proposal(),
                    signer_public_key,
                    requester_node_id,
                    ADMIN_SERVICE_PROTOCOL_VERSION,
                )
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.propose_disband(
                    payload,
                    signer_public_key,
                    requester_node_id,
                    "local".to_string(),
                )
            }
            CircuitManagementPayload_Action::CIRCUIT_PURGE_REQUEST => {
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();
                let circuit_id = payload.get_circuit_purge_request().get_circuit_id();
                debug!("received purge request for circuit {}", circuit_id);

                self.validate_purge_request(circuit_id, signer_public_key, requester_node_id)
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.purge_circuit(circuit_id)
            }
            CircuitManagementPayload_Action::CIRCUIT_ABANDON => {
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();
                let circuit_id = payload.get_circuit_abandon().get_circuit_id();
                debug!("received abandon request for circuit {}", circuit_id);

                self.validate_abandon_circuit(circuit_id, signer_public_key, requester_node_id)
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.abandon_circuit(circuit_id)
            }
            CircuitManagementPayload_Action::PROPOSAL_REMOVE_REQUEST => {
                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();
                let circuit_id = payload.get_proposal_remove_request().get_circuit_id();
                debug!("received removal request for proposal {}", circuit_id);

                self.validate_remove_proposal(circuit_id, signer_public_key, requester_node_id)
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.request_proposal_removal(circuit_id)
            }
            CircuitManagementPayload_Action::ACTION_UNSET => {
                Err(ServiceError::UnableToHandleMessage(Box::new(
                    AdminSharedError::ValidationFailed(String::from("No action specified")),
                )))
            }
            unknown_action => Err(ServiceError::UnableToHandleMessage(Box::new(
                AdminSharedError::ValidationFailed(format!(
                    "Unable to handle {:?}",
                    unknown_action
                )),
            ))),
        }
    }

    /// Handle a new circuit proposal
    ///
    /// This operation will accept a new circuit proposal.  If there is no peer connection, a
    /// connection to the peer will also be established.
    pub fn handle_proposed_circuit(
        &mut self,
        proposal: Proposal,
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_peers = vec![];
        let mut added_peers: Vec<PeerTokenPair> = vec![];
        let mut pending_members = vec![];
        let mut members: Vec<PeerNode> = vec![];

        // Check if that payload is to create a circuit, in which case PeerRefs for the new
        // members must be added.
        if payload.has_circuit_create_request() {
            let store_proposed_circuit = ProposedCircuit::from_proto(
                payload.get_circuit_create_request().get_circuit().clone(),
            )
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

            let local_required_auth = store_proposed_circuit
                .get_node_token(&self.node_id)
                .map_err(|err| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(format!(
                            "Unable to get local nodes token: {}",
                            err
                        )),
                    ))
                })?
                .ok_or_else(|| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(
                            "Circuit does not have the local node".to_string(),
                        ),
                    ))
                })?;

            if !self.is_local_node(&local_required_auth) {
                return Err(ServiceError::UnableToHandleMessage(Box::new(
                    AdminSharedError::ValidationFailed(format!(
                        "Circuit contains unsupported token for local node: {}",
                        local_required_auth
                    )),
                )));
            }

            let peer_members = store_proposed_circuit.list_nodes().map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get peer tokens for members: {}", err),
                )))
            })?;

            for node in &peer_members {
                let peer_token_pair =
                    PeerTokenPair::new(node.token.clone(), local_required_auth.clone());
                if !self.is_local_node(peer_token_pair.peer_id()) {
                    debug!("Referencing node {:?}", &peer_token_pair);
                    let peer_ref = self
                        .peer_connector
                        .add_peer_ref(
                            node.token.clone(),
                            node.endpoints.to_vec(),
                            local_required_auth.clone(),
                        )
                        .map_err(|err| {
                            // remove all peer refs added for this proposal
                            self.remove_peer_refs(added_peers.to_vec());

                            ServiceError::UnableToHandleMessage(Box::new(err))
                        })?;

                    self.add_peer_ref(peer_ref);
                    added_peers.push(peer_token_pair.clone());

                    // if we have a protocol the connection exists for the peer already
                    if self.service_protocols.get(&peer_token_pair).is_none() {
                        pending_peers.push(peer_token_pair.clone());
                        missing_protocol_ids.push(node.clone())
                    }
                }
                pending_members.push(peer_token_pair.clone());

                self.token_to_peer.insert(
                    peer_token_pair,
                    PeerNodePair {
                        peer_node: node.clone(),
                        local_peer_token: local_required_auth.clone(),
                    },
                );
            }
            members.extend(peer_members);
        } else if payload.has_circuit_disband_request() {
            // If a `CircuitDisbandRequest` is present in the payload, the members must be gathered
            // from the admin store based on the provided circuit id.
            // If the members list has already been updated, the payload was to create a
            // new circuit.
            if !members.is_empty() {
                return Err(ServiceError::UnableToHandleMessage(Box::new(
                    AdminSharedError::ValidationFailed(
                        "Invalid payload; has two requests".to_string(),
                    ),
                )));
            }
            let circuit_id = payload.get_circuit_disband_request().get_circuit_id();
            // If the proposed circuit is being disbanded, the circuit information must be
            // gathered from the admin store, as the `CircuitDisbandRequest` only contains
            // the `circuit_id`.
            let circuit = self
                .admin_store
                .get_circuit(circuit_id)
                .map_err(|err| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(format!(
                            "error occurred when trying to get circuit {}",
                            err
                        )),
                    ))
                })?
                .ok_or_else(|| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(format!(
                            "unable to get circuit {}",
                            circuit_id
                        )),
                    ))
                })?;

            let local_required_auth = circuit
                .get_node_token(&self.node_id)
                .map_err(|err| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(format!(
                            "Unable to get local nodes token: {}",
                            err
                        )),
                    ))
                })?
                .ok_or_else(|| {
                    ServiceError::UnableToHandleMessage(Box::new(
                        AdminSharedError::ValidationFailed(
                            "Circuit does not have the local node".to_string(),
                        ),
                    ))
                })?;

            let tokens = circuit.list_nodes().map_err(|err| {
                ServiceError::UnableToHandleMessage(Box::new(AdminSharedError::ValidationFailed(
                    format!("Unable to get peer tokens for members: {}", err),
                )))
            })?;

            for node in tokens {
                let peer_token_pair =
                    PeerTokenPair::new(node.token.clone(), local_required_auth.clone());
                // Verify each disband member has an agreed upon protocol version with this node
                // Otherwise, re-establish a peer connection
                if !self.is_local_node(peer_token_pair.peer_id())
                    && self.service_protocols.get(&peer_token_pair).is_none()
                {
                    pending_peers.push(peer_token_pair.clone());
                    missing_protocol_ids.push(node.clone())
                }
            }
        }

        if missing_protocol_ids.is_empty() {
            self.add_pending_consensus_proposal(proposal.id.clone(), (proposal.clone(), payload));
            self.proposal_sender
                .as_ref()
                .ok_or(ServiceError::NotStarted)?
                .send(ProposalUpdate::ProposalReceived(
                    proposal,
                    message_sender.as_bytes().into(),
                ))
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
        } else {
            debug!(
                "Members {:?} added; service protocol agreement before proceeding",
                &missing_protocol_ids
            );

            self.pending_protocol_payloads.push(PendingPayload {
                unpeered_ids: pending_peers,
                missing_protocol_ids,
                payload_type: PayloadType::Consensus(proposal.id.clone(), (proposal, payload)),
                members: pending_members,
                message_sender,
            });
            Ok(())
        }
    }

    #[cfg(not(feature = "admin-service-event-subscriber-glob"))]
    pub fn get_events_since(
        &self,
        since_event_id: &i64,
        circuit_management_type: &str,
    ) -> Result<Events, AdminSharedError> {
        let events = self
            .event_store
            .list_events_by_management_type_since(
                circuit_management_type.to_string(),
                *since_event_id,
            )
            .map_err(|err| AdminSharedError::UnableToAddSubscriber(err.to_string()))?;
        Ok(Events {
            inner: Box::new(events),
        })
    }

    #[cfg(feature = "admin-service-event-subscriber-glob")]
    pub fn get_events_since(
        &self,
        since_event_id: &i64,
        circuit_management_type: &str,
    ) -> Result<Events, AdminSharedError> {
        let events = if circuit_management_type == "*" {
            self.event_store
                .list_events_since(*since_event_id)
                .map_err(|err| AdminSharedError::UnableToAddSubscriber(err.to_string()))?
        } else {
            self.event_store
                .list_events_by_management_type_since(
                    circuit_management_type.to_string(),
                    *since_event_id,
                )
                .map_err(|err| AdminSharedError::UnableToAddSubscriber(err.to_string()))?
        };
        Ok(Events {
            inner: Box::new(events),
        })
    }

    pub fn add_subscriber(
        &mut self,
        circuit_management_type: String,
        subscriber: Box<dyn AdminServiceEventSubscriber>,
    ) -> Result<(), AdminSharedError> {
        self.event_subscribers
            .add_subscriber(circuit_management_type, subscriber);

        Ok(())
    }

    pub fn send_event(
        &mut self,
        circuit_management_type: &str,
        event: messages::AdminServiceEvent,
    ) {
        let admin_event = match self.event_store.add_event(event) {
            Ok(admin_event) => admin_event,
            Err(err) => {
                error!("Unable to store admin event: {}", err);
                return;
            }
        };

        self.event_subscribers
            .broadcast_by_type(circuit_management_type, &admin_event);
    }

    pub fn remove_all_event_subscribers(&mut self) {
        self.event_subscribers.clear();
    }

    pub fn on_peer_disconnected(&mut self, peer_id: PeerTokenPair) {
        if let Some(peer_node_pair) = self.token_to_peer.remove(&peer_id) {
            self.service_protocols.remove(&peer_id);
            let mut pending_protocol_payloads = std::mem::take(&mut self.pending_protocol_payloads);

            // Add peer back to any pending payloads
            for pending_protocol_payload in pending_protocol_payloads.iter_mut() {
                if pending_protocol_payload.members.contains(&peer_id) {
                    pending_protocol_payload
                        .missing_protocol_ids
                        .push(peer_node_pair.peer_node.clone())
                }
            }

            let (peering, protocol): (Vec<PendingPayload>, Vec<PendingPayload>) =
                pending_protocol_payloads
                    .into_iter()
                    .partition(|pending_payload| {
                        pending_payload
                            .missing_protocol_ids
                            .contains(&peer_node_pair.peer_node)
                    });

            self.pending_protocol_payloads = protocol;
            // Add peer back to any pending payloads
            let mut unpeered_payloads = std::mem::take(&mut self.unpeered_payloads);
            for unpeered_payload in unpeered_payloads.iter_mut() {
                if unpeered_payload.members.contains(&peer_id) {
                    unpeered_payload.unpeered_ids.push(peer_id.clone())
                }
            }
            // add payloads that are not waiting on peer connection
            unpeered_payloads.extend(peering);
            self.unpeered_payloads = unpeered_payloads;
        }
    }

    pub fn on_peer_connected(&mut self, peer_id: &PeerTokenPair) -> Result<(), AdminSharedError> {
        let mut unpeered_payloads = std::mem::take(&mut self.unpeered_payloads);
        for unpeered_payload in unpeered_payloads.iter_mut() {
            unpeered_payload
                .unpeered_ids
                .retain(|unpeered_id| unpeered_id != peer_id);
        }

        let (fully_peered, still_unpeered): (Vec<PendingPayload>, Vec<PendingPayload>) =
            unpeered_payloads
                .into_iter()
                .partition(|unpeered_payload| unpeered_payload.unpeered_ids.is_empty());

        self.unpeered_payloads = still_unpeered;
        for peered_payload in fully_peered {
            self.pending_protocol_payloads.push(peered_payload);
        }

        // Ignore own admin service
        if self.is_local_admin_service(peer_id) {
            return Ok(());
        }

        let peer_node_pair = match self.token_to_peer.get(peer_id) {
            Some(peer_node_pair) => peer_node_pair,
            None => {
                warn!(
                    "Ignoring connection; missing service information for peer token: {}",
                    peer_id
                );
                return Ok(());
            }
        };

        // We have already received a service protocol request, don't sent another request
        if self.service_protocols.get(peer_id).is_some() {
            return Ok(());
        }

        // Send protocol request
        if let Some(ref mut network_sender) = &mut self.network_sender {
            debug!(
                "Sending service protocol request to {}",
                peer_node_pair.peer_node.admin_service
            );
            let mut request = ServiceProtocolVersionRequest::new();
            request.set_protocol_min(ADMIN_SERVICE_PROTOCOL_MIN);
            request.set_protocol_max(ADMIN_SERVICE_PROTOCOL_VERSION);
            let mut msg = AdminMessage::new();
            msg.set_message_type(AdminMessage_Type::SERVICE_PROTOCOL_VERSION_REQUEST);
            msg.set_protocol_request(request);

            let envelope_bytes = msg.write_to_bytes().map_err(|err| {
                AdminSharedError::ServiceProtocolError(format!(
                    "Unable to send service protocol request: {}",
                    err
                ))
            })?;

            // need to set the sender to our local auth that is being used
            network_sender
                .send_with_sender(
                    &admin_service_id(&peer_id.id_as_string()),
                    &envelope_bytes,
                    &admin_service_id(
                        &PeerTokenPair::new(
                            peer_node_pair.local_peer_token.clone(),
                            peer_id.peer_id().clone(),
                        )
                        .id_as_string(),
                    ),
                )
                .map_err(|err| {
                    AdminSharedError::ServiceProtocolError(format!(
                        "Unable to send service protocol request: {}",
                        err
                    ))
                })?;
        } else {
            return Err(AdminSharedError::ServiceProtocolError(format!(
                "AdminService is not started, can't send request to {}",
                peer_id
            )));
        }

        Ok(())
    }

    pub fn on_protocol_agreement(
        &mut self,
        service_id: &str,
        protocol: u32,
    ) -> Result<(), AdminSharedError> {
        // parse the admin service to know if the peer token is trust or challenge
        let peer_token =
            get_peer_token_from_service_id(service_id, &self.node_id).map_err(|err| {
                AdminSharedError::ServiceProtocolError(format!(
                    "Unable to verify peer token for service id: {}",
                    err
                ))
            })?;

        // if trust the service ID remains the same, if challenge need to get the actual service
        // ID from the known peer
        let service_id = match peer_token.peer_id() {
            PeerAuthorizationToken::Trust { .. } => service_id.to_string(),
            PeerAuthorizationToken::Challenge { .. } => {
                if let Some(peer_node_pair) = self.token_to_peer.get(&peer_token) {
                    peer_node_pair.peer_node.admin_service.to_string()
                } else {
                    // If the peer is unknown add the service ID as is, it will be replaced once
                    // the peer is known
                    service_id.to_string()
                }
            }
        };

        self.update_pending_for_protocol_agreement(service_id, peer_token, protocol)
    }

    fn update_pending_for_protocol_agreement(
        &mut self,
        service_id: String,
        token: PeerTokenPair,
        protocol: u32,
    ) -> Result<(), AdminSharedError> {
        // Update any unpeered payloads that this service might be a member of
        let mut unpeered_payloads = std::mem::take(&mut self.unpeered_payloads);
        for pending_protocol_payload in unpeered_payloads.iter_mut() {
            match protocol {
                0 => {
                    if pending_protocol_payload
                        .missing_protocol_ids
                        .iter()
                        .any(|missing_protocol_id| missing_protocol_id.admin_service == service_id)
                    {
                        warn!(
                            "Dropping circuit request including service {}, \
                             due to protocol mismatch",
                            service_id
                        );
                        pending_protocol_payload.missing_protocol_ids.clear();
                    }
                }
                _ => {
                    debug!(
                        "Agreed with {} to use protocol version {}",
                        service_id, protocol
                    );
                    pending_protocol_payload
                        .missing_protocol_ids
                        .retain(|missing_protocol_id| {
                            missing_protocol_id.admin_service != service_id
                        });
                }
            }
        }

        // Failed peers are those that have had the missing protocol ids cleared, so we remove them.
        unpeered_payloads
            .retain(|pending_payload| !pending_payload.missing_protocol_ids.is_empty());

        self.unpeered_payloads = unpeered_payloads;

        // update the fully peered but pending protocol payloads.
        let mut pending_protocol_payloads = std::mem::take(&mut self.pending_protocol_payloads);
        for pending_protocol_payload in pending_protocol_payloads.iter_mut() {
            match protocol {
                0 => {
                    if pending_protocol_payload
                        .missing_protocol_ids
                        .iter()
                        .any(|missing_protocol_id| missing_protocol_id.admin_service == service_id)
                    {
                        warn!(
                            "Dropping circuit request including service {}, \
                             due to protocol mismatch",
                            service_id
                        );
                        pending_protocol_payload.missing_protocol_ids.clear();
                    }
                }
                _ => {
                    debug!(
                        "Agreed with {} to use protocol version {}",
                        service_id, protocol
                    );
                    pending_protocol_payload
                        .missing_protocol_ids
                        .retain(|missing_protocol_id| {
                            missing_protocol_id.admin_service != service_id
                        });
                }
            }
        }

        let (ready, waiting): (Vec<PendingPayload>, Vec<PendingPayload>) =
            pending_protocol_payloads
                .into_iter()
                .partition(|pending_payload| pending_payload.missing_protocol_ids.is_empty());

        self.pending_protocol_payloads = waiting;

        if protocol == 0 {
            // if no agreed protocol, remove all peer refs for proposals
            for pending_payload in ready {
                self.remove_peer_refs(pending_payload.members.to_vec());
            }
            return Ok(());
        }

        self.service_protocols.insert(token, protocol);
        for pending_payload in ready {
            match pending_payload.payload_type {
                PayloadType::Circuit(payload) => self.pending_circuit_payloads.push_back(payload),
                PayloadType::Consensus(id, (proposal, payload)) => {
                    self.add_pending_consensus_proposal(id, (proposal.clone(), payload));

                    // Admin service should always will always be started at this point
                    if let Some(proposal_sender) = &self.proposal_sender {
                        proposal_sender
                            .send(ProposalUpdate::ProposalReceived(
                                proposal,
                                pending_payload.message_sender.as_bytes().into(),
                            ))
                            .map_err(|err| {
                                AdminSharedError::ServiceProtocolError(format!(
                                    "Unable to send consensus proposal update: {}",
                                    err
                                ))
                            })?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_proposal(
        &self,
        circuit_id: &str,
    ) -> Result<Option<StoreProposal>, AdminSharedError> {
        Ok(self.admin_store.get_proposal(circuit_id)?)
    }

    pub fn get_proposals(
        &self,
        filters: &[CircuitPredicate],
    ) -> Result<Box<dyn ExactSizeIterator<Item = StoreProposal>>, AdminSharedError> {
        self.admin_store
            .list_proposals(filters)
            .map_err(AdminSharedError::from)
    }

    pub fn remove_proposal(
        &mut self,
        circuit_id: &str,
    ) -> Result<Option<StoreProposal>, AdminSharedError> {
        let proposal = self.admin_store.get_proposal(circuit_id)?;
        self.admin_store.remove_proposal(circuit_id)?;
        Ok(proposal)
    }

    pub fn add_proposal(
        &mut self,
        circuit_proposal: CircuitProposal,
    ) -> Result<(), AdminSharedError> {
        Ok(self
            .admin_store
            .add_proposal(StoreProposal::from_proto(circuit_proposal).map_err(|err| {
                AdminSharedError::SplinterStateError(format!("Unable to add proposal: {}", err))
            })?)?)
    }

    pub fn update_proposal(
        &mut self,
        circuit_proposal: CircuitProposal,
    ) -> Result<(), AdminSharedError> {
        Ok(self.admin_store.update_proposal(
            StoreProposal::from_proto(circuit_proposal).map_err(|err| {
                AdminSharedError::SplinterStateError(format!("Unable to update proposal: {}", err))
            })?,
        )?)
    }

    /// Use the internal `admin_store`'s `remove_circuit` method
    pub fn remove_circuit(
        &mut self,
        circuit_id: &str,
    ) -> Result<Option<StoreCircuit>, AdminSharedError> {
        let circuit = self.admin_store.get_circuit(circuit_id)?;
        self.admin_store.remove_circuit(circuit_id)?;
        Ok(circuit)
    }

    /// Add a circuit definition as an uninitialized circuit. If all members are ready, verify
    /// the proposal type to check if we are creating a circuit and will initialize the services
    /// or if the proposal type is to disband a circuit, in which case the services are stopped.
    fn add_uninitialized_circuit(
        &mut self,
        circuit: CircuitProposal,
    ) -> Result<(), AdminSharedError> {
        let circuit_id = circuit.get_circuit_id().to_string();
        let circuit_proposal_type = circuit.get_proposal_type();
        // If uninitialized circuit already exists, add the circuit definition; if not, create the
        // uninitialized circuit.
        match self.uninitialized_circuits.get_mut(&circuit_id) {
            Some(uninit_circuit) => uninit_circuit.circuit = Some(circuit),
            None => {
                self.uninitialized_circuits.insert(
                    circuit_id.to_string(),
                    UninitializedCircuit {
                        circuit: Some(circuit),
                        ready_members: HashSet::new(),
                    },
                );
            }
        }

        // Add self as ready
        self.uninitialized_circuits
            .get_mut(&circuit_id)
            .expect("Uninitialized circuit not set")
            .ready_members
            .insert(self.node_id.clone());

        // Check if members are ready for the next step of the proposal, based on the proposal's
        // type. If the proposal has type `CircuitProposal_ProposalType::CREATE`, the proposal
        // is intended to create a circuit and the associated services need to be initialized.
        // In this case, the next step is to `initialize_services_if_members_ready`.
        // If the proposal has type `CircuitProposal_ProposalType::DISBAND`, the proposal is
        // intended to disband a circuit and the associated services will need to be stopped. In
        // this case, the next step is to `cleanup_disbanded_circuit_if_members_ready`.
        if circuit_proposal_type == CircuitProposal_ProposalType::DISBAND {
            self.cleanup_disbanded_circuit_if_members_ready(&circuit_id)
        } else {
            self.initialize_services_if_members_ready(&circuit_id)
        }
    }

    /// A member may be ready to initialize a circuit or to disband a circuit. The proposal type
    /// of the proposal associated with the `circuit_id` determines the operation the member
    /// voted for. If the proposal type is `Create`, the vote submitted pertains to creating a
    /// circuit so the services must be initialized if all members are now ready. If the proposal
    /// type is disband, the vote submitted pertains to disbanding a circuit so the services
    /// must be stopped if all members are now ready.
    pub fn add_ready_member(
        &mut self,
        circuit_id: &str,
        member_node_id: String,
    ) -> Result<(), AdminSharedError> {
        // The requesting node will have already updated their state, so the associated proposal
        // may not be available. In this case, the proposal type must be pulled from the circuit
        // proposal stored in the `uninitialized_circuits` list.
        let mut proposal_type = ProposalType::Create;
        if let Some(proposal) = self.get_proposal(circuit_id)? {
            proposal_type = proposal.proposal_type().clone();
        } else if let Some(uninit_circuit) = self.uninitialized_circuits.get(circuit_id) {
            if let Some(circuit) = &uninit_circuit.circuit {
                proposal_type =
                    ProposalType::try_from(&circuit.get_proposal_type()).map_err(|_| {
                        AdminSharedError::SplinterStateError(
                            "CircuitProposal proto's ProposalType is unset".to_string(),
                        )
                    })?
            }
        }

        // If uninitialized circuit does not already exist, create it
        if self.uninitialized_circuits.get(circuit_id).is_none() {
            self.uninitialized_circuits.insert(
                circuit_id.to_string(),
                UninitializedCircuit {
                    circuit: None,
                    ready_members: HashSet::new(),
                },
            );
        }

        self.uninitialized_circuits
            .get_mut(circuit_id)
            .expect("Uninitialized circuit not set")
            .ready_members
            .insert(member_node_id);

        // Move onto either initializing the services or stopping the services, depending on the
        // associated circuit proposal's type.
        match proposal_type {
            ProposalType::Disband => self.cleanup_disbanded_circuit_if_members_ready(circuit_id),
            _ => self.initialize_services_if_members_ready(circuit_id),
        }
    }

    /// If all members of an uninitialized circuit are ready, initialize services. Also send
    /// CircuitReady notification to application authorization handler.
    fn initialize_services_if_members_ready(
        &mut self,
        circuit_id: &str,
    ) -> Result<(), AdminSharedError> {
        let ready = {
            if let Some(uninitialized_circuit) = self.uninitialized_circuits.get(circuit_id) {
                if let Some(ref circuit_proposal) = uninitialized_circuit.circuit {
                    let all_members = circuit_proposal
                        .get_circuit_proposal()
                        .members
                        .iter()
                        .map(|node| node.node_id.clone())
                        .collect::<HashSet<String>>();
                    all_members.is_subset(&uninitialized_circuit.ready_members)
                } else {
                    false
                }
            } else {
                false
            }
        };

        if ready {
            let circuit_proposal = self
                .uninitialized_circuits
                .remove(circuit_id)
                .expect("Uninitialized circuit not set")
                .circuit
                .expect("Uninitialized circuit's circuit proposal not set");
            self.initialize_services(circuit_proposal.get_circuit_proposal())?;

            let mgmt_type = circuit_proposal
                .get_circuit_proposal()
                .circuit_management_type
                .clone();
            let event = messages::AdminServiceEvent::CircuitReady(
                messages::CircuitProposal::from_proto(circuit_proposal)?,
            );
            self.send_event(&mgmt_type, event);
        }

        Ok(())
    }

    fn validate_create_circuit(
        &self,
        circuit: &Circuit,
        signer_public_key: &[u8],
        requester_node_id: &str,
        protocol: u32,
    ) -> Result<(), AdminSharedError> {
        match protocol {
            ADMIN_SERVICE_PROTOCOL_VERSION => {
                // verify that the circuit version is supported
                if circuit.get_circuit_version() > CIRCUIT_PROTOCOL_VERSION {
                    return Err(AdminSharedError::ValidationFailed(format!(
                        "Proposed circuit's schema version is unsupported: {}",
                        circuit.get_circuit_version()
                    )));
                }
            }

            1 => {
                // if using the previous version, display name cannot be set
                if !circuit.get_display_name().is_empty() {
                    return Err(AdminSharedError::ValidationFailed(
                        "Proposed circuit cannot have a display name on protocol 1".to_string(),
                    ));
                } else if circuit.get_circuit_status()
                    != Circuit_CircuitStatus::UNSET_CIRCUIT_STATUS
                {
                    return Err(AdminSharedError::ValidationFailed(
                        "Proposed circuit cannot have a circuit status on protocol 1".to_string(),
                    ));
                }
                // check that the circuit includes supported versions
                match circuit.get_circuit_version() {
                    0 => (),
                    _ => {
                        return Err(AdminSharedError::ValidationFailed(
                            "Proposed circuit schema version is not supported by protocol 1"
                                .to_string(),
                        ))
                    }
                }
            }
            // Unsupported version, this should never happen
            _ => {
                return Err(AdminSharedError::ServiceProtocolError(format!(
                    "Agreed upon unsupported protocol version: {}",
                    protocol
                )))
            }
        }

        if requester_node_id.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "requester_node_id is empty".to_string(),
            ));
        }

        self.validate_key(signer_public_key)?;

        if !self
            .key_verifier
            .is_permitted(requester_node_id, signer_public_key)?
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for the requester node {}",
                to_hex(signer_public_key),
                requester_node_id,
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, PROPOSER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to vote for node {}",
                    to_hex(signer_public_key),
                    requester_node_id
                ))
            })?;

        if self.has_proposal(circuit.get_circuit_id())? {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Ignoring duplicate proposal for circuit {}",
                circuit.get_circuit_id()
            )));
        }

        if self
            .admin_store
            .get_circuit(circuit.get_circuit_id())?
            .is_some()
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Circuit with circuit id {} already exists",
                circuit.get_circuit_id()
            )));
        }

        self.validate_circuit(circuit)?;
        Ok(())
    }

    fn validate_key(&self, public_key: &[u8]) -> Result<(), AdminSharedError> {
        if public_key.len() != 33 {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not a valid public key: invalid length",
                to_hex(public_key)
            )));
        }

        Ok(())
    }

    fn validate_circuit(&self, circuit: &Circuit) -> Result<(), AdminSharedError> {
        if circuit.get_authorization_type() == Circuit_AuthorizationType::UNSET_AUTHORIZATION_TYPE {
            return Err(AdminSharedError::ValidationFailed(
                "authorization_type cannot be unset".to_string(),
            ));
        }

        if circuit.get_circuit_version() < CIRCUIT_PROTOCOL_VERSION
            && circuit.get_authorization_type()
                == Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "authorization_type CHALLENGE is not support in circuit schema version {}",
                circuit.get_circuit_version()
            )));
        }

        if circuit.get_persistence() == Circuit_PersistenceType::UNSET_PERSISTENCE_TYPE {
            return Err(AdminSharedError::ValidationFailed(
                "persistence_type cannot be unset".to_string(),
            ));
        }

        if circuit.get_durability() == Circuit_DurabilityType::UNSET_DURABILITY_TYPE {
            return Err(AdminSharedError::ValidationFailed(
                "durability_type cannot be unset".to_string(),
            ));
        }

        if circuit.get_routes() == Circuit_RouteType::UNSET_ROUTE_TYPE {
            return Err(AdminSharedError::ValidationFailed(
                "route_type cannot be unset".to_string(),
            ));
        }

        if circuit.get_circuit_id().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "circuit_id must be set".to_string(),
            ));
        }
        if !messages::is_valid_circuit_id(circuit.get_circuit_id()) {
            return Err(AdminSharedError::ValidationFailed(format!(
                "'{}' is not a valid circuit ID: must be an 11 character string compose of two, 5 \
                 character base62 strings joined with a '-' (example: abcDE-F0123)",
                circuit.get_circuit_id(),
            )));
        }

        if circuit.get_circuit_management_type().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "circuit_management_type must be set".to_string(),
            ));
        }

        let mut members: Vec<String> = Vec::new();
        let mut all_endpoints: Vec<String> = Vec::new();
        for member in circuit.get_members() {
            let node_id = member.get_node_id().to_string();
            if node_id.is_empty() {
                return Err(AdminSharedError::ValidationFailed(
                    "Member node id cannot be empty".to_string(),
                ));
            } else if members.contains(&node_id) {
                return Err(AdminSharedError::ValidationFailed(
                    "Every member must be unique in the circuit.".to_string(),
                ));
            } else {
                members.push(node_id);
            }

            let mut endpoints = member.get_endpoints().to_vec();
            if endpoints.is_empty() {
                return Err(AdminSharedError::ValidationFailed(
                    "Member endpoints cannot be empty".to_string(),
                ));
            } else if endpoints.iter().any(|endpoint| endpoint.is_empty()) {
                return Err(AdminSharedError::ValidationFailed(
                    "Member cannot have an empty endpoint".to_string(),
                ));
            } else if endpoints
                .iter()
                .any(|endpoint| all_endpoints.contains(endpoint))
            {
                return Err(AdminSharedError::ValidationFailed(
                    "Every member endpoint must be unique in the circuit.".to_string(),
                ));
            } else {
                all_endpoints.append(&mut endpoints);
            }

            if circuit.get_authorization_type()
                == Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION
                && member.get_public_key().is_empty()
            {
                return Err(AdminSharedError::ValidationFailed(
                    "All members must have public keys if authorization type is challenge"
                        .to_string(),
                ));
            }
        }

        if members.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "The circuit must have members".to_string(),
            ));
        }

        // check this node is in members
        if !members.contains(&self.node_id) {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Circuit does not contain this node: {}",
                self.node_id
            )));
        }

        if circuit.get_roster().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "The circuit must have services".to_string(),
            ));
        }

        let mut services: Vec<String> = Vec::new();
        // check that all services' allowed nodes are in members
        for service in circuit.get_roster() {
            if service.get_allowed_nodes().is_empty() {
                return Err(AdminSharedError::ValidationFailed(
                    "Service cannot have an empty allowed nodes list".to_string(),
                ));
            }

            if service.get_allowed_nodes().len() > 1 {
                return Err(AdminSharedError::ValidationFailed(
                    "Only one allowed node for a service is supported".to_string(),
                ));
            }

            for node in service.get_allowed_nodes() {
                if !members.contains(node) {
                    return Err(AdminSharedError::ValidationFailed(format!(
                        "Service cannot have an allowed node that is not in members: {}",
                        node
                    )));
                }
            }

            let service_id = service.get_service_id().to_string();
            if service_id.is_empty() {
                return Err(AdminSharedError::ValidationFailed(
                    "Service id cannot be empty".to_string(),
                ));
            } else if !messages::is_valid_service_id(&service_id) {
                return Err(AdminSharedError::ValidationFailed(format!(
                    "'{}' is not a valid service ID: must be a 4 character base62 string",
                    service_id,
                )));
            } else if services.contains(&service_id) {
                return Err(AdminSharedError::ValidationFailed(
                    "Every service must be unique in the circuit.".to_string(),
                ));
            } else {
                services.push(service_id)
            }

            self.validate_service_args(service)?;
        }

        if circuit.get_circuit_management_type().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "The circuit must have a mangement type".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_service_args(&self, service: &SplinterService) -> Result<(), AdminSharedError> {
        if let Some(validator) = self.service_arg_validators.get(service.get_service_type()) {
            let args: HashMap<String, String> = service
                .get_arguments()
                .iter()
                .map(|arg| (arg.get_key().into(), arg.get_value().into()))
                .collect();

            validator
                .validate(&args)
                .map_err(|err| AdminSharedError::ValidationFailed(err.to_string()))
        } else {
            Ok(())
        }
    }

    fn validate_circuit_vote(
        &self,
        proposal_vote: &CircuitProposalVote,
        signer_public_key: &[u8],
        circuit_proposal: &StoreProposal,
        node_id: &str,
    ) -> Result<(), AdminSharedError> {
        if circuit_proposal.proposal_type() == &ProposalType::Create {
            let circuit = circuit_proposal.circuit();
            // verify that the circuit version is supported
            match circuit.circuit_version() {
                1 | CIRCUIT_PROTOCOL_VERSION => (),
                _ => {
                    return Err(AdminSharedError::ValidationFailed(format!(
                        "Proposed circuit's schema version is unsupported: {}",
                        circuit.circuit_version()
                    )));
                }
            }
        };

        let circuit_hash = proposal_vote.get_circuit_hash();

        self.validate_key(signer_public_key)?;

        if !self.key_verifier.is_permitted(node_id, signer_public_key)? {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for voting node {}",
                to_hex(signer_public_key),
                node_id,
            )));
        }

        if circuit_proposal.requester_node_id() == node_id {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Received vote from requester node: {}",
                to_hex(circuit_proposal.requester().as_slice())
            )));
        }

        let voted_nodes: Vec<String> = circuit_proposal
            .votes()
            .iter()
            .map(|vote| vote.voter_node_id().to_string())
            .collect();

        if voted_nodes.iter().any(|node| *node == node_id) {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Received duplicate vote from {} for {}",
                node_id, proposal_vote.circuit_id
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, VOTER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to vote for node {}",
                    to_hex(signer_public_key),
                    node_id
                ))
            })?;

        // validate hash of circuit
        if circuit_proposal.circuit_hash() != circuit_hash {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Hash of circuit does not match circuit proposal: {}",
                proposal_vote.circuit_id
            )));
        }

        Ok(())
    }

    /// Validates a `CircuitDisbandRequest` using the following:
    ///
    /// - Validate the protocol version used by the submitter node. Currently, disbanding is only
    ///   available to nodes using `ADMIN_SERVICE_PROTOCOL_VERSION` 2.
    /// - Validate the requester is authorized to propose a change for the requesting node
    /// - Validate the signer's public key is authorized for the requesting node
    /// - Validate a `CircuitProposal` with the same ID is not present
    /// - Validate the circuit being disbanded has a valid `circuit_version` and `circuit_status`.
    ///   A circuit must have a `circuit_version` of at least 2 and a `circuit_status` of `Active`
    ///   in order to be disbanded.
    fn validate_disband_circuit(
        &self,
        circuit: &Circuit,
        signer_public_key: &[u8],
        requester_node_id: &str,
        protocol: u32,
    ) -> Result<(), AdminSharedError> {
        if protocol != ADMIN_SERVICE_PROTOCOL_VERSION {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Circuit-Disband is not available for protocol version {}",
                protocol
            )));
        }

        if requester_node_id.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "requester_node_id is empty".to_string(),
            ));
        }

        self.validate_key(signer_public_key)?;

        if !self
            .key_verifier
            .is_permitted(requester_node_id, signer_public_key)?
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for the requester node {}",
                to_hex(signer_public_key),
                requester_node_id,
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, PROPOSER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to disband for node {}",
                    to_hex(signer_public_key),
                    requester_node_id
                ))
            })?;

        if self.has_proposal(circuit.get_circuit_id())? {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Ignoring duplicate proposal for circuit {}",
                circuit.get_circuit_id()
            )));
        }

        // Verifying the circuit has not already been disbanded or abandoned and has a valid
        // version to perform the disband request
        let stored_circuit = self
            .admin_store
            .get_circuit(circuit.get_circuit_id())
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to get circuit {}",
                    err
                ))
            })?
            .ok_or_else(|| {
                AdminSharedError::ValidationFailed(format!(
                    "Received disband request for a circuit that does not exist: circuit id {}",
                    circuit.get_circuit_id()
                ))
            })?;

        if stored_circuit.circuit_status() != &StoreCircuitStatus::Active {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Attempting to disband an inactive circuit {}",
                circuit.get_circuit_id()
            )));
        }

        if stored_circuit.circuit_version() < CIRCUIT_PROTOCOL_VERSION {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Attempting to disband a circuit with schema version {}, must be {}",
                stored_circuit.circuit_version(),
                CIRCUIT_PROTOCOL_VERSION,
            )));
        }

        Ok(())
    }

    /// Validates a `CircuitPurgeRequest` using the following:
    ///
    /// - Validate the requester is authorized to propose a change on the requesting node
    /// - Validate the signer's public key is authorized for the requesting node
    /// - Validate the circuit being purged has a valid `circuit_status`.
    ///   A circuit must have a `circuit_status` of `Disbanded` or `Abandoned` in order to be
    ///   purged.
    fn validate_purge_request(
        &self,
        circuit_id: &str,
        signer_public_key: &[u8],
        requester_node_id: &str,
    ) -> Result<(), AdminSharedError> {
        if requester_node_id.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "requester_node_id is empty".to_string(),
            ));
        }

        if requester_node_id != self.node_id {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Unable to purge circuit from node {}: request came from node {}",
                self.node_id, requester_node_id
            )));
        }

        self.validate_key(signer_public_key)?;

        if !self
            .key_verifier
            .is_permitted(requester_node_id, signer_public_key)?
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for the requester node {}",
                to_hex(signer_public_key),
                requester_node_id,
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, PROPOSER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to propose change for node {}",
                    to_hex(signer_public_key),
                    requester_node_id
                ))
            })?;

        // Verifying the circuit is `Disbanded` and able to be purged
        let stored_circuit = self
            .admin_store
            .get_circuit(circuit_id)
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to get circuit {}",
                    err
                ))
            })?
            .ok_or_else(|| {
                AdminSharedError::ValidationFailed(format!(
                    "Received purged request for a circuit that does not exist: circuit id {}",
                    circuit_id
                ))
            })?;

        if stored_circuit.circuit_status() == &StoreCircuitStatus::Active {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Attempting to purge a circuit that is still active: {}",
                circuit_id
            )));
        }

        Ok(())
    }

    /// Validate a `CircuitAbandon` payload by the following:
    ///
    /// - Validate the requester is authorized to propose a change for the requesting node
    /// - Validate the signer's public key is authorized for the requesting node
    /// - Validate the circuit being abandoned has a valid `circuit_status`.
    ///   A circuit must have a `circuit_status` of `Active` in order to be abandoned.
    ///
    /// Note: abandoning a circuit on protocol version 1 and circuit version 1 is allowed because
    /// abandon does not require communication with other nodes.
    fn validate_abandon_circuit(
        &self,
        circuit_id: &str,
        signer_public_key: &[u8],
        requester_node_id: &str,
    ) -> Result<(), AdminSharedError> {
        if requester_node_id.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "requester_node_id is empty".to_string(),
            ));
        }

        if requester_node_id != self.node_id {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Unable to abandon circuit from node {}: request came from node {}",
                self.node_id, requester_node_id
            )));
        }

        self.validate_key(signer_public_key)?;

        if !self
            .key_verifier
            .is_permitted(requester_node_id, signer_public_key)?
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for the requester node {}",
                to_hex(signer_public_key),
                requester_node_id,
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, PROPOSER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to propose change for node {}",
                    to_hex(signer_public_key),
                    requester_node_id
                ))
            })?;

        // Verifying the circuit is available in the admin store, `Active`, and able to be abandoned
        let stored_circuit = self
            .admin_store
            .get_circuit(circuit_id)
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to get circuit {}",
                    err
                ))
            })?
            .ok_or_else(|| {
                AdminSharedError::ValidationFailed(format!(
                    "Received abandon request for a circuit that does not exist: circuit id {}",
                    circuit_id
                ))
            })?;

        if stored_circuit.circuit_status() != &StoreCircuitStatus::Active {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Attempting to abandon a circuit that is not active: {}",
                circuit_id
            )));
        }

        Ok(())
    }

    /// Validate a `ProposalRemoveRequest` payload by the following:
    ///
    /// - Validate the requester is authorized to propose a change for the requesting node
    /// - Validate the signer's public key is authorized for the requesting node
    /// - Validate the proposal being removed exists
    ///
    /// Note: removing a proposal on protocol version 1 and circuit version 1 is allowed because
    /// abandon does not require communication with other nodes.
    fn validate_remove_proposal(
        &self,
        circuit_id: &str,
        signer_public_key: &[u8],
        requester_node_id: &str,
    ) -> Result<(), AdminSharedError> {
        if requester_node_id.is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "requester_node_id is empty".to_string(),
            ));
        }

        if requester_node_id != self.node_id {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Unable to remove proposal from node {}: request came from node {}",
                self.node_id, requester_node_id
            )));
        }

        self.validate_key(signer_public_key)?;

        if !self
            .key_verifier
            .is_permitted(requester_node_id, signer_public_key)?
        {
            return Err(AdminSharedError::ValidationFailed(format!(
                "{} is not registered for the requester node {}",
                to_hex(signer_public_key),
                requester_node_id,
            )));
        }

        self.key_permission_manager
            .is_permitted(signer_public_key, PROPOSER_ROLE)
            .map_err(|_| {
                AdminSharedError::ValidationFailed(format!(
                    "{} is not permitted to propose change for node {}",
                    to_hex(signer_public_key),
                    requester_node_id
                ))
            })?;

        if self.get_proposal(circuit_id)?.is_none() {
            return Err(AdminSharedError::ValidationFailed(format!(
                "Attempting to remove proposal for circuit {} that does not exist",
                &circuit_id,
            )));
        }

        Ok(())
    }

    fn validate_circuit_management_payload(
        &self,
        payload: &CircuitManagementPayload,
        header: &CircuitManagementPayload_Header,
    ) -> Result<(), AdminSharedError> {
        // Validate payload signature
        if payload.get_signature().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "CircuitManagementPayload signature must be set".to_string(),
            ));
        };

        // Validate the payload header
        if payload.get_header().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "CircuitManagementPayload header must be set".to_string(),
            ));
        };

        // Validate the header, requester field is set
        if header.get_requester().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "CircuitManagementPayload must have a requester".to_string(),
            ));
        };

        self.validate_key(header.get_requester())?;

        // Validate the header, requester_node_id is set
        if header.get_requester_node_id().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "CircuitManagementPayload must have a requester node id".to_string(),
            ));
        };

        Ok(())
    }

    fn check_approved(&self, proposal: &CircuitProposal) -> CircuitProposalStatus {
        let mut received_votes = HashSet::new();
        for vote in proposal.get_votes() {
            if vote.get_vote() == CircuitProposalVote_Vote::REJECT {
                return CircuitProposalStatus::Rejected;
            }
            received_votes.insert(vote.get_voter_node_id().to_string());
        }

        let mut required_votes = proposal
            .get_circuit_proposal()
            .get_members()
            .to_vec()
            .iter()
            .map(|member| member.get_node_id().to_string())
            .collect::<HashSet<String>>();

        required_votes.remove(proposal.get_requester_node_id());

        if required_votes == received_votes {
            CircuitProposalStatus::Accepted
        } else {
            CircuitProposalStatus::Pending
        }
    }

    /// Makes the `CircuitProposal` associated with a `CircuitDisbandRequest` based on information
    /// gathered from the currently active circuit that is specified in the disband request
    fn make_disband_request_circuit_proposal(
        &self,
        circuit_id: &str,
        requester: &[u8],
        requester_node_id: &str,
    ) -> Result<CircuitProposal, AdminSharedError> {
        let store_circuit = self
            .admin_store
            .get_circuit(circuit_id)
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to get circuit {}",
                    err
                ))
            })?
            .ok_or_else(|| {
                AdminSharedError::SplinterStateError(format!(
                    "Unable to get circuit: {}",
                    circuit_id
                ))
            })?;
        // Collecting the endpoints of the nodes apart of the circuit being disbanded
        let circuit_members = store_circuit
            .members()
            .iter()
            .map(|circuit_node| messages::SplinterNode {
                node_id: circuit_node.node_id().to_string(),
                endpoints: circuit_node.endpoints().to_vec(),
                public_key: circuit_node
                    .public_key()
                    .clone()
                    .map(|public_key| public_key.into_bytes()),
            })
            .collect::<Vec<messages::SplinterNode>>();
        let mut create_circuit_builder = messages::CreateCircuitBuilder::new()
            .with_circuit_id(circuit_id)
            .with_roster(
                store_circuit
                    .roster()
                    .iter()
                    .map(|service| messages::SplinterService {
                        service_id: service.service_id().into(),
                        service_type: service.service_type().into(),
                        allowed_nodes: vec![service.node_id().to_string()],
                        arguments: service
                            .arguments()
                            .iter()
                            .map(|(key, value)| (key.to_string(), value.to_string()))
                            .collect(),
                    })
                    .collect::<Vec<messages::SplinterService>>()
                    .as_ref(),
            )
            .with_members(circuit_members.as_ref())
            .with_authorization_type(&messages::AuthorizationType::from(
                store_circuit.authorization_type(),
            ))
            .with_persistence(&messages::PersistenceType::from(
                store_circuit.persistence(),
            ))
            .with_durability(&messages::DurabilityType::from(store_circuit.durability()))
            .with_routes(&messages::RouteType::from(store_circuit.routes()))
            .with_circuit_management_type(store_circuit.circuit_management_type())
            .with_circuit_version(store_circuit.circuit_version())
            .with_circuit_status(&messages::CircuitStatus::Disbanded);

        if let Some(display_name) = store_circuit.display_name() {
            create_circuit_builder = create_circuit_builder.with_display_name(display_name);
        }

        let proposed_circuit: Circuit = create_circuit_builder
            .build()
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to build circuit {}",
                    err
                ))
            })?
            .try_into()
            .map_err(|err| {
                AdminSharedError::ValidationFailed(format!(
                    "error occurred when trying to create proto circuit {}",
                    err
                ))
            })?;

        let mut circuit_proposal = CircuitProposal::new();
        circuit_proposal.set_proposal_type(CircuitProposal_ProposalType::DISBAND);
        circuit_proposal.set_circuit_id(circuit_id.to_string());
        circuit_proposal.set_circuit_hash(sha256(&proposed_circuit)?);
        circuit_proposal.set_circuit_proposal(proposed_circuit);
        circuit_proposal.set_requester(requester.to_vec());
        circuit_proposal.set_requester_node_id(requester_node_id.to_string());

        Ok(circuit_proposal)
    }

    /// Makes a `Circuit` and `StoreCircuit` with an `Abandoned` `circuit_status` to be used to
    /// update circuit state to reflect the abandoning change
    fn make_abandoned_circuit(
        &self,
        store_circuit: &StoreCircuit,
    ) -> Result<(Circuit, StoreCircuit), AdminSharedError> {
        // Collecting the endpoints of the nodes apart of the circuit being abandoned
        let circuit_members = store_circuit
            .members()
            .iter()
            .map(|circuit_node| {
                let mut node = SplinterNode::new();
                node.set_node_id(circuit_node.node_id().to_string());
                node.set_endpoints(RepeatedField::from_vec(circuit_node.endpoints().to_vec()));
                {
                    if let Some(public_key) = circuit_node.public_key() {
                        node.set_public_key(public_key.clone().into_bytes());
                    }
                }
                node
            })
            .collect::<Vec<SplinterNode>>();

        let services: Vec<SplinterService> = store_circuit
            .roster()
            .iter()
            .map(|store_service| {
                let mut service = SplinterService::new();
                service.set_service_id(store_service.service_id().to_string());
                service.set_service_type(store_service.service_type().to_string());
                service.set_allowed_nodes(RepeatedField::from_vec(vec![store_service
                    .node_id()
                    .to_string()]));
                service
            })
            .collect::<Vec<SplinterService>>();
        let mut circuit = Circuit::new();
        circuit.set_circuit_id(store_circuit.circuit_id().to_string());
        circuit.set_roster(RepeatedField::from_vec(services));
        circuit.set_members(RepeatedField::from_vec(circuit_members));
        circuit.set_authorization_type(Circuit_AuthorizationType::from(
            store_circuit.authorization_type(),
        ));
        circuit.set_persistence(Circuit_PersistenceType::from(store_circuit.persistence()));
        circuit.set_durability(Circuit_DurabilityType::from(store_circuit.durability()));
        circuit.set_routes(Circuit_RouteType::from(store_circuit.routes()));
        circuit.set_circuit_management_type(store_circuit.circuit_management_type().to_string());
        if let Some(display) = store_circuit.display_name() {
            circuit.set_display_name(display.to_string());
        }
        circuit.set_circuit_version(store_circuit.circuit_version());
        circuit.set_circuit_status(Circuit_CircuitStatus::from(store_circuit.circuit_status()));

        // Creating the `Abandoned` StoreCircuit
        let mut store_circuit = StoreCircuitBuilder::new()
            .with_circuit_id(store_circuit.circuit_id())
            .with_roster(store_circuit.roster())
            .with_members(store_circuit.members())
            .with_authorization_type(store_circuit.authorization_type())
            .with_persistence(store_circuit.persistence())
            .with_durability(store_circuit.durability())
            .with_routes(store_circuit.routes())
            .with_circuit_management_type(store_circuit.circuit_management_type())
            .with_circuit_version(store_circuit.circuit_version())
            .with_circuit_status(&StoreCircuitStatus::Abandoned);
        if let Some(display_name) = store_circuit.display_name() {
            store_circuit = store_circuit.with_display_name(&display_name);
        }

        Ok((
            circuit,
            store_circuit.build().map_err(|err| {
                AdminSharedError::SplinterStateError(format!(
                    "error occurred when trying to build circuit {}",
                    err
                ))
            })?,
        ))
    }

    /// Initialize all services that this node should run on the created circuit using the service
    /// orchestrator. This may not include all services if they are not supported locally. It is
    /// expected that some services will be started externally.
    pub fn initialize_services(&mut self, circuit: &Circuit) -> Result<(), AdminSharedError> {
        let orchestrator = self.orchestrator.lock().map_err(|_| {
            AdminSharedError::ServiceInitializationFailed {
                context: "ServiceOrchestrator lock poisoned".into(),
                source: None,
            }
        })?;

        // Get all services this node is allowed to run
        let services = circuit
            .get_roster()
            .iter()
            .filter(|service| {
                service.allowed_nodes.contains(&self.node_id)
                    && orchestrator
                        .supported_service_types()
                        .contains(&service.get_service_type().to_string())
            })
            .collect::<Vec<_>>();

        // Start all services the orchestrator has a factory for
        for service in services {
            let service_definition = ServiceDefinition {
                circuit: circuit.circuit_id.clone(),
                service_id: service.service_id.clone(),
                service_type: service.service_type.clone(),
            };

            let service_arguments = service
                .arguments
                .iter()
                .map(|arg| (arg.key.clone(), arg.value.clone()))
                .collect();

            orchestrator
                .initialize_service(service_definition.clone(), service_arguments)
                .map_err(|err| AdminSharedError::ServiceInitializationFailed {
                    context: format!(
                        "Unable to start service {} on circuit {}",
                        service.service_id, circuit.circuit_id
                    ),
                    source: Some(err),
                })?;
        }

        Ok(())
    }

    /// Stops all services that this node was running on the disbanded or abandoned circuit using
    /// the service orchestrator. This may not include all services if they are not supported
    /// locally. It is expected that some services will be stopped externally.
    pub fn stop_services(&mut self, circuit: &Circuit) -> Result<(), AdminSharedError> {
        let orchestrator =
            self.orchestrator
                .lock()
                .map_err(|_| AdminSharedError::ServiceShutdownFailed {
                    context: "ServiceOrchestrator lock poisoned".into(),
                    source: None,
                })?;

        // Get all services this node is allowed to run
        let services = circuit
            .get_roster()
            .iter()
            .filter(|service| {
                service.allowed_nodes.contains(&self.node_id)
                    && orchestrator
                        .supported_service_types()
                        .contains(&service.get_service_type().to_string())
            })
            .collect::<Vec<_>>();

        // Shutdown all services the orchestrator has a factory for
        for service in services {
            debug!("Stopping service: {}", service.service_id.clone());
            let service_definition = ServiceDefinition {
                circuit: circuit.circuit_id.clone(),
                service_id: service.service_id.clone(),
                service_type: service.service_type.clone(),
            };

            orchestrator
                .stop_service(&service_definition)
                .map_err(|err| AdminSharedError::ServiceShutdownFailed {
                    context: format!(
                        "Unable to shutdown service {} on circuit {}",
                        service.service_id, circuit.circuit_id
                    ),
                    source: Some(err),
                })?;
        }

        Ok(())
    }

    /// Purges all services that this node was running on the disbanded circuit using the service
    /// orchestrator. Destroying a service will also remove the service's state LMDB files.
    pub fn purge_services(
        &mut self,
        circuit_id: &str,
        services: &[StoreService],
    ) -> Result<(), AdminSharedError> {
        let orchestrator =
            self.orchestrator
                .lock()
                .map_err(|_| AdminSharedError::ServiceShutdownFailed {
                    context: "ServiceOrchestrator lock poisoned".into(),
                    source: None,
                })?;

        // Get all services this node is allowed to run
        let purge_results = services
            .iter()
            .filter_map(|service| {
                if service.node_id() == self.node_id
                    && orchestrator
                        .supported_service_types()
                        .contains(&service.service_type().to_string())
                {
                    return Some(ServiceDefinition {
                        circuit: circuit_id.to_string(),
                        service_id: service.service_id().to_string(),
                        service_type: service.service_type().to_string(),
                    });
                }
                None
            })
            .map(|service| {
                debug!(
                    "Purging service: {}::{} ({})",
                    &service.circuit, &service.service_id, &service.service_type,
                );

                let res = orchestrator.purge_service(&service);
                (service, res)
            })
            .filter(|(_, res)| res.is_err())
            .collect::<Vec<_>>();

        for (service_def, res) in purge_results {
            if let Err(err) = res {
                error!(
                    "Service {}::{} ({}) failed to purge: {}",
                    service_def.circuit, service_def.service_id, service_def.service_type, err
                );
            }
        }

        Ok(())
    }

    /// Verify all members are ready before cleaning up after the disbanded circuit, i.e. removing
    /// peer refs, removing the circuit from the routing table, and shutting down the circuit's
    /// associated services.
    pub fn cleanup_disbanded_circuit_if_members_ready(
        &mut self,
        circuit_id: &str,
    ) -> Result<(), AdminSharedError> {
        let ready = {
            if let Some(disbanded_circuit) = self.uninitialized_circuits.get(circuit_id) {
                if let Some(ref circuit_proposal) = disbanded_circuit.circuit {
                    let all_members = circuit_proposal
                        .get_circuit_proposal()
                        .members
                        .iter()
                        .map(|node| node.node_id.clone())
                        .collect::<HashSet<String>>();
                    all_members.is_subset(&disbanded_circuit.ready_members)
                } else {
                    false
                }
            } else {
                false
            }
        };

        if ready {
            let mut circuit_proposal = self
                .uninitialized_circuits
                .remove(circuit_id)
                .expect("Pending disband circuit not set")
                .circuit
                .expect("Pending disband circuit's circuit proposal not set");
            // send message about circuit acceptance
            let circuit_proposal_proto =
                messages::CircuitProposal::from_proto(circuit_proposal.clone())
                    .map_err(AdminSharedError::InvalidMessageFormat)?;
            let mgmt_type = circuit_proposal
                .get_circuit_proposal()
                .circuit_management_type
                .clone();
            let event = messages::AdminServiceEvent::CircuitDisbanded(circuit_proposal_proto);
            self.send_event(&mgmt_type, event);

            // Circuit has been disbanded: all associated services will be shut
            // down, the circuit removed from the routing table, and peer refs
            // for this circuit will be removed.
            self.stop_services(circuit_proposal.get_circuit_proposal())?;
            // Removing the circuit from the routing table
            self.routing_table_writer
                .remove_circuit(circuit_proposal.get_circuit_id())
                .map_err(|_| {
                    AdminSharedError::SplinterStateError(format!(
                        "Unable to remove circuit from routing table: {}",
                        circuit_id
                    ))
                })?;

            let proposed_circuit = ProposedCircuit::from_proto(
                circuit_proposal.take_circuit_proposal(),
            )
            .map_err(|err| {
                AdminSharedError::SplinterStateError(format!(
                    "Unable to get store proposed circuit from proto: {}",
                    err
                ))
            })?;
            // Removing the circuit's peer refs
            self.remove_peer_refs(proposed_circuit.list_tokens(&self.node_id).map_err(|err| {
                AdminSharedError::SplinterStateError(format!(
                    "Unable to remove peer refs for proposal {}: {}",
                    proposed_circuit.circuit_id(),
                    err
                ))
            })?);
        }

        Ok(())
    }

    /// Collect all circuits from the admin store, including `Disbanded` or `Abandoned` circuits
    pub fn get_circuits(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = StoreCircuit>>, AdminSharedError> {
        let predicates = vec![
            CircuitPredicate::CircuitStatus(StoreCircuitStatus::Active),
            CircuitPredicate::CircuitStatus(StoreCircuitStatus::Disbanded),
            CircuitPredicate::CircuitStatus(StoreCircuitStatus::Abandoned),
        ];
        self.admin_store
            .list_circuits(&predicates)
            .map_err(AdminSharedError::from)
    }

    fn verify_signature(&self, payload: &CircuitManagementPayload) -> Result<bool, ServiceError> {
        let header: CircuitManagementPayload_Header =
            Message::parse_from_bytes(payload.get_header())?;

        let signature = payload.get_signature().to_vec();
        let public_key = header.get_requester().to_vec();

        self.signature_verifier
            .verify(
                payload.get_header(),
                &Signature::new(signature),
                &PublicKey::new(public_key),
            )
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
    }
}

// This should never return an error since we recieved a message from this service id
fn get_peer_token_from_service_id(
    service_id: &str,
    local_node_id: &str,
) -> Result<PeerTokenPair, InternalError> {
    let mut iter = service_id.split("::");

    let admin_prefix = iter
        .next()
        .expect("str::split cannot return an empty iterator")
        .to_string();

    if admin_prefix.is_empty() {
        return Err(InternalError::with_message(
            "Empty admin_id argument detected".into(),
        ));
    }

    let node_id = iter
        .next()
        .ok_or_else(|| InternalError::with_message("Missing node id for recipient".into()))?;

    if node_id.is_empty() {
        return Err(InternalError::with_message("Empty node id provided".into()));
    }

    // If challenge authorization the admin id will be in the format
    // admin::public_key::<public key string>.
    if node_id == ADMIN_SERVICE_PUBLIC_KEY_PREFIX {
        let public_key = iter
            .next()
            .ok_or_else(|| InternalError::with_message("Missing public key for recipient".into()))?
            .to_string();

        if public_key.is_empty() {
            return Err(InternalError::with_message(
                "Empty public key provided".into(),
            ));
        }

        let second_public_key = iter.next().ok_or_else(|| {
            InternalError::with_message("Missing local public key for recipient".into())
        })?;

        if second_public_key != ADMIN_SERVICE_PUBLIC_KEY_PREFIX {
            return Err(InternalError::with_message(
                "Missing local public key for recipient".into(),
            ));
        }

        let local_public_key = iter
            .next()
            .ok_or_else(|| {
                InternalError::with_message("Missing local_public key for recipient".into())
            })?
            .to_string();

        if local_public_key.is_empty() {
            return Err(InternalError::with_message(
                "Empty local public key provided".into(),
            ));
        }

        Ok(PeerTokenPair::new(
            PeerAuthorizationToken::from_public_key(
                &parse_hex(&public_key)
                    .map_err(|err| InternalError::with_message(err.to_string()))?,
            ),
            PeerAuthorizationToken::from_public_key(
                &parse_hex(&local_public_key)
                    .map_err(|err| InternalError::with_message(err.to_string()))?,
            ),
        ))
    } else {
        Ok(PeerTokenPair::new(
            PeerAuthorizationToken::from_peer_id(node_id),
            PeerAuthorizationToken::from_peer_id(local_node_id),
        ))
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use protobuf::RepeatedField;

    use diesel::{
        r2d2::{ConnectionManager as DieselConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    use crate::admin::service::AdminKeyVerifierError;
    use crate::admin::store;
    use crate::admin::store::diesel::DieselAdminServiceStore;
    use crate::admin::store::CircuitNode;
    use crate::circuit::routing::memory::RoutingTable;
    use crate::keys::insecure::AllowAllKeyPermissionManager;
    use crate::mesh::{Envelope, Mesh};
    use crate::migrations::run_sqlite_migrations;
    use crate::network::auth::AuthorizationManager;
    use crate::network::connection_manager::authorizers::{Authorizers, InprocAuthorizer};
    use crate::network::connection_manager::ConnectionManager;
    use crate::orchestrator::ServiceOrchestratorBuilder;
    use crate::peer::{PeerManager, PeerManagerConnector};
    use crate::protocol::authorization::{
        AuthorizationMessage, AuthorizationType, Authorized, ConnectRequest, ConnectResponse,
        TrustRequest,
    };
    use crate::protocol::network::NetworkMessage;
    use crate::protos::admin;
    use crate::protos::admin::{
        CircuitProposalVote_Vote, CircuitProposal_VoteRecord, SplinterNode, SplinterService,
    };
    use crate::protos::network;
    use crate::protos::prelude::*;
    use crate::service::{ServiceMessageContext, ServiceSendError};
    use crate::threading::lifecycle::ShutdownHandle;
    use crate::transport::{
        inproc::InprocTransport, ConnectError, Connection, DisconnectError, RecvError, SendError,
        Transport,
    };

    const PUB_KEY: &[u8] = &[
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32,
    ];

    /// Test that the CircuitManagementPayload is moved to the pending payloads when the peers are
    /// fully authorized.
    #[test]
    fn test_protocol_agreement() {
        let mut transport = InprocTransport::default();
        let mut orchestrator_transport = transport.clone();

        let mut other_listener = transport
            .listen("inproc://otherplace:8000")
            .expect("Unable to get listener");
        let _test_listener = transport
            .listen("inproc://someplace:8000")
            .expect("Unable to get listener");
        let _orchestator_listener = transport
            .listen("inproc://orchestator")
            .expect("Unable to get listener");

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(Some(transport));
        let orchestrator_connection = orchestrator_transport
            .connect("inproc://orchestator")
            .expect("failed to create connection");
        let orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .expect("failed to create orchestrator")
            .run()
            .expect("failed to start orchestrator");
        let store = setup_admin_service_store();

        let event_store = store.clone_boxed();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut shared = AdminServiceShared::new(
            "my_peer_id".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let service_sender = MockServiceNetworkSender::new();
        shared.set_network_sender(Some(Box::new(service_sender.clone())));

        let mut circuit = admin::Circuit::new();
        circuit.set_circuit_id("01234-ABCDE".into());
        circuit.set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE);
        circuit.set_durability(admin::Circuit_DurabilityType::NO_DURABILITY);
        circuit.set_circuit_management_type("test app auth handler".into());
        circuit.set_comments("test circuit".into());
        circuit.set_display_name("test_display".into());
        circuit.set_circuit_status(admin::Circuit_CircuitStatus::ACTIVE);

        circuit.set_members(protobuf::RepeatedField::from_vec(vec![
            splinter_node("test-node", &["inproc://someplace:8000".to_string()]),
            splinter_node("other-node", &["inproc://otherplace:8000".to_string()]),
            splinter_node("my_peer_id", &["inproc://myplace:8000".to_string()]),
        ]));
        circuit.set_roster(protobuf::RepeatedField::from_vec(vec![
            splinter_service("0123", "sabre"),
            splinter_service("ABCD", "sabre"),
        ]));

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);

        let mut payload = admin::CircuitManagementPayload::new();

        payload.set_signature(Vec::new());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_circuit_create_request(request);

        // start up thread for other node
        std::thread::spawn(move || {
            let mut mesh = Mesh::new(2, 2);
            let conn = other_listener.accept().unwrap();
            mesh.add(conn, "my_peer_id".to_string()).unwrap();

            handle_auth(&mesh, "my_peer_id", "other-node");

            mesh.signal_shutdown();
            mesh.wait_for_shutdown().expect("Unable to shutdown mesh");
        });

        shared
            .propose_circuit(payload, "test".to_string())
            .expect("Proposal not accepted");

        // None of the proposed members are peered
        assert_eq!(1, shared.unpeered_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set other-node to peered
        shared
            .on_peer_connected(&PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("other-node"),
                PeerAuthorizationToken::from_peer_id("my_peer_id"),
            ))
            .expect("Unable to set peer to peered");

        // Still waitin on 1 peer
        assert_eq!(1, shared.unpeered_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set other-node to peered
        shared
            .on_peer_connected(&PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("test-node"),
                PeerAuthorizationToken::from_peer_id("my_peer_id"),
            ))
            .expect("Unable to set peer to peered");

        // We're fully peered, but need to wait for protocol to be agreed on
        assert_eq!(1, shared.pending_protocol_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        shared
            .on_protocol_agreement("admin::other-node", 1)
            .expect("received unexpected error");

        // Waiting on 1 node for protocol agreement
        assert_eq!(1, shared.pending_protocol_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        shared
            .on_protocol_agreement("admin::test-node", 1)
            .expect("received unexpected error");
        // We're fully peered and agreed on protocol, so the pending payload is now available
        assert_eq!(0, shared.pending_protocol_payloads.len());
        assert_eq!(1, shared.pending_circuit_payloads.len());
        shutdown(mesh, cm, pm);
    }

    /// Test that the CircuitManagementPayload message is dropped, if a node fails to match
    /// protocol versions
    #[test]
    fn test_protocol_disagreement() {
        let mut transport = InprocTransport::default();
        let mut orchestrator_transport = transport.clone();

        let _listener = transport
            .listen("inproc://otherplace:8000")
            .expect("Unable to get listener");
        let _admin_listener = transport
            .listen("inproc://admin-service")
            .expect("Unable to get listener");

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(Some(transport));
        let orchestrator_connection = orchestrator_transport
            .connect("inproc://admin-service")
            .expect("failed to create connection");
        let orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .expect("failed to create orchestrator")
            .run()
            .expect("failed to start orchestrator");
        let store = setup_admin_service_store();

        let event_store = store.clone_boxed();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut shared = AdminServiceShared::new(
            "test-node".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let service_sender = MockServiceNetworkSender::new();
        shared.set_network_sender(Some(Box::new(service_sender.clone())));

        let mut circuit = admin::Circuit::new();
        circuit.set_circuit_id("01234-ABCDE".into());
        circuit.set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE);
        circuit.set_circuit_management_type("test app auth handler".into());
        circuit.set_comments("test circuit".into());
        circuit.set_display_name("test_display".into());
        circuit.set_circuit_status(admin::Circuit_CircuitStatus::ACTIVE);
        circuit.set_durability(admin::Circuit_DurabilityType::NO_DURABILITY);

        circuit.set_members(protobuf::RepeatedField::from_vec(vec![
            splinter_node("test-node", &["inproc://someplace:8000".to_string()]),
            splinter_node("other-node", &["inproc://otherplace:8000".to_string()]),
        ]));
        circuit.set_roster(protobuf::RepeatedField::from_vec(vec![
            splinter_service("0123", "sabre"),
            splinter_service("ABCD", "sabre"),
        ]));

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);

        let mut payload = admin::CircuitManagementPayload::new();

        payload.set_signature(Vec::new());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_circuit_create_request(request);

        shared
            .propose_circuit(payload, "local".to_string())
            .expect("Proposal not accepted");

        // None of the proposed members are peered
        assert_eq!(1, shared.unpeered_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set other-node to peered
        shared
            .on_peer_connected(&PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("other-node"),
                PeerAuthorizationToken::from_peer_id("test-node"),
            ))
            .expect("Unable to set peer to peered");

        assert_eq!(0, shared.unpeered_payloads.len());
        assert_eq!(1, shared.pending_protocol_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());
        shared
            .on_protocol_agreement("admin::other-node", 0)
            .expect("received unexpected error");

        // The message should be dropped
        assert_eq!(0, shared.pending_circuit_payloads.len());
        assert_eq!(0, shared.pending_protocol_payloads.len());

        // sent the service protocol request but not the payload
        assert_eq!(
            1,
            service_sender
                .sent
                .lock()
                .expect("Network sender lock poisoned")
                .len()
        );
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that a valid circuit is validated correctly
    fn test_validate_circuit_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();

        if let Err(err) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    #[test]
    // Test that a valid circuit on version 2, would fail on protocol 1 because display_name is
    // set. Protocol 1 should fail any circuit that has display name set, as display name is not
    // included in the protobuf.
    fn test_validate_invalid_protocol_display_name() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();

        if let Ok(()) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a", 1) {
            panic!("Should have been invalid because display name is set");
        }

        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that a circuit proposed with a key that is not permitted for the requesting node is
    // invalid
    fn test_validate_circuit_signer_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to requester node not being registered");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit is proposed by a signer key is not a valid public key the proposal is
    // invalid
    fn test_validate_circuit_signer_key_invalid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();

        let pub_key = (0u8..50).collect::<Vec<_>>();
        // too short
        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            &pub_key[0..10],
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to key being too short");
        }
        // too long
        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            &pub_key,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to key being too long");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service in its roster with an allowed node that is not in
    // members an error is returned
    fn test_validate_circuit_bad_node() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut service_bad = SplinterService::new();
        service_bad.set_service_id("0123".to_string());
        service_bad.set_service_type("type_a".to_string());
        service_bad.set_allowed_nodes(RepeatedField::from_vec(vec!["node_bad".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_bad]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to service having an allowed node not in members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service in its roster with too many allowed nodes
    fn test_validate_circuit_too_many_allowed_nodes() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut service_bad = SplinterService::new();
        service_bad.set_service_id("0123".to_string());
        service_bad.set_service_type("type_a".to_string());
        service_bad.set_allowed_nodes(RepeatedField::from_vec(vec![
            "node_b".to_string(),
            "extra".to_string(),
        ]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_bad]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to service having too many allowed nodes");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with "" for a service id an error is returned
    fn test_validate_circuit_empty_service_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut service_ = SplinterService::new();
        service_.set_service_id("".to_string());
        service_.set_service_type("type_a".to_string());
        service_.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to service's id being empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with an invalid service id an error is returned
    fn test_validate_circuit_invalid_service_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut service_ = SplinterService::new();
        service_.set_service_id("invalid_service_id".to_string());
        service_.set_service_type("type_a".to_string());
        service_.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to service's id being empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with duplicate service ids an error is returned
    fn test_validate_circuit_duplicate_service_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut service_a = SplinterService::new();
        service_a.set_service_id("0123".to_string());
        service_a.set_service_type("type_a".to_string());
        service_a.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        let mut service_a2 = SplinterService::new();
        service_a2.set_service_id("0123".to_string());
        service_a2.set_service_type("type_a".to_string());
        service_a2.set_allowed_nodes(RepeatedField::from_vec(vec!["node_b".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_a, service_a2]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to service's id being a duplicate");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have any services in its roster an error is returned
    fn test_validate_circuit_empty_roster() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();
        circuit.set_roster(RepeatedField::from_vec(vec![]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to empty roster");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have any nodes in its members an error is returned
    fn test_validate_circuit_empty_members() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_members(RepeatedField::from_vec(vec![]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid empty members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have the local node in the member list an error is
    // returned
    fn test_validate_circuit_missing_local_node() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because node_a is not in members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with node id of "" an error is
    // returned
    fn test_validate_circuit_empty_node_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        let mut node_ = SplinterNode::new();
        node_.set_node_id("".to_string());
        node_.set_endpoints(vec!["test://endpoint_:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b, node_]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because node_ is has an empty node id");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has duplicate members an error is returned
    fn test_validate_circuit_duplicate_members() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        let mut node_b2 = SplinterNode::new();
        node_b2.set_node_id("node_b".to_string());
        node_b2.set_endpoints(vec!["test://endpoint_b2:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b, node_b2]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because there are duplicate members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has an empty circuit id an error is returned
    fn test_validate_circuit_empty_circuit_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_id("".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because the circuit ID is empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has an invalid circuit id an error is returned
    fn test_validate_circuit_invalid_circuit_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_id("invalid_circuit_id".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because the circuit ID is invalid");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with no endpoints an error is returned
    fn test_validate_circuit_no_endpoints() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec![].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because a member has no endpoints");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with an empty endpoint an error is returned
    fn test_validate_circuit_empty_endpoint() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because a member has an empty endpoint");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with a duplicate endpoint an error is returned
    fn test_validate_circuit_duplicate_endpoint() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because a member has a duplicate endpoint");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have authorization set an error is returned
    fn test_validate_circuit_no_authorization() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_authorization_type(Circuit_AuthorizationType::UNSET_AUTHORIZATION_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because authorization type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have persistence set an error is returned
    fn test_validate_circuit_no_persitance() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_persistence(Circuit_PersistenceType::UNSET_PERSISTENCE_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because persistence type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have durability set an error is returned
    fn test_validate_circuit_unset_durability() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_durability(Circuit_DurabilityType::UNSET_DURABILITY_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because durabilty type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have route type set an error is returned
    fn test_validate_circuit_no_routes() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_routes(Circuit_RouteType::UNSET_ROUTE_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because route type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have circuit_management_type set an error is returned
    fn test_validate_circuit_no_management_type() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_management_type("".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because route type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has challenge auth set while circuit version 1 an error is returned
    fn test_validate_circuit_challenge_auth_not_supported() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_v1_test_circuit();

        circuit.set_authorization_type(Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION);

        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because cannot have challenge auth if version 1");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has challenge auth set and nodes do not have public keys, the circuit
    // is invalid
    fn test_validate_circuit_challenge_auth_no_public_keys() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let mut circuit = setup_test_circuit();

        circuit.set_authorization_type(Circuit_AuthorizationType::CHALLENGE_AUTHORIZATION);
        if let Ok(_) = admin_shared.validate_create_circuit(
            &circuit,
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because nodes do not have public keys set");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that a valid circuit proposal vote comes back as valid
    fn test_validate_proposal_vote_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();
        let vote = setup_test_vote(&circuit);
        let proposal = setup_test_proposal(&circuit);

        if let Err(err) = admin_shared.validate_circuit_vote(
            &vote,
            PUB_KEY,
            &StoreProposal::from_proto(proposal).expect("Unable to get proposal"),
            "node_a",
        ) {
            panic!("Should have been valid: {}", err);
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if the vote is from a key that is not permitted for the voting node the vote is
    // invalid
    fn test_validate_proposal_vote_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();
        let vote = setup_test_vote(&circuit);
        let proposal = setup_test_proposal(&circuit);

        if let Ok(_) = admin_shared.validate_circuit_vote(
            &vote,
            PUB_KEY,
            &StoreProposal::from_proto(proposal).expect("Unable to get proposal"),
            "node_a",
        ) {
            panic!("Should have been invalid because voting node is not registered");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test if the voter is the original requester node the vote is invalid
    fn test_validate_proposal_vote_requester() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();
        let vote = setup_test_vote(&circuit);
        let proposal = setup_test_proposal(&circuit);

        if let Ok(_) = admin_shared.validate_circuit_vote(
            &vote,
            PUB_KEY,
            &StoreProposal::from_proto(proposal).expect("Unable to get proposal"),
            "node_b",
        ) {
            panic!("Should have been invalid because voter is the requester");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test if a voter has already voted on a proposal the new vote is invalid
    fn test_validate_proposal_vote_duplicate_vote() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();
        let vote = setup_test_vote(&circuit);
        let mut proposal = setup_test_proposal(&circuit);

        let mut vote_record = CircuitProposal_VoteRecord::new();
        vote_record.set_vote(CircuitProposalVote_Vote::ACCEPT);
        vote_record.set_public_key(b"test_signer_a".to_vec());
        vote_record.set_voter_node_id("node_a".to_string());

        proposal.set_votes(RepeatedField::from_vec(vec![vote_record]));

        if let Ok(_) = admin_shared.validate_circuit_vote(
            &vote,
            PUB_KEY,
            &StoreProposal::from_proto(proposal).expect("Unable to get proposal"),
            "node_a",
        ) {
            panic!("Should have been invalid because node as already submitted a vote");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if the circuit hash in the circuit proposal does not match the circuit hash on
    // the vote, the vote is invalid
    fn test_validate_proposal_vote_circuit_hash_mismatch() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let circuit = setup_test_circuit();
        let vote = setup_test_vote(&circuit);
        let mut proposal = setup_test_proposal(&circuit);

        proposal.set_circuit_hash("bad_hash".to_string());

        if let Ok(_) = admin_shared.validate_circuit_vote(
            &vote,
            PUB_KEY,
            &StoreProposal::from_proto(proposal).expect("Unable to get proposal"),
            "node_a",
        ) {
            panic!("Should have been invalid because the circuit hash does not match");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that the validate_circuit_management_payload method returns an error in case the
    // signature is empty.
    fn test_validate_circuit_management_payload_signature() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let circuit = setup_test_circuit();

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        header.set_requester(PUB_KEY.into());
        header.set_requester_node_id("node_b".to_string());
        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_signature(Vec::new());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_circuit_create_request(request);

        // Asserting the payload will be deemed invalid as the signature is an empty vec.
        if let Ok(_) = shared.validate_circuit_management_payload(&payload, &header) {
            panic!("Should have been invalid due to empty signature");
        }

        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        // Asserting the payload passed through validation.
        if let Err(_) = shared.validate_circuit_management_payload(&payload, &header) {
            panic!("Should have been valid");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that the validate_circuit_management_payload method returns an error in case the
    // header is empty.
    fn test_validate_circuit_management_payload_header() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let circuit = setup_test_circuit();

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        header.set_requester(PUB_KEY.into());
        header.set_requester_node_id("node_b".to_string());
        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_circuit_create_request(request);

        // Asserting the payload will be deemed invalid as the header is empty.
        match shared.validate_circuit_management_payload(&payload, &header) {
            Err(err) => assert!(err
                .to_string()
                .contains("CircuitManagementPayload header must be set")),
            _ => panic!("Should have been invalid because empty requester field"),
        }
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        // Asserting the payload passed through validation, and failed at a further step.
        if let Err(_) = shared.validate_circuit_management_payload(&payload, &header) {
            panic!("Should have been valid");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that the validate_circuit_management_payload method returns an error in case the header
    // requester field is empty.
    fn test_validate_circuit_management_header_requester() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let circuit = setup_test_circuit();

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        header.set_requester_node_id("node_b".to_string());
        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_circuit_create_request(request);

        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        // Asserting the payload will be deemed invalid as the header is empty.
        match shared.validate_circuit_management_payload(&payload, &header) {
            Err(err) => assert!(err
                .to_string()
                .contains("CircuitManagementPayload must have a requester")),
            _ => panic!("Should have been invalid because empty requester field"),
        }

        header.set_requester(PUB_KEY.into());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        // Asserting the payload passed through validation, and failed at a further step.
        if let Err(_) = shared.validate_circuit_management_payload(&payload, &header) {
            panic!("Should have been valid");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that the CircuitManagementPayload returns an error in case the header requester_node_id
    // field is empty.
    fn test_validate_circuit_management_header_requester_node_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let circuit = setup_test_circuit();

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(circuit);

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        header.set_requester(PUB_KEY.into());
        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_circuit_create_request(request);

        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        // Asserting the payload will be deemed invalid as the header is empty.
        match shared.validate_circuit_management_payload(&payload, &header) {
            Err(err) => assert!(err
                .to_string()
                .contains("CircuitManagementPayload must have a requester node id")),
            _ => panic!("Should have been invalid because empty requester field"),
        }

        header.set_requester_node_id("node_b".to_string());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        // Asserting the payload passed through validation, and failed at a further step.
        if let Err(_) = shared.validate_circuit_management_payload(&payload, &header) {
            panic!("Should have been valid");
        }
        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being disbanded is validated correctly
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the circuit to be disbanded to the admin store. This is required by the
    ///    `validate_disband_circuit` which verifies the circuit exists.
    /// 3. Validate the call to `validate_disband_circuit` returns successfully
    ///
    /// This test verifies the `validate_disband_circuit` returns successfully when given a
    /// valid request to disband an existing circuit.
    #[test]
    fn test_validate_disband_circuit_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be disbanded
        shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Err(err) = shared.validate_disband_circuit(
            &setup_test_circuit(),
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit is unable to be disbanded when an invalid admin service protocol
    /// version is used. Currently, the disband functionality is not available for
    /// admin service protocol 1.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the circuit to be disbanded to the admin store.
    /// 3. Call `validate_disband_circuit` with a valid Circuit, valid requester info and protocol
    ///    version 1.
    /// 4. Validate the call to `validate_disband_circuit` returns an error
    ///
    /// This test verifies the `validate_disband_circuit` returns an error when given
    /// an admin service protocol that is not above 1.
    #[test]
    fn test_validate_disband_circuit_invalid_protocol() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be disbanded
        shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) =
            shared.validate_disband_circuit(&setup_v1_test_circuit(), PUB_KEY, "node_a", 1)
        {
            panic!(
                "Should have been invalid because the admin service protocol schema version is 1"
            );
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being disbanded is invalid if the circuit to be disbanded has an
    /// invalid circuit version.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a v1 circuit to the admin store. This is required by the `validate_disband_circuit`,
    ///    which verifies the circuit exists.
    /// 3. Call `validate_disband_circuit` with a version 1 Circuit and valid requester info
    /// 4. Validate the call to `validate_disband_circuit` returns an error
    ///
    /// This test verifies the `validate_disband_circuit` returns an error when given
    /// a version 1 circuit as disbanding functionality is not supported by version 1 circuits.
    #[test]
    fn test_validate_disband_circuit_invalid_circuit_version() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the v1 circuit to be attempted to disbanded
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(1, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_disband_circuit(
            &setup_v1_test_circuit(),
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!(
                "Should have been invalid because the circuit being disbanded is schema version 1"
            );
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being disbanded is invalid if the circuit to be disbanded does not
    /// exist in the admin store, indicating an invalid disband request.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Call `validate_disband_circuit` with a valid circuit and valid requester info
    /// 3. Validate the call to `validate_disband_circuit` returns an error
    ///
    /// This test verifies the `validate_disband_circuit` returns an error when given
    /// a circuit that does not already exist in the admin store.
    #[test]
    fn test_validate_disband_circuit_no_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        if let Ok(()) = admin_shared.validate_disband_circuit(
            &setup_test_circuit(),
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because the circuit being disbanded does not exist");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being disbanded is invalid if the requester is not permitted for
    /// the node.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit to the admin store
    /// 3. Call `validate_disband_circuit` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_disband_circuit` returns an error
    ///
    /// This test verifies the `validate_disband_circuit` returns an error when given a signer
    /// key that is not permitted on the node.
    #[test]
    fn test_validate_disband_circuit_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be disbanded
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_disband_circuit(
            &setup_test_circuit(),
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid because the requester is not authorized");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being disbanded is invalid if the requester is not permitted for
    /// the node.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit, with `circuit_status` set to `Disbanded`, to the admin store
    /// 3. Call `validate_disband_circuit` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_disband_circuit` returns an error
    ///
    /// This test verifies the `validate_disband_circuit` returns an error when the circuit
    /// attempted to be disbanded already has a status of `Disbanded`. The disbanded action
    /// is only valid for currently `Active` circuits.
    #[test]
    fn test_validate_disband_circuit_already_disbanded() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_disband_circuit(
            &setup_test_circuit(),
            PUB_KEY,
            "node_a",
            ADMIN_SERVICE_PROTOCOL_VERSION,
        ) {
            panic!("Should have been invalid due to circuit already being disbanded");
        }
        shutdown(mesh, cm, pm);
    }

    /// Tests that the payload submitted via `propose_disband` is moved to the admin service's
    /// payload lists as peers become fully peered, authorized and agree on a service protocol.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit to the admin store
    /// 3. Create the `CircuitDisbandRequest` payload
    /// 4. Start another thread to handle authorization for the peer node, "node_b"
    /// 5. Set each node to peered. The `CircuitDisbandRequest` is only valid for circuits that
    ///    have already been created, so nodes are assumed to be peered before the request is
    ///    submitted.
    /// 6. Submit the `CircuitDisbandRequest` payload
    /// 7. Assert the payload is present in the "pending_protocol_payloads" list and that no
    ///    payload is available in the `pending_circuit_payloads` list
    /// 8. Set each node to agree on the protocol
    /// 9. Assert the payload is present in the "pending_circuit_payloads" list and that no
    ///    payload is available in the `pending_protocol_payloads` list
    ///
    /// This test verifies a payload submitted using `propose_disband` is moved to the admin
    /// service's payload lists as expected when the circuit nodes have already been peered
    /// and then agree on the admin service protocol.
    #[test]
    fn test_disband_request() {
        let mut transport = InprocTransport::default();
        let mut orchestrator_transport = transport.clone();

        let mut other_listener = transport
            .listen("inproc://otherplace:8000")
            .expect("Unable to get listener");
        let _test_listener = transport
            .listen("inproc://someplace:8000")
            .expect("Unable to get listener");
        let _orchestator_listener = transport
            .listen("inproc://orchestator")
            .expect("Unable to get listener");

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(Some(transport));
        let orchestrator_connection = orchestrator_transport
            .connect("inproc://orchestator")
            .expect("failed to create connection");
        let orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .expect("failed to create orchestrator")
            .run()
            .expect("failed to start orchestrator");
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let service_sender = MockServiceNetworkSender::new();
        shared.set_network_sender(Some(Box::new(service_sender.clone())));

        shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        for node in store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active)
            .list_nodes()
            .expect("Unable to get peer nodes from circuit")
        {
            shared.token_to_peer.insert(
                PeerTokenPair::new(
                    node.token.clone(),
                    PeerAuthorizationToken::from_peer_id("node_a"),
                ),
                PeerNodePair {
                    peer_node: node,
                    local_peer_token: PeerAuthorizationToken::from_peer_id("node_a"),
                },
            );
        }

        let mut request = admin::CircuitDisbandRequest::new();
        request.set_circuit_id("01234-ABCDE".to_string());

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_DISBAND_REQUEST);
        header.set_requester(b"test_signer_a".to_vec());
        header.set_requester_node_id("node_a".to_string());

        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_signature(b"test_signer_a".to_vec());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_circuit_disband_request(request);
        // start up thread for other node
        std::thread::spawn(move || {
            let mut mesh = Mesh::new(2, 2);
            let conn = other_listener.accept().unwrap();
            mesh.add(conn, "my_peer_id".to_string()).unwrap();

            handle_auth(&mesh, "my_peer_id", "node_b");

            mesh.signal_shutdown();
            mesh.wait_for_shutdown().expect("Unable to shutdown mesh");
        });

        // Set `node_b` to peered
        shared
            .on_peer_connected(&PeerTokenPair::new(
                PeerAuthorizationToken::from_peer_id("node_b"),
                PeerAuthorizationToken::from_peer_id("node_a"),
            ))
            .expect("Unable to set peer to peered");

        shared
            .propose_disband(
                payload,
                &b"test_signer_a".to_vec(),
                "node_a",
                "test".to_string(),
            )
            .expect("Proposal not accepted");

        // We're fully peered, but need to wait for protocol to be agreed on
        assert_eq!(1, shared.pending_protocol_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set `node_b` to agree on the protocol
        shared
            .on_protocol_agreement("admin::node_b", ADMIN_SERVICE_PROTOCOL_VERSION)
            .expect("received unexpected error");

        // Set `node_a` to agree on the protocol
        shared
            .on_protocol_agreement("admin::node_a", ADMIN_SERVICE_PROTOCOL_VERSION)
            .expect("received unexpected error");

        // We're fully peered and agreed on protocol, so the pending payload is now available
        assert_eq!(0, shared.pending_protocol_payloads.len());
        assert_eq!(1, shared.pending_circuit_payloads.len());
        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being purged is validated correctly
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the disbanded circuit to be purged to the admin store
    /// 3. Call `validate_purge_request` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_purge_request` returns successfully
    ///
    /// This test verifies the `validate_purge_request` returns successfully given a valid purge
    /// request.
    #[test]
    fn test_validate_purge_request_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the disbanded circuit to be purged
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Err(err) = admin_shared.validate_purge_request("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a purge request is invalid if the circuit to be purged does not exist in the
    /// admin store, indicating an invalid purge request.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Call `validate_purge_request` with a valid circuit and valid requester info
    /// 3. Validate the call to `validate_purge_request` returns an error
    ///
    /// This test verifies the `validate_purge_request` returns an error when given
    /// a circuit that does not already exist in the admin store.
    #[test]
    fn test_validate_purge_request_no_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        if let Ok(()) = admin_shared.validate_purge_request("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit being disbanded does not exist");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a purge request is invalid if the requester is not permitted for the node.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the disbanded circuit to be purged to the admin store
    /// 3. Call `validate_purge_request` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_purge_request` returns an error
    ///
    /// This test verifies the `validate_purge_request` returns an error when given a signer
    /// key that is not permitted on the node.
    #[test]
    fn test_validate_purge_request_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be purged
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_purge_request("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid because the requester is not authorized");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a purge request is invalid if the request doesn't come from the admin service's
    /// own node. The `CircuitPurgeRequest` is a local operation, other nodes should not be able
    /// to submit a purge request.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the disbanded circuit to be purged to the admin store
    /// 3. Call `validate_purge_request` with a valid circuit and requester info for "node_b"
    /// 4. Validate the call to `validate_purge_request` returns an error
    ///
    /// This test verifies the `validate_purge_request` returns as expected when the purge request
    /// does not come from the local node.
    #[test]
    fn test_validate_purge_request_invalid_requester() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the disbanded circuit to be purged
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_purge_request("01234-ABCDE", PUB_KEY, "node_b") {
            panic!("Should have been invalid as requester does not belong to the `node_a`");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit purge request is invalid if the circuit to be purged is still `Active`
    /// in the admin store. The `CircuitPurgeRequest` is only valid for circuits that have already
    /// been disbanded, a `circuit_status` of `Disbanded`.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the circuit to be purged to the admin store, with a `circuit_status` of `Active`
    /// 3. Call `validate_purge_request` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_purge_request` returns an error
    ///
    /// This test verifies the `validate_purge_request` returns an error when the circuit
    /// attempted to be purged has a status of `Active`. The purge action is only valid for
    /// currently `Disbanded` circuits.
    #[test]
    fn test_validate_purge_request_active_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_purge_request("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid due to circuit still being `Active`");
        }
        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit is able to be purged using the `CircuitPurgeRequest`.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the disbanded circuit to be purged to the admin store
    /// 3. Create `CircuitPurgeRequest` for the circuit previously added to the admin store, and
    ///    payload for the request
    /// 4. Validate the call to `submit` returns successfully
    /// 5. Validate the circuit has been purged from the admin store
    ///
    /// This test verifies a `CircuitPurgeRequest` submitted to the admin service is validated
    /// correctly and successfully removes the circuit from the admin store.
    #[test]
    fn test_purge_request() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();

        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let pub_key = context
            .get_public_key(&private_key)
            .expect("Unable to get corresponding public key");
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the disbanded circuit to be purged
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");
        // Make `CircuitPurgeRequest` and corresponding payload
        let mut request = admin::CircuitPurgeRequest::new();
        request.set_circuit_id("01234-ABCDE".to_string());

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_PURGE_REQUEST);
        header.set_requester(pub_key.into_bytes());
        header.set_requester_node_id("node_a".to_string());

        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_circuit_purge_request(request);
        // Submit `CircuitPurgeRequest` payload
        if let Err(err) = admin_shared.submit(payload) {
            panic!("Should have been valid: {}", err);
        }
        // Validate the corresponding circuit has been removed from the admin store
        if let Some(_) = admin_shared
            .admin_store
            .get_circuit(&"01234-ABCDE".to_string())
            .expect("Unable to get circuit")
        {
            panic!("Circuit should have been purged");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a circuit being abandoned is validated correctly
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the active circuit to be abandoned to the admin store
    /// 3. Call `validate_abandon_circuit` with a valid circuit and valid requester info
    /// 4. Validate the call to `validate_abandon_circuit` returns successfully
    ///
    /// This test verifies the `validate_abandon_circuit` returns successfully given a valid
    /// abandon request.
    #[test]
    fn test_validate_abandon_circuit_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be abandoned
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Err(err) = admin_shared.validate_abandon_circuit("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to abandon a circuit returns an error if the circuit does not exist.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Call `validate_abandon_circuit` with a circuit and valid requester info
    /// 3. Validate the call to `validate_abandon_circuit` returns an error
    #[test]
    fn test_validate_abandon_circuit_no_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        if let Ok(()) = admin_shared.validate_abandon_circuit("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit does not exist");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to abandon a circuit returns an error if the circuit is not active.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a `Disbanded` circuit to the admin store.
    /// 3. Call `validate_abandon_circuit` with the circuit and valid requester info
    /// 4. Validate the call to `validate_abandon_circuit` returns an error
    #[test]
    fn test_validate_abandon_circuit_invalid_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be abandoned
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Disbanded),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_abandon_circuit("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit is not active");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to abandon a circuit returns an error if the request does not come
    /// from the local node.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a `Active` circuit to the admin store.
    /// 3. Call `validate_abandon_circuit` with the circuit and requester info, specifying "node_b"
    ///    `requester_node_id`
    /// 4. Validate the call to `validate_abandon_circuit` returns an error
    #[test]
    fn test_validate_abandon_circuit_invalid_node_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be abandoned
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_abandon_circuit("01234-ABCDE", PUB_KEY, "node_b") {
            panic!("Should have been invalid because the request came from a remote node");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to abandon a circuit returns an error if the requester is not
    /// permitted for the admin service.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a `Active` circuit to the admin store
    /// 3. Call `validate_abandon_circuit` with the circuit and requester info
    /// 4. Validate the call to `validate_abandon_circuit` returns an error
    #[test]
    fn test_validate_abandon_circuit_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be abandoned
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");

        if let Ok(()) = admin_shared.validate_abandon_circuit("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid due to requester node not being registered");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to abandon a circuit is completed successfully, resulting in the
    /// updated circuit status.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add an `Active` circuit to the admin store
    /// 3. Create a `CircuitAbandon` message
    /// 4. Submit the `CircuitAbandon` request to the node's admin service
    /// 5. Fetch the circuit that was abandoned in the previous step
    /// 6. Validate the returned circuit has a `circuit_status` of `Abandoned`
    #[test]
    fn test_abandon_circuit() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let pub_key = context
            .get_public_key(&private_key)
            .expect("Unable to get corresponding public key");
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        // Add the circuit to be abandoned
        admin_shared
            .admin_store
            .add_circuit(
                store_circuit(CIRCUIT_PROTOCOL_VERSION, StoreCircuitStatus::Active),
                store_circuit_nodes(),
            )
            .expect("unable to add circuit to store");
        // Make `CircuitAbandon` and corresponding payload
        let mut abandon = admin::CircuitAbandon::new();
        abandon.set_circuit_id("01234-ABCDE".to_string());

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_ABANDON);
        header.set_requester(pub_key.into_bytes());
        header.set_requester_node_id("node_a".to_string());

        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_circuit_abandon(abandon);

        // Submit `CircuitAbandon` payload
        if let Err(err) = admin_shared.submit(payload) {
            panic!("Should have been valid: {}", err);
        }

        let abandoned_circuit = admin_shared
            .admin_store
            .get_circuit(&"01234-ABCDE".to_string())
            .expect("Unable to get circuit")
            .unwrap();
        assert_eq!(
            &StoreCircuitStatus::Abandoned,
            abandoned_circuit.circuit_status()
        );

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to remove a circuit proposal is validated correctly
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add the circuit proposal to be removed from the admin store
    /// 3. Call `validate_remove_proposal` with a valid proposal and valid requester info
    /// 4. Validate the call to `validate_remove_proposal` returns successfully
    ///
    /// This test verifies the `validate_remove_proposal` returns successfully given a valid
    /// circuit proposal remove request.
    #[test]
    fn test_validate_remove_proposal_valid() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );
        let store_proposal = StoreProposal::from_proto(setup_test_proposal(&setup_test_circuit()))
            .expect("Unable to build CircuitProposal");
        // Add the circuit proposal to be removed
        admin_shared
            .admin_store
            .add_proposal(store_proposal)
            .expect("Unable to add circuit proposal to store");

        if let Err(err) = admin_shared.validate_remove_proposal("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to remove a circuit proposal returns an error if the proposal does
    /// not exist.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Call `validate_remove_proposal` with a circuit proposal and valid requester info
    /// 3. Validate the call to `validate_remove_proposal` returns an error
    #[test]
    fn test_validate_remove_proposal_no_proposal() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        if let Ok(()) = admin_shared.validate_remove_proposal("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit proposal does not exist");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to remove a circuit proposal returns an error if the request does not
    /// come from the local node.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit proposal to the admin store.
    /// 3. Call `validate_remove_proposal` with the proposal and requester info, specifying
    ///     "node_b" as the `requester_node_id`
    /// 4. Validate the call to `validate_remove_proposal` returns an error
    #[test]
    fn test_validate_remove_proposal_invalid_node_id() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let store_proposal = StoreProposal::from_proto(setup_test_proposal(&setup_test_circuit()))
            .expect("Unable to build CircuitProposal");
        // Add the circuit proposal to be removed
        admin_shared
            .admin_store
            .add_proposal(store_proposal)
            .expect("Unable to add circuit proposal to store");

        if let Ok(()) = admin_shared.validate_remove_proposal("01234-ABCDE", PUB_KEY, "node_b") {
            panic!("Should have been invalid because the request came from a remote node");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a request to remove a circuit proposal returns an error if the requester is not
    /// permitted for the admin service.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit proposal to the admin store
    /// 3. Call `validate_remove_proposal` with the circuit proposal and requester info
    /// 4. Validate the call to `validate_remove_proposal` returns an error
    #[test]
    fn test_validate_remove_proposal_not_permitted() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let store_proposal = StoreProposal::from_proto(setup_test_proposal(&setup_test_circuit()))
            .expect("Unable to build CircuitProposal");
        // Add the circuit proposal to be removed
        admin_shared
            .admin_store
            .add_proposal(store_proposal)
            .expect("Unable to add circuit proposal to store");

        if let Ok(()) = admin_shared.validate_remove_proposal("01234-ABCDE", PUB_KEY, "node_a") {
            panic!("Should have been invalid due to requester node not being registered");
        }

        shutdown(mesh, cm, pm);
    }

    /// Tests that a `ProposalRemoveRequest` submitted to the admin service will result as expected,
    /// removing the indicated circuit proposa.
    ///
    /// 1. Set up `AdminServiceShared`
    /// 2. Add a circuit proposal to the admin store
    /// 3. Create a `ProposalRemoveRequest`, indicating the proposal added in the previous step
    /// 4. Submit the `ProposalRemoveRequest` to the node's admin service
    /// 5. Attempt to fetch the circuit proposal removed in the previous step
    /// 6. Validate the value returned is `None`
    #[test]
    fn test_remove_proposal() {
        let store = setup_admin_service_store();
        let event_store = store.clone_boxed();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let pub_key = context
            .get_public_key(&private_key)
            .expect("Unable to get corresponding public key");
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
            event_store,
            vec![],
        );

        let store_proposal = StoreProposal::from_proto(setup_test_proposal(&setup_test_circuit()))
            .expect("Unable to build CircuitProposal");
        // Add the circuit proposal to be removed
        admin_shared
            .admin_store
            .add_proposal(store_proposal)
            .expect("Unable to add circuit proposal to store");

        // Make `ProposalRemoveRequest` and corresponding payload
        let mut remove_proposal = admin::ProposalRemoveRequest::new();
        remove_proposal.set_circuit_id("01234-ABCDE".to_string());

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::PROPOSAL_REMOVE_REQUEST);
        header.set_requester(pub_key.into_bytes());
        header.set_requester_node_id("node_a".to_string());

        let mut payload = admin::CircuitManagementPayload::new();
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_proposal_remove_request(remove_proposal);

        // Submit `ProposalRemoveRequest` payload
        if let Err(err) = admin_shared.submit(payload) {
            panic!("Should have been valid: {}", err);
        }

        let proposal_option = admin_shared
            .admin_store
            .get_proposal(&"01234-ABCDE".to_string())
            .expect("Unable to get circuit proposal");
        assert!(proposal_option.is_none());

        shutdown(mesh, cm, pm);
    }

    pub fn setup_test_circuit() -> Circuit {
        let mut service_a = SplinterService::new();
        service_a.set_service_id("0123".to_string());
        service_a.set_service_type("type_a".to_string());
        service_a.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        let mut service_b = SplinterService::new();
        service_b.set_service_id("ABCD".to_string());
        service_b.set_service_type("type_a".to_string());
        service_b.set_allowed_nodes(RepeatedField::from_vec(vec!["node_b".to_string()]));

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        let mut circuit = Circuit::new();
        circuit.set_circuit_id("01234-ABCDE".to_string());
        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));
        circuit.set_roster(RepeatedField::from_vec(vec![
            service_b.clone(),
            service_a.clone(),
        ]));
        circuit.set_authorization_type(Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_durability(Circuit_DurabilityType::NO_DURABILITY);
        circuit.set_routes(Circuit_RouteType::ANY_ROUTE);
        circuit.set_circuit_management_type("test_circuit".to_string());
        circuit.set_application_metadata(b"test_data".to_vec());
        circuit.set_comments("test circuit".to_string());
        circuit.set_display_name("test_display".into());
        circuit.set_circuit_status(admin::Circuit_CircuitStatus::ACTIVE);
        circuit.set_circuit_version(2);

        circuit
    }

    pub fn setup_v1_test_circuit() -> Circuit {
        let mut service_a = SplinterService::new();
        service_a.set_service_id("0123".to_string());
        service_a.set_service_type("type_a".to_string());
        service_a.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        let mut service_b = SplinterService::new();
        service_b.set_service_id("ABCD".to_string());
        service_b.set_service_type("type_a".to_string());
        service_b.set_allowed_nodes(RepeatedField::from_vec(vec!["node_b".to_string()]));

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        let mut circuit = Circuit::new();
        circuit.set_circuit_id("01234-ABCDE".to_string());
        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));
        circuit.set_roster(RepeatedField::from_vec(vec![
            service_b.clone(),
            service_a.clone(),
        ]));
        circuit.set_authorization_type(Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_durability(Circuit_DurabilityType::NO_DURABILITY);
        circuit.set_routes(Circuit_RouteType::ANY_ROUTE);
        circuit.set_circuit_management_type("test_circuit".to_string());
        circuit.set_application_metadata(b"test_data".to_vec());
        circuit.set_comments("test circuit".to_string());
        circuit.set_display_name("test_display".into());
        circuit.set_circuit_status(admin::Circuit_CircuitStatus::ACTIVE);
        circuit.set_circuit_version(1);

        circuit
    }

    fn setup_test_vote(circuit: &Circuit) -> CircuitProposalVote {
        let mut circuit_vote = CircuitProposalVote::new();
        circuit_vote.set_vote(CircuitProposalVote_Vote::ACCEPT);
        circuit_vote.set_circuit_id(circuit.get_circuit_id().to_string());
        let circuit_hash = sha256(circuit).unwrap();
        circuit_vote.set_circuit_hash(circuit_hash);

        circuit_vote
    }

    fn setup_test_proposal(proposed_circuit: &Circuit) -> CircuitProposal {
        let mut circuit_proposal = CircuitProposal::new();
        circuit_proposal.set_proposal_type(CircuitProposal_ProposalType::CREATE);
        circuit_proposal.set_circuit_id(proposed_circuit.get_circuit_id().into());
        circuit_proposal.set_circuit_hash(sha256(proposed_circuit).unwrap());
        circuit_proposal.set_circuit_proposal(proposed_circuit.clone());
        circuit_proposal.set_requester(b"test_signer_b".to_vec());
        circuit_proposal.set_requester_node_id("node_b".to_string());

        circuit_proposal
    }

    fn setup_admin_service_store() -> Box<dyn AdminServiceStore> {
        let connection_manager = DieselConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        Box::new(DieselAdminServiceStore::new(pool))
    }

    fn setup_peer_connector(
        inproct_transport: Option<InprocTransport>,
    ) -> (Mesh, ConnectionManager, PeerManager, PeerManagerConnector) {
        let transport = {
            if let Some(transport) = inproct_transport {
                transport
            } else {
                InprocTransport::default()
            }
        };

        let mesh = Mesh::new(2, 2);
        let inproc_authorizer = InprocAuthorizer::new(
            vec![
                (
                    "inproc://orchestator".to_string(),
                    "orchestator".to_string(),
                ),
                (
                    "inproc://otherplace:8000".to_string(),
                    "other-node".to_string(),
                ),
                (
                    "inproc://someplace:8000".to_string(),
                    "test-node".to_string(),
                ),
            ],
            "node_id".to_string(),
        );

        let authorization_manager = AuthorizationManager::new(
            "test-node".into(),
            #[cfg(feature = "challenge-authorization")]
            vec![],
            #[cfg(feature = "challenge-authorization")]
            Arc::new(Mutex::new(Box::new(Secp256k1Context::new()))),
        )
        .expect("Unable to create authorization pool");
        let mut authorizers = Authorizers::new();
        authorizers.add_authorizer("inproc", inproc_authorizer);
        authorizers.add_authorizer("", authorization_manager.authorization_connector());
        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(authorizers))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(Box::new(transport.clone()))
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        let pm = PeerManager::builder()
            .with_connector(connector)
            .with_retry_interval(1)
            .with_identity("my_id".to_string())
            .with_strict_ref_counts(true)
            .start()
            .expect("Cannot start peer_manager");
        let peer_connector = pm.connector();
        (mesh, cm, pm, peer_connector)
    }

    fn shutdown(mut mesh: Mesh, mut cm: ConnectionManager, mut pm: PeerManager) {
        pm.signal_shutdown();
        cm.signal_shutdown();
        pm.wait_for_shutdown()
            .expect("Unable to shutdown peer manager");
        cm.wait_for_shutdown()
            .expect("Unable to shutdown connection manager");
        mesh.signal_shutdown();
        mesh.wait_for_shutdown().expect("Unable to shutdown mesh");
    }

    fn setup_orchestrator() -> ServiceOrchestrator {
        let mut transport =
            MockConnectingTransport::expect_connections(vec![Ok(Box::new(MockConnection::new()))]);
        let orchestrator_connection = transport
            .connect("inproc://orchestator-service")
            .expect("failed to create connection");

        ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .expect("failed to create orchestrator")
            .run()
            .expect("failed to start orchestrator")
    }

    fn splinter_node(node_id: &str, endpoints: &[String]) -> admin::SplinterNode {
        let mut node = admin::SplinterNode::new();
        node.set_node_id(node_id.into());
        node.set_endpoints(endpoints.into());
        node
    }

    fn splinter_service(service_id: &str, service_type: &str) -> admin::SplinterService {
        let mut service = admin::SplinterService::new();
        service.set_service_id(service_id.into());
        service.set_service_type(service_type.into());
        service.set_allowed_nodes(RepeatedField::from_vec(vec!["node_id".into()]));
        service
    }

    fn store_circuit(version: i32, status: StoreCircuitStatus) -> StoreCircuit {
        let nodes = store_circuit_nodes();
        store::CircuitBuilder::new()
            .with_circuit_id("01234-ABCDE")
            .with_roster(&vec![
                store::ServiceBuilder::new()
                    .with_service_id("0123")
                    .with_service_type("type_a")
                    .with_node_id("node_a")
                    .build()
                    .expect("unable to build admin store Service"),
                store::ServiceBuilder::new()
                    .with_service_id("ABCD")
                    .with_service_type("type_a")
                    .with_node_id("node_b")
                    .build()
                    .expect("unable to build admin store Service"),
            ])
            .with_members(&nodes)
            .with_authorization_type(&store::AuthorizationType::Trust)
            .with_persistence(&store::PersistenceType::Any)
            .with_durability(&store::DurabilityType::NoDurability)
            .with_routes(&store::RouteType::Any)
            .with_circuit_management_type("test_circuit")
            .with_display_name("test_display")
            .with_circuit_version(version)
            .with_circuit_status(&status)
            .build()
            .expect("unable to build store Circuit")
    }

    fn store_circuit_nodes() -> Vec<CircuitNode> {
        vec![
            store::CircuitNodeBuilder::new()
                .with_node_id("node_a")
                .with_endpoints(&vec!["test://endpoint_a:0".to_string()])
                .build()
                .expect("unable to build store CircuitNode"),
            store::CircuitNodeBuilder::new()
                .with_node_id("node_b")
                .with_endpoints(&vec!["test://endpoint_b:0".to_string()])
                .build()
                .expect("unable to build store CircuitNode"),
        ]
    }

    struct MockConnectingTransport {
        connection_results: VecDeque<Result<Box<dyn Connection>, ConnectError>>,
    }

    impl MockConnectingTransport {
        fn expect_connections(results: Vec<Result<Box<dyn Connection>, ConnectError>>) -> Self {
            Self {
                connection_results: results.into_iter().collect(),
            }
        }
    }

    impl Transport for MockConnectingTransport {
        fn accepts(&self, _: &str) -> bool {
            true
        }

        fn connect(&mut self, _: &str) -> Result<Box<dyn Connection>, ConnectError> {
            self.connection_results
                .pop_front()
                .expect("No test result added to mock")
        }

        fn listen(
            &mut self,
            _: &str,
        ) -> Result<Box<dyn crate::transport::Listener>, crate::transport::ListenError> {
            panic!("MockConnectingTransport.listen unexpectedly called")
        }
    }

    struct MockConnection {
        evented: MockEvented,
    }

    impl MockConnection {
        fn new() -> Self {
            Self {
                evented: MockEvented::new(),
            }
        }
    }

    impl Connection for MockConnection {
        fn send(&mut self, _message: &[u8]) -> Result<(), SendError> {
            Ok(())
        }

        fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
            Ok(vec![])
        }

        fn remote_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn local_endpoint(&self) -> String {
            String::from("MockConnection")
        }

        fn disconnect(&mut self) -> Result<(), DisconnectError> {
            Ok(())
        }

        fn evented(&self) -> &dyn mio::Evented {
            &self.evented
        }
    }

    struct MockEvented {
        registration: mio::Registration,
        set_readiness: mio::SetReadiness,
    }

    impl MockEvented {
        fn new() -> Self {
            let (registration, set_readiness) = mio::Registration::new2();

            Self {
                registration,
                set_readiness,
            }
        }
    }

    impl mio::Evented for MockEvented {
        fn register(
            &self,
            poll: &mio::Poll,
            token: mio::Token,
            interest: mio::Ready,
            opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            self.registration.register(poll, token, interest, opts)?;
            self.set_readiness.set_readiness(mio::Ready::readable())?;

            Ok(())
        }

        fn reregister(
            &self,
            poll: &mio::Poll,
            token: mio::Token,
            interest: mio::Ready,
            opts: mio::PollOpt,
        ) -> std::io::Result<()> {
            self.registration.reregister(poll, token, interest, opts)
        }

        fn deregister(&self, poll: &mio::Poll) -> std::io::Result<()> {
            poll.deregister(&self.registration)
        }
    }

    #[derive(Clone, Debug)]
    pub struct MockServiceNetworkSender {
        pub sent: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
        pub sent_and_awaited: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
        pub replied: Arc<Mutex<Vec<(ServiceMessageContext, Vec<u8>)>>>,
    }

    impl MockServiceNetworkSender {
        pub fn new() -> Self {
            MockServiceNetworkSender {
                sent: Arc::new(Mutex::new(vec![])),
                sent_and_awaited: Arc::new(Mutex::new(vec![])),
                replied: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl ServiceNetworkSender for MockServiceNetworkSender {
        fn send(&self, recipient: &str, message: &[u8]) -> Result<(), ServiceSendError> {
            self.sent
                .lock()
                .expect("sent lock poisoned")
                .push((recipient.to_string(), message.to_vec()));
            Ok(())
        }

        fn send_and_await(
            &self,
            recipient: &str,
            message: &[u8],
        ) -> Result<Vec<u8>, ServiceSendError> {
            self.sent_and_awaited
                .lock()
                .expect("sent_and_awaited lock poisoned")
                .push((recipient.to_string(), message.to_vec()));
            Ok(vec![])
        }

        fn reply(
            &self,
            message_origin: &ServiceMessageContext,
            message: &[u8],
        ) -> Result<(), ServiceSendError> {
            self.replied
                .lock()
                .expect("replied lock poisoned")
                .push((message_origin.clone(), message.to_vec()));
            Ok(())
        }

        fn clone_box(&self) -> Box<dyn ServiceNetworkSender> {
            Box::new(self.clone())
        }

        fn send_with_sender(
            &mut self,
            recipient: &str,
            message: &[u8],
            _sender: &str,
        ) -> Result<(), ServiceSendError> {
            self.sent
                .lock()
                .expect("sent lock poisoned")
                .push((recipient.to_string(), message.to_vec()));
            Ok(())
        }
    }

    struct MockAdminKeyVerifier(bool);

    impl MockAdminKeyVerifier {
        fn new(is_permitted: bool) -> Self {
            Self(is_permitted)
        }
    }

    impl Default for MockAdminKeyVerifier {
        fn default() -> Self {
            Self::new(true)
        }
    }

    impl AdminKeyVerifier for MockAdminKeyVerifier {
        fn is_permitted(&self, _node_id: &str, _key: &[u8]) -> Result<bool, AdminKeyVerifierError> {
            Ok(self.0)
        }
    }

    fn handle_auth(mesh: &Mesh, connection_id: &str, identity: &str) {
        let _env = mesh.recv().unwrap();
        // send our own connect request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::ConnectRequest(ConnectRequest::Unidirectional),
        );
        mesh.send(env).expect("Unable to send connect request");

        let _env = mesh.recv().unwrap();
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::ConnectResponse(ConnectResponse {
                accepted_authorization_types: vec![AuthorizationType::Trust],
            }),
        );
        mesh.send(env).expect("Unable to send connect response");
        let _env = mesh.recv().unwrap();

        let env = write_auth_message(connection_id, AuthorizationMessage::Authorized(Authorized));
        mesh.send(env).expect("unable to send authorized");

        // send trust request
        let env = write_auth_message(
            connection_id,
            AuthorizationMessage::TrustRequest(TrustRequest {
                identity: identity.to_string(),
            }),
        );
        mesh.send(env).expect("Unable to send trust request");
        let _env = mesh.recv().unwrap();
    }

    fn write_auth_message(connection_id: &str, auth_msg: AuthorizationMessage) -> Envelope {
        let msg = NetworkMessage::from(auth_msg);

        Envelope::new(
            connection_id.to_string(),
            IntoBytes::<network::NetworkMessage>::into_bytes(msg)
                .expect("Unable to write to bytes"),
        )
    }
}
