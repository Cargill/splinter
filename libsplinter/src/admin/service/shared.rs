// Copyright 2018-2020 Cargill Incorporated
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

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::iter::ExactSizeIterator;
use std::iter::FromIterator;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use cylinder::{PublicKey, Signature, Verifier as SignatureVerifier};
use protobuf::Message;

use crate::admin::store::{
    AdminServiceStore, Circuit as StoreCircuit, CircuitNode, CircuitPredicate,
    CircuitProposal as StoreProposal, ProposalType, ProposedNode, Vote, VoteRecordBuilder,
};
use crate::circuit::routing::{self, RoutingTableWriter};
use crate::consensus::{Proposal, ProposalId, ProposalUpdate};
use crate::hex::to_hex;
use crate::keys::KeyPermissionManager;
use crate::orchestrator::{ServiceDefinition, ServiceOrchestrator};
use crate::peer::{PeerManagerConnector, PeerRef};
use crate::protocol::{ADMIN_SERVICE_PROTOCOL_MIN, ADMIN_SERVICE_PROTOCOL_VERSION};
#[cfg(feature = "service-arg-validation")]
use crate::protos::admin::SplinterService;
use crate::protos::admin::{
    AdminMessage, AdminMessage_Type, Circuit, CircuitManagementPayload,
    CircuitManagementPayload_Action, CircuitManagementPayload_Header, CircuitProposal,
    CircuitProposalVote, CircuitProposalVote_Vote, CircuitProposal_ProposalType,
    Circuit_AuthorizationType, Circuit_DurabilityType, Circuit_PersistenceType, Circuit_RouteType,
    MemberReady, ServiceProtocolVersionRequest, SplinterNode,
};
use crate::service::error::ServiceError;
#[cfg(feature = "service-arg-validation")]
use crate::service::validation::ServiceArgValidator;

use crate::service::ServiceNetworkSender;
use crate::sets::mem::DurableBTreeSet;

use super::error::{AdminSharedError, MarshallingError};
use super::mailbox::Mailbox;
use super::messages;
use super::{
    admin_service_id, sha256, AdminKeyVerifier, AdminServiceEventSubscriber, AdminSubscriberError,
    Events,
};

static VOTER_ROLE: &str = "voter";
static PROPOSER_ROLE: &str = "proposer";

const DEFAULT_IN_MEMORY_EVENT_LIMIT: usize = 100;

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
    pub unpeered_ids: Vec<String>,
    pub missing_protocol_ids: Vec<String>,
    pub payload_type: PayloadType,
    pub message_sender: String,
    pub members: Vec<String>,
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

struct SubscriberMap {
    subscribers_by_type: RefCell<HashMap<String, Vec<Box<dyn AdminServiceEventSubscriber>>>>,
}

impl SubscriberMap {
    fn new() -> Self {
        Self {
            subscribers_by_type: RefCell::new(HashMap::new()),
        }
    }

    fn broadcast_by_type(
        &self,
        event_type: &str,
        admin_service_event: &messages::AdminServiceEvent,
        timestamp: &SystemTime,
    ) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        if let Some(subscribers) = subscribers_by_type.get_mut(event_type) {
            subscribers.retain(|subscriber| {
                match subscriber.handle_event(admin_service_event, timestamp) {
                    Ok(()) => true,
                    Err(AdminSubscriberError::Unsubscribe) => false,
                    Err(AdminSubscriberError::UnableToHandleEvent(msg)) => {
                        error!("Unable to send event: {}", msg);
                        true
                    }
                }
            });
        }
    }

    fn add_subscriber(
        &mut self,
        event_type: String,
        listener: Box<dyn AdminServiceEventSubscriber>,
    ) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        let subscribers = subscribers_by_type
            .entry(event_type)
            .or_insert_with(Vec::new);
        subscribers.push(listener);
    }

    fn clear(&mut self) {
        self.subscribers_by_type.borrow_mut().clear()
    }
}

pub struct AdminServiceShared {
    // the node id of the connected splinter node
    node_id: String,
    // the list of circuit that have been committed to splinter state but whose services haven't
    // been initialized
    uninitialized_circuits: HashMap<String, UninitializedCircuit>,
    orchestrator: Arc<Mutex<ServiceOrchestrator>>,
    // map of service arg validators, by service type
    #[cfg(feature = "service-arg-validation")]
    service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
    // peer connector used to connect to new members listed in a circuit
    peer_connector: PeerManagerConnector,
    // PeerRef Map, peer_id to PeerRef, these PeerRef should be dropped when the peer is no longer
    // needed
    peer_refs: HashMap<String, Vec<PeerRef>>,
    // network sender is used to comunicated with other services on the splinter network
    network_sender: Option<Box<dyn ServiceNetworkSender>>,
    // the CircuitManagementPayloads that are waiting for members to be peered
    unpeered_payloads: Vec<PendingPayload>,
    // the CircuitManagementPayloads that require the peers' admin services to negotiate a protocol
    // version
    pending_protocol_payloads: Vec<PendingPayload>,
    // the agreed upon protocol version between another admin service, map of service id to
    // version protocol
    service_protocols: HashMap<String, u32>,
    // CircuitManagmentPayloads that still need to go through consensus
    pending_circuit_payloads: VecDeque<CircuitManagementPayload>,
    // The pending consensus proposals
    pending_consensus_proposals: HashMap<ProposalId, (Proposal, CircuitManagementPayload)>,
    // the pending changes for the current proposal
    pending_changes: Option<CircuitProposalContext>,
    // the verifiers that should be broadcasted for the pending change
    current_consensus_verifiers: Vec<String>,
    // Admin Service Event Subscribers
    event_subscribers: SubscriberMap,
    // Mailbox of AdminServiceEvent values
    event_mailbox: Mailbox,
    // AdminServiceStore
    admin_store: Box<dyn AdminServiceStore>,
    // signature verifier
    signature_verifier: Box<dyn SignatureVerifier>,
    key_verifier: Box<dyn AdminKeyVerifier>,
    key_permission_manager: Box<dyn KeyPermissionManager>,
    proposal_sender: Option<Sender<ProposalUpdate>>,

    admin_service_status: AdminServiceStatus,
    routing_table_writer: Box<dyn RoutingTableWriter>,
}

impl AdminServiceShared {
    #![allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: String,
        orchestrator: Arc<Mutex<ServiceOrchestrator>>,
        #[cfg(feature = "service-arg-validation")] service_arg_validators: HashMap<
            String,
            Box<dyn ServiceArgValidator + Send>,
        >,
        peer_connector: PeerManagerConnector,
        admin_store: Box<dyn AdminServiceStore>,
        signature_verifier: Box<dyn SignatureVerifier>,
        key_verifier: Box<dyn AdminKeyVerifier>,
        key_permission_manager: Box<dyn KeyPermissionManager>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
    ) -> Result<Self, ServiceError> {
        let event_mailbox = Mailbox::new(DurableBTreeSet::new_boxed_with_bound(
            std::num::NonZeroUsize::new(DEFAULT_IN_MEMORY_EVENT_LIMIT).unwrap(),
        ));

        Ok(AdminServiceShared {
            node_id,
            network_sender: None,
            uninitialized_circuits: Default::default(),
            orchestrator,
            #[cfg(feature = "service-arg-validation")]
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
            event_mailbox,
            admin_store,
            signature_verifier,
            key_verifier,
            key_permission_manager,
            proposal_sender: None,
            admin_service_status: AdminServiceStatus::NotRunning,
            routing_table_writer,
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
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

    pub fn current_consensus_verifiers(&self) -> &Vec<String> {
        &self.current_consensus_verifiers
    }

    pub fn add_peer_ref(&mut self, peer_ref: PeerRef) {
        if let Some(peer_ref_vec) = self.peer_refs.get_mut(peer_ref.peer_id()) {
            peer_ref_vec.push(peer_ref);
        } else {
            self.peer_refs
                .insert(peer_ref.peer_id().to_string(), vec![peer_ref]);
        }
    }

    pub fn add_peer_refs(&mut self, peer_refs: Vec<PeerRef>) {
        for peer_ref in peer_refs {
            self.add_peer_ref(peer_ref);
        }
    }

    pub fn remove_peer_ref(&mut self, peer_id: &str) {
        if let Some(mut peer_ref_vec) = self.peer_refs.remove(peer_id) {
            peer_ref_vec.pop();
            if !peer_ref_vec.is_empty() {
                self.peer_refs.insert(peer_id.to_string(), peer_ref_vec);
            }
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
                    Ok(CircuitProposalStatus::Accepted) => {
                        // commit new circuit
                        self.admin_store.upgrade_proposal_to_circuit(circuit_id)?;
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
                            circuit.members().to_vec(),
                        );

                        let routing_members = circuit_proposal
                            .get_circuit_proposal()
                            .get_members()
                            .iter()
                            .map(|node| {
                                routing::CircuitNode::new(
                                    node.get_node_id().to_string(),
                                    node.get_endpoints().to_vec(),
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
                            for member in circuit.members().iter() {
                                if member != &self.node_id {
                                    network_sender
                                        .send(&admin_service_id(member), &envelope_bytes)?;
                                }
                            }
                        }

                        // add circuit as pending initialization
                        self.add_uninitialized_circuit(circuit_proposal.clone())
                    }
                    Ok(CircuitProposalStatus::Pending) => {
                        self.add_proposal(circuit_proposal.clone())?;

                        match action {
                            CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST => {
                                // notify registered application authorization handlers of the
                                // committed circuit proposal
                                let event = messages::AdminServiceEvent::ProposalSubmitted(
                                    messages::CircuitProposal::from_proto(circuit_proposal.clone())
                                        .map_err(AdminSharedError::InvalidMessageFormat)?,
                                );
                                self.send_event(&mgmt_type, event);

                                info!("committed changes for new circuit proposal {}", circuit_id);
                                Ok(())
                            }

                            CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE => {
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
                            _ => Err(AdminSharedError::UnknownAction(format!(
                                "Received unknown action: {:?}",
                                action
                            ))),
                        }
                    }
                    Ok(CircuitProposalStatus::Rejected) => {
                        // remove circuit
                        let proposal = self.remove_proposal(&circuit_id)?;
                        if let Some(proposal) = proposal {
                            for member in proposal.circuit().members().iter() {
                                self.remove_peer_ref(member.node_id());
                            }
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
                    Err(err) => Err(err),
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
        let header = protobuf::parse_from_bytes::<CircuitManagementPayload_Header>(
            circuit_payload.get_header(),
        )
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
                for member in proposed_circuit.get_members() {
                    verifiers.push(admin_service_id(member.get_node_id()));
                }

                let signer_public_key = header.get_requester();
                let requester_node_id = header.get_requester_node_id();

                self.validate_create_circuit(
                    &proposed_circuit,
                    signer_public_key,
                    requester_node_id,
                )
                .map_err(|err| {
                    // remove peer_ref because we will not accept this proposal
                    for member in proposed_circuit.get_members() {
                        self.remove_peer_ref(member.get_node_id())
                    }
                    err
                })?;
                debug!("proposing {}", proposed_circuit.get_circuit_id());

                let mut circuit_proposal = CircuitProposal::new();
                circuit_proposal.set_proposal_type(CircuitProposal_ProposalType::CREATE);
                circuit_proposal.set_circuit_id(proposed_circuit.get_circuit_id().into());
                circuit_proposal.set_circuit_hash(sha256(&proposed_circuit)?);
                circuit_proposal.set_circuit_proposal(proposed_circuit);
                circuit_proposal.set_requester(header.get_requester().to_vec());
                circuit_proposal.set_requester_node_id(header.get_requester_node_id().to_string());

                let expected_hash = sha256(&circuit_proposal)?;
                self.pending_changes = Some(CircuitProposalContext {
                    circuit_proposal: circuit_proposal.clone(),
                    signer_public_key: header.get_requester().to_vec(),
                    action: CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST,
                });
                self.current_consensus_verifiers = verifiers;

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
                        // remove peer_ref because we will not accept this proposal
                        for member in circuit_proposal.circuit().members() {
                            self.remove_peer_ref(member.node_id())
                        }
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
                    .with_public_key(signer_public_key)
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

                let proto_circuit_proposal = circuit_proposal.into_proto();

                let expected_hash = sha256(&proto_circuit_proposal)?;
                self.pending_changes = Some(CircuitProposalContext {
                    circuit_proposal: proto_circuit_proposal.clone(),
                    signer_public_key: header.get_requester().to_vec(),
                    action: CircuitManagementPayload_Action::CIRCUIT_PROPOSAL_VOTE,
                });
                self.current_consensus_verifiers = verifiers;
                Ok((expected_hash, proto_circuit_proposal))
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

        // get members as vec to payload can be sent to helper function as well
        let members = payload
            .get_circuit_create_request()
            .get_circuit()
            .get_members()
            .to_vec();
        self.check_connected_peers_payload_create(&members, payload, message_sender)
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

        self.check_connected_peers_payload_vote(
            &proposal.circuit().members(),
            payload,
            message_sender,
        )
    }

    pub fn send_protocol_request(&mut self, node_id: &str) -> Result<(), ServiceError> {
        if self
            .service_protocols
            .get(&admin_service_id(node_id))
            .is_none()
        {
            // we will always have the network sender at this point
            if let Some(ref network_sender) = self.network_sender {
                debug!(
                    "Sending service protocol request to {}",
                    admin_service_id(node_id)
                );
                let mut request = ServiceProtocolVersionRequest::new();
                request.set_protocol_min(ADMIN_SERVICE_PROTOCOL_MIN);
                request.set_protocol_max(ADMIN_SERVICE_PROTOCOL_VERSION);
                let mut msg = AdminMessage::new();
                msg.set_message_type(AdminMessage_Type::SERVICE_PROTOCOL_VERSION_REQUEST);
                msg.set_protocol_request(request);

                let envelope_bytes = msg.write_to_bytes()?;
                network_sender.send(&admin_service_id(node_id), &envelope_bytes)?;
            }
        } else {
            debug!(
                "Already agreed on protocol version with {}",
                admin_service_id(node_id)
            );
        }
        Ok(())
    }

    fn check_connected_peers_payload_vote(
        &mut self,
        members: &[ProposedNode],
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_members = vec![];
        for node in members {
            if self.node_id() != node.node_id()
                && self
                    .service_protocols
                    .get(&admin_service_id(node.node_id()))
                    .is_none()
            {
                self.send_protocol_request(node.node_id())?;
                missing_protocol_ids.push(admin_service_id(node.node_id()))
            }
            pending_members.push(node.node_id().to_string());
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
        members: &[SplinterNode],
        payload: CircuitManagementPayload,
        message_sender: String,
    ) -> Result<(), ServiceError> {
        let mut missing_protocol_ids = vec![];
        let mut pending_peers = vec![];
        let mut pending_members = vec![];
        let mut added_peers: Vec<String> = vec![];
        for node in members {
            if self.node_id() != node.get_node_id() {
                debug!("Referencing node {:?}", node);
                let peer_ref = self
                    .peer_connector
                    .add_peer_ref(
                        node.get_node_id().to_string(),
                        node.get_endpoints().to_vec(),
                    )
                    .map_err(|err| {
                        // remove all peer refs added for this proposal
                        for node_id in added_peers.iter() {
                            self.remove_peer_ref(node_id);
                        }

                        ServiceError::UnableToHandleMessage(Box::new(err))
                    })?;

                self.add_peer_ref(peer_ref);
                added_peers.push(node.get_node_id().to_string());

                // if we have a protocol the connection exists for the peer already
                if self
                    .service_protocols
                    .get(&admin_service_id(node.get_node_id()))
                    .is_none()
                {
                    pending_peers.push(node.get_node_id().to_string());
                    missing_protocol_ids.push(admin_service_id(node.get_node_id()))
                }
            }
            pending_members.push(node.get_node_id().to_string())
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

    pub fn submit(&mut self, payload: CircuitManagementPayload) -> Result<(), ServiceError> {
        debug!("Payload submitted: {:?}", payload);

        let header =
            protobuf::parse_from_bytes::<CircuitManagementPayload_Header>(payload.get_header())?;
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
        let mut added_peers: Vec<String> = vec![];
        let mut pending_members = vec![];
        for node in payload
            .get_circuit_create_request()
            .get_circuit()
            .get_members()
        {
            if self.node_id() != node.get_node_id() {
                debug!("Referencing node {:?}", node);
                let peer_ref = self
                    .peer_connector
                    .add_peer_ref(
                        node.get_node_id().to_string(),
                        node.get_endpoints().to_vec(),
                    )
                    .map_err(|err| {
                        // remove all peer refs added for this proposal
                        for node_id in added_peers.iter() {
                            self.remove_peer_ref(node_id);
                        }

                        ServiceError::UnableToHandleMessage(Box::new(err))
                    })?;

                self.add_peer_ref(peer_ref);
                added_peers.push(node.get_node_id().to_string());

                // if we have a protocol the connection exists for the peer already
                if self
                    .service_protocols
                    .get(&admin_service_id(node.get_node_id()))
                    .is_none()
                {
                    pending_peers.push(node.get_node_id().to_string());
                    missing_protocol_ids.push(admin_service_id(node.get_node_id()))
                }
            }
            pending_members.push(node.get_node_id().to_string())
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

    pub fn get_events_since(
        &self,
        since_timestamp: &SystemTime,
        circuit_management_type: &str,
    ) -> Result<Events, AdminSharedError> {
        let events = self
            .event_mailbox
            .iter_since(*since_timestamp)
            .map_err(|err| AdminSharedError::UnableToAddSubscriber(err.to_string()))?;

        let circuit_management_type = circuit_management_type.to_string();
        Ok(Events {
            inner: Box::new(events.filter(move |(_, evt)| {
                evt.proposal().circuit.circuit_management_type == circuit_management_type
            })),
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
        let (ts, event) = match self.event_mailbox.add(event) {
            Ok((ts, event)) => (ts, event),
            Err(err) => {
                error!("Unable to store admin event: {}", err);
                return;
            }
        };

        self.event_subscribers
            .broadcast_by_type(&circuit_management_type, &event, &ts);
    }

    pub fn remove_all_event_subscribers(&mut self) {
        self.event_subscribers.clear();
    }

    pub fn on_peer_disconnected(&mut self, peer_id: String) {
        self.service_protocols.remove(&admin_service_id(&peer_id));
        let mut pending_protocol_payloads =
            std::mem::replace(&mut self.pending_protocol_payloads, vec![]);

        // Add peer back to any pending payloads
        for pending_protocol_payload in pending_protocol_payloads.iter_mut() {
            if pending_protocol_payload.members.contains(&peer_id) {
                pending_protocol_payload
                    .missing_protocol_ids
                    .push(peer_id.to_string())
            }
        }

        let (peering, protocol): (Vec<PendingPayload>, Vec<PendingPayload>) =
            pending_protocol_payloads
                .into_iter()
                .partition(|pending_payload| {
                    pending_payload.missing_protocol_ids.contains(&peer_id)
                });

        self.pending_protocol_payloads = protocol;
        // Add peer back to any pending payloads
        let mut unpeered_payloads = std::mem::replace(&mut self.unpeered_payloads, vec![]);
        for unpeered_payload in unpeered_payloads.iter_mut() {
            if unpeered_payload.members.contains(&peer_id) {
                unpeered_payload.unpeered_ids.push(peer_id.to_string())
            }
        }
        // add payloads that are not waiting on peer connection
        unpeered_payloads.extend(peering);
        self.unpeered_payloads = unpeered_payloads;
    }

    pub fn on_peer_connected(&mut self, peer_id: &str) -> Result<(), AdminSharedError> {
        let mut unpeered_payloads = std::mem::replace(&mut self.unpeered_payloads, vec![]);
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
        if peer_id == admin_service_id(self.node_id()) {
            return Ok(());
        }

        // We have already received a service protocol request, don't sent another request
        if self
            .service_protocols
            .get(&admin_service_id(peer_id))
            .is_some()
        {
            return Ok(());
        }

        // Send protocol request
        if let Some(ref network_sender) = self.network_sender {
            debug!(
                "Sending service protocol request to {}",
                admin_service_id(peer_id)
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

            network_sender
                .send(&admin_service_id(peer_id), &envelope_bytes)
                .map_err(|err| {
                    AdminSharedError::ServiceProtocolError(format!(
                        "Unable to send service protocol request: {}",
                        err
                    ))
                })?;
        } else {
            return Err(AdminSharedError::ServiceProtocolError(format!(
                "AdminService is not started, can't sent request to {}",
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
        let mut pending_protocol_payloads =
            std::mem::replace(&mut self.pending_protocol_payloads, vec![]);
        for pending_protocol_payload in pending_protocol_payloads.iter_mut() {
            match protocol {
                0 => {
                    if pending_protocol_payload
                        .missing_protocol_ids
                        .iter()
                        .any(|missing_protocol_id| missing_protocol_id == service_id)
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
                        .retain(|missing_protocol_id| missing_protocol_id != service_id);
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
                for peer in pending_payload.members {
                    self.remove_peer_ref(&peer);
                }
            }
            return Ok(());
        }

        self.service_protocols.insert(service_id.into(), protocol);
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

    /// Add a circuit definition as an uninitialized circuit. If all members are ready, initialize
    /// services.
    fn add_uninitialized_circuit(
        &mut self,
        circuit: CircuitProposal,
    ) -> Result<(), AdminSharedError> {
        let circuit_id = circuit.get_circuit_id().to_string();

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

        self.initialize_services_if_members_ready(&circuit_id)
    }

    /// Mark member node as ready to initialize services on the given circuit. If all members are
    /// now ready, initialize services.
    pub fn add_ready_member(
        &mut self,
        circuit_id: &str,
        member_node_id: String,
    ) -> Result<(), AdminSharedError> {
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

        self.initialize_services_if_members_ready(circuit_id)
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
                    let all_members = HashSet::from_iter(
                        circuit_proposal
                            .get_circuit_proposal()
                            .members
                            .iter()
                            .map(|node| node.node_id.clone()),
                    );
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
    ) -> Result<(), AdminSharedError> {
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
                "Ignoring duplicate create proposal of circuit {}",
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

            #[cfg(feature = "service-arg-validation")]
            {
                self.validate_service_args(&service)?;
            }
        }

        if circuit.get_circuit_management_type().is_empty() {
            return Err(AdminSharedError::ValidationFailed(
                "The circuit must have a mangement type".to_string(),
            ));
        }

        Ok(())
    }

    #[cfg(feature = "service-arg-validation")]
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
                to_hex(circuit_proposal.requester())
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

    fn check_approved(
        &self,
        proposal: &CircuitProposal,
    ) -> Result<CircuitProposalStatus, AdminSharedError> {
        let mut received_votes = HashSet::new();
        for vote in proposal.get_votes() {
            if vote.get_vote() == CircuitProposalVote_Vote::REJECT {
                return Ok(CircuitProposalStatus::Rejected);
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
            Ok(CircuitProposalStatus::Accepted)
        } else {
            Ok(CircuitProposalStatus::Pending)
        }
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

    pub fn get_circuits(
        &self,
    ) -> Result<Box<dyn ExactSizeIterator<Item = StoreCircuit>>, AdminSharedError> {
        self.admin_store
            .list_circuits(&Vec::new())
            .map_err(AdminSharedError::from)
    }

    pub fn get_nodes(&self) -> Result<BTreeMap<String, CircuitNode>, AdminSharedError> {
        Ok(self
            .admin_store
            .list_nodes()
            .map_err(AdminSharedError::from)?
            .into_iter()
            .map(|node| (node.node_id().into(), node))
            .collect())
    }

    fn verify_signature(&self, payload: &CircuitManagementPayload) -> Result<bool, ServiceError> {
        let header =
            protobuf::parse_from_bytes::<CircuitManagementPayload_Header>(payload.get_header())?;

        let signature = payload.get_signature().to_vec();
        let public_key = header.get_requester().to_vec();

        self.signature_verifier
            .verify(
                &payload.get_header(),
                &Signature::new(signature),
                &PublicKey::new(public_key),
            )
            .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use protobuf::{Message, RepeatedField};

    use diesel::{
        r2d2::{ConnectionManager as DieselConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    use crate::admin::service::AdminKeyVerifierError;
    use crate::admin::store::diesel::DieselAdminServiceStore;
    use crate::circuit::routing::memory::RoutingTable;
    use crate::keys::insecure::AllowAllKeyPermissionManager;
    use crate::mesh::{Envelope, Mesh};
    use crate::migrations::run_sqlite_migrations;
    use crate::network::auth::AuthorizationManager;
    use crate::network::connection_manager::authorizers::{Authorizers, InprocAuthorizer};
    use crate::network::connection_manager::ConnectionManager;
    use crate::peer::{PeerManager, PeerManagerConnector};
    use crate::protocol::authorization::{
        AuthorizationMessage, AuthorizationType, Authorized, ConnectRequest, ConnectResponse,
        TrustRequest,
    };
    use crate::protos::admin;
    use crate::protos::admin::{
        CircuitProposalVote_Vote, CircuitProposal_VoteRecord, SplinterNode, SplinterService,
    };
    use crate::protos::authorization;
    use crate::protos::network::{NetworkMessage, NetworkMessageType};
    use crate::protos::prelude::*;
    use crate::service::{ServiceMessageContext, ServiceSendError};
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
        let (orchestrator, _) = ServiceOrchestrator::new(vec![], orchestrator_connection, 1, 1, 1)
            .expect("failed to create orchestrator");
        let store = setup_admin_service_store();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut shared = AdminServiceShared::new(
            "my_peer_id".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

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

        // start up thread for other node
        std::thread::spawn(move || {
            let mesh = Mesh::new(2, 2);
            let conn = other_listener.accept().unwrap();
            mesh.add(conn, "my_peer_id".to_string()).unwrap();

            handle_auth(&mesh, "my_peer_id", "other-node");

            mesh.shutdown_signaler().shutdown();
        });

        shared
            .propose_circuit(payload, "test".to_string())
            .expect("Proposal not accepted");

        // None of the proposed members are peered
        assert_eq!(1, shared.unpeered_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set other-node to peered
        shared
            .on_peer_connected("other-node")
            .expect("Unable to set peer to peered");

        // Still waitin on 1 peer
        assert_eq!(1, shared.unpeered_payloads.len());
        assert_eq!(0, shared.pending_circuit_payloads.len());

        // Set other-node to peered
        shared
            .on_peer_connected("test-node")
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
        let (orchestrator, _) = ServiceOrchestrator::new(vec![], orchestrator_connection, 1, 1, 1)
            .expect("failed to create orchestrator");
        let store = setup_admin_service_store();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let mut shared = AdminServiceShared::new(
            "test-node".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

        let service_sender = MockServiceNetworkSender::new();
        shared.set_network_sender(Some(Box::new(service_sender.clone())));

        let mut circuit = admin::Circuit::new();
        circuit.set_circuit_id("01234-ABCDE".into());
        circuit.set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE);
        circuit.set_circuit_management_type("test app auth handler".into());
        circuit.set_comments("test circuit".into());

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
            .on_peer_connected("other-node")
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
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let circuit = setup_test_circuit();

        if let Err(err) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been valid: {}", err);
        }

        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that a circuit proposed with a key that is not permitted for the requesting node is
    // invalid
    fn test_validate_circuit_signer_not_permitted() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let circuit = setup_test_circuit();

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to requester node not being registered");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit is proposed by a signer key is not a valid public key the proposal is
    // invalid
    fn test_validate_circuit_signer_key_invalid() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let circuit = setup_test_circuit();

        let pub_key = (0u8..50).collect::<Vec<_>>();
        // too short
        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, &pub_key[0..10], "node_a") {
            panic!("Should have been invalid due to key being too short");
        }
        // too long
        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, &pub_key, "node_a") {
            panic!("Should have been invalid due to key being too long");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service in its roster with an allowed node that is not in
    // members an error is returned
    fn test_validate_circuit_bad_node() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut service_bad = SplinterService::new();
        service_bad.set_service_id("0123".to_string());
        service_bad.set_service_type("type_a".to_string());
        service_bad.set_allowed_nodes(RepeatedField::from_vec(vec!["node_bad".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_bad]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to service having an allowed node not in members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service in its roster with too many allowed nodes
    fn test_validate_circuit_too_many_allowed_nodes() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut service_bad = SplinterService::new();
        service_bad.set_service_id("0123".to_string());
        service_bad.set_service_type("type_a".to_string());
        service_bad.set_allowed_nodes(RepeatedField::from_vec(vec![
            "node_b".to_string(),
            "extra".to_string(),
        ]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_bad]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to service having too many allowed nodes");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with "" for a service id an error is returned
    fn test_validate_circuit_empty_service_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut service_ = SplinterService::new();
        service_.set_service_id("".to_string());
        service_.set_service_type("type_a".to_string());
        service_.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to service's id being empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with an invalid service id an error is returned
    fn test_validate_circuit_invalid_service_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut service_ = SplinterService::new();
        service_.set_service_id("invalid_service_id".to_string());
        service_.set_service_type("type_a".to_string());
        service_.set_allowed_nodes(RepeatedField::from_vec(vec!["node_a".to_string()]));

        circuit.set_roster(RepeatedField::from_vec(vec![service_]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to service's id being empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a service with duplicate service ids an error is returned
    fn test_validate_circuit_duplicate_service_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to service's id being a duplicate");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have any services in its roster an error is returned
    fn test_validate_circuit_empty_roster() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();
        circuit.set_roster(RepeatedField::from_vec(vec![]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid due to empty roster");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have any nodes in its members an error is returned
    fn test_validate_circuit_empty_members() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_members(RepeatedField::from_vec(vec![]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid empty members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have the local node in the member list an error is
    // returned
    fn test_validate_circuit_missing_local_node() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_b:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because node_a is not in members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with node id of "" an error is
    // returned
    fn test_validate_circuit_empty_node_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because node_ is has an empty node id");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has duplicate members an error is returned
    fn test_validate_circuit_duplicate_members() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because there are duplicate members");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has an empty circuit id an error is returned
    fn test_validate_circuit_empty_circuit_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_id("".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit ID is empty");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has an invalid circuit id an error is returned
    fn test_validate_circuit_invalid_circuit_id() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_id("invalid_circuit_id".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because the circuit ID is invalid");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with no endpoints an error is returned
    fn test_validate_circuit_no_endpoints() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec![].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because a member has no endpoints");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with an empty endpoint an error is returned
    fn test_validate_circuit_empty_endpoint() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because a member has an empty endpoint");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit has a member with a duplicate endpoint an error is returned
    fn test_validate_circuit_duplicate_endpoint() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        let mut node_a = SplinterNode::new();
        node_a.set_node_id("node_a".to_string());
        node_a.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        let mut node_b = SplinterNode::new();
        node_b.set_node_id("node_b".to_string());
        node_b.set_endpoints(vec!["test://endpoint_a:0".to_string()].into());

        circuit.set_members(RepeatedField::from_vec(vec![node_a, node_b]));

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because a member has a duplicate endpoint");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have authorization set an error is returned
    fn test_validate_circuit_no_authorization() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_authorization_type(Circuit_AuthorizationType::UNSET_AUTHORIZATION_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because authorizaiton type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have persistence set an error is returned
    fn test_validate_circuit_no_persitance() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_persistence(Circuit_PersistenceType::UNSET_PERSISTENCE_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because persistence type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have durability set an error is returned
    fn test_validate_circuit_unset_durability() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_durability(Circuit_DurabilityType::UNSET_DURABILITY_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because durabilty type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have route type set an error is returned
    fn test_validate_circuit_no_routes() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_routes(Circuit_RouteType::UNSET_ROUTE_TYPE);

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because route type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if a circuit does not have circuit_management_type set an error is returned
    fn test_validate_circuit_no_management_type() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
        let mut circuit = setup_test_circuit();

        circuit.set_circuit_management_type("".to_string());

        if let Ok(_) = admin_shared.validate_create_circuit(&circuit, PUB_KEY, "node_a") {
            panic!("Should have been invalid because route type is unset");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that a valid circuit proposal vote comes back as valid
    fn test_validate_proposal_vote_valid() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::new(false)),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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
            panic!("Should have been invalid because node as already submited a vote");
        }
        shutdown(mesh, cm, pm);
    }

    #[test]
    // test that if the circuit hash in the circuit proposal does not match the circuit hash on
    // the vote, the vote is invalid
    fn test_validate_proposal_vote_circuit_hash_mismatch() {
        let store = setup_admin_service_store();
        let (mesh, cm, pm, peer_connector) = setup_peer_connector(None);
        let orchestrator = setup_orchestrator();

        let signature_verifier = Secp256k1Context::new().new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

        let admin_shared = AdminServiceShared::new(
            "node_a".into(),
            Arc::new(Mutex::new(orchestrator)),
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();
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
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

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
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

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
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

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
            #[cfg(feature = "service-arg-validation")]
            HashMap::new(),
            peer_connector,
            store,
            signature_verifier,
            Box::new(MockAdminKeyVerifier::default()),
            Box::new(AllowAllKeyPermissionManager),
            writer,
        )
        .unwrap();

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
        circuit.set_roster(RepeatedField::from_vec(vec![service_b, service_a]));
        circuit.set_authorization_type(Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        circuit.set_persistence(Circuit_PersistenceType::ANY_PERSISTENCE);
        circuit.set_durability(Circuit_DurabilityType::NO_DURABILITY);
        circuit.set_routes(Circuit_RouteType::ANY_ROUTE);
        circuit.set_circuit_management_type("test_circuit".to_string());
        circuit.set_application_metadata(b"test_data".to_vec());
        circuit.set_comments("test circuit".to_string());

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
        let inproc_authorizer = InprocAuthorizer::new(vec![
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
        ]);

        let authorization_manager = AuthorizationManager::new("test-node".into())
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

    fn shutdown(mesh: Mesh, cm: ConnectionManager, pm: PeerManager) {
        pm.shutdown_signaler().shutdown();
        cm.shutdown_signaler().shutdown();
        pm.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
    }

    fn setup_orchestrator() -> ServiceOrchestrator {
        let mut transport =
            MockConnectingTransport::expect_connections(vec![Ok(Box::new(MockConnection::new()))]);
        let orchestrator_connection = transport
            .connect("inproc://orchestator-service")
            .expect("failed to create connection");
        let (orchestrator, _) = ServiceOrchestrator::new(vec![], orchestrator_connection, 1, 1, 1)
            .expect("failed to create orchestrator");
        orchestrator
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
        service
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
        let mut msg = NetworkMessage::new();
        msg.set_message_type(NetworkMessageType::AUTHORIZATION);
        msg.set_payload(
            IntoBytes::<authorization::AuthorizationMessage>::into_bytes(auth_msg)
                .expect("Unable to convert into bytes"),
        );

        Envelope::new(
            connection_id.to_string(),
            msg.write_to_bytes().expect("Unable to write to bytes"),
        )
    }
}
