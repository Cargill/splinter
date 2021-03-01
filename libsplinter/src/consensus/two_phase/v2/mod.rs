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

//! Version 2 of the two-phase commit (2PC) consensus algorithm
//!
//! This is a bully algorithm where the coordinator for a proposal is determined as the node with
//! the lowest ID in the set of verifiers (peers). Only one proposal is considered at a time, and
//! only the coordinator creates new proposals.
//!
//! # Known limitations of this 2PC implementation
//!
//! A limitation of this implementation is that it is not fully resilient to crashes; for instance,
//! if the coordinator commits a proposal but crashes before it is able to send the `APPLY` message
//! to the other nodes, the network will be out of sync because the coordinator does not know to
//! send the message when it restarts. This limitation will be solved by re-implementing 2PC as a
//! stateless algorithm.
//!
//! # Differences from previous version
//!
//! This version of the 2PC implementation differs from the previous version in the following ways:
//!
//! * A custom list of verifiers may no longer be provided by the proposal manager; the verifiers
//!   for each proposal will be the list of all peers + the local node. This means that there is
//!   only one coordinator for all proposals, which solves one of the known limitations of version
//!   1.
//! * Only the coordinator creates new proposals. Because the coordinator determines the order in
//!   which proposals are evaluated and is responsible for determining when to accept them, it is
//!   the only node that can reliably produce proposals that are based on the most current state.

mod timing;

use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use protobuf::Message;

use crate::consensus::{
    ConsensusEngine, ConsensusEngineError, ConsensusMessage, ConsensusNetworkSender, PeerId,
    ProposalId, ProposalManager, ProposalUpdate, StartupState,
};
use crate::protos::two_phase::{
    TwoPhaseMessage, TwoPhaseMessage_ProposalResult, TwoPhaseMessage_ProposalVerificationResponse,
    TwoPhaseMessage_Type,
};

use self::timing::Timeout;

const MESSAGE_RECV_TIMEOUT_MILLIS: u64 = 100;
const PROPOSAL_RECV_TIMEOUT_MILLIS: u64 = 100;

#[derive(Debug)]
enum State {
    Idle,
    AwaitingProposal,
    EvaluatingProposal(TwoPhaseProposal),
}

impl State {
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_awaiting_proposal(&self) -> bool {
        matches!(self, Self::AwaitingProposal)
    }

    pub fn is_evaluating_proposal_with_id(&self, proposal_id: &ProposalId) -> bool {
        matches!(
            self,
            Self::EvaluatingProposal(tpc_proposal) if tpc_proposal.proposal_id() == proposal_id,
        )
    }
}

/// Contains information about a proposal that two phase consensus needs to keep track of
#[derive(Debug)]
struct TwoPhaseProposal {
    proposal_id: ProposalId,
    peers_verified: HashSet<PeerId>,
}

impl TwoPhaseProposal {
    fn new(proposal_id: ProposalId) -> Self {
        TwoPhaseProposal {
            proposal_id,
            peers_verified: HashSet::new(),
        }
    }

    fn proposal_id(&self) -> &ProposalId {
        &self.proposal_id
    }

    fn peers_verified(&self) -> &HashSet<PeerId> {
        &self.peers_verified
    }

    fn add_verified_peer(&mut self, id: PeerId) {
        self.peers_verified.insert(id);
    }
}

pub struct TwoPhaseEngine {
    id: PeerId,
    verifiers: HashSet<PeerId>,
    state: State,
    coordinator_timeout: Timeout,
    proposals_received: HashSet<ProposalId>,
    verification_request_backlog: VecDeque<ProposalId>,
}

impl TwoPhaseEngine {
    pub fn new(coordinator_timeout_duration: Duration) -> Self {
        TwoPhaseEngine {
            id: PeerId::default(),
            verifiers: HashSet::new(),
            state: State::Idle,
            coordinator_timeout: Timeout::new(coordinator_timeout_duration),
            proposals_received: HashSet::new(),
            verification_request_backlog: VecDeque::new(),
        }
    }

    /// Determines if this node is the coordinator.
    fn is_coordinator(&self) -> bool {
        &self.id == self.coordinator_id()
    }

    /// Gets the ID of the coordinator. The coordinator is the node with the lowest ID in the set of
    /// verifiers.
    fn coordinator_id(&self) -> &PeerId {
        self.verifiers
            .iter()
            .min()
            .expect("2PC always has at least one verifier (self)")
    }

    fn handle_consensus_msg(
        &mut self,
        consensus_msg: ConsensusMessage,
        network_sender: &dyn ConsensusNetworkSender,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        let two_phase_msg: TwoPhaseMessage = Message::parse_from_bytes(&consensus_msg.message)?;
        let proposal_id = ProposalId::from(two_phase_msg.get_proposal_id());

        match two_phase_msg.get_message_type() {
            TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST => {
                debug!("Proposal verification request received: {}", proposal_id);

                if self.state.is_evaluating_proposal_with_id(&proposal_id) {
                    debug!(
                        "Proposal already in progress, backlogging verification request: {}",
                        proposal_id
                    );
                    self.verification_request_backlog.push_back(proposal_id);
                } else {
                    // Try to get the proposal from the backlog
                    if self.proposals_received.remove(&proposal_id) {
                        debug!("Checking proposal {}", proposal_id);
                        proposal_manager.check_proposal(&proposal_id)?;
                        self.state = State::EvaluatingProposal(TwoPhaseProposal::new(proposal_id));
                    } else {
                        debug!(
                            "Proposal not yet received, backlogging verification request: \
                             {}",
                            proposal_id
                        );
                        self.verification_request_backlog.push_back(proposal_id);
                    }
                }
            }
            TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE => {
                if !self.state.is_evaluating_proposal_with_id(&proposal_id) {
                    warn!(
                        "Received unexpected verification response for proposal {}",
                        proposal_id
                    );
                    return Ok(());
                }

                match two_phase_msg.get_proposal_verification_response() {
                    TwoPhaseMessage_ProposalVerificationResponse::VERIFIED => {
                        debug!(
                            "Proposal {} verified by peer {}",
                            proposal_id, consensus_msg.origin_id
                        );
                        // Already checked state above in self.state.is_evaluating_proposal_with_id
                        if let State::EvaluatingProposal(tpc_proposal) = &mut self.state {
                            tpc_proposal.add_verified_peer(consensus_msg.origin_id);

                            if tpc_proposal.peers_verified() == &self.verifiers {
                                debug!(
                                    "All verifiers have approved; accepting proposal {}",
                                    proposal_id
                                );
                                self.complete_coordination(
                                    proposal_id,
                                    TwoPhaseMessage_ProposalResult::APPLY,
                                    network_sender,
                                    proposal_manager,
                                )?;
                            }
                        }
                    }
                    TwoPhaseMessage_ProposalVerificationResponse::FAILED => {
                        debug!(
                            "Proposal failed by peer {}; rejecting proposal {}",
                            consensus_msg.origin_id, proposal_id
                        );
                        self.complete_coordination(
                            proposal_id,
                            TwoPhaseMessage_ProposalResult::REJECT,
                            network_sender,
                            proposal_manager,
                        )?;
                    }
                    TwoPhaseMessage_ProposalVerificationResponse::UNSET_VERIFICATION_RESPONSE => {
                        warn!(
                            "Ignoring improperly specified proposal verification response from {}",
                            consensus_msg.origin_id
                        )
                    }
                }
            }
            TwoPhaseMessage_Type::PROPOSAL_RESULT => match two_phase_msg.get_proposal_result() {
                TwoPhaseMessage_ProposalResult::APPLY => {
                    if self.state.is_evaluating_proposal_with_id(&proposal_id) {
                        debug!("Accepting proposal {}", proposal_id);
                        proposal_manager.accept_proposal(&proposal_id, None)?;
                        self.state = State::Idle;
                    } else {
                        warn!(
                            "Received unexpected apply result for proposal {}",
                            proposal_id
                        );
                    }
                }
                TwoPhaseMessage_ProposalResult::REJECT => {
                    debug!("Rejecting proposal {}", proposal_id);
                    proposal_manager.reject_proposal(&proposal_id)?;

                    // Only update state if this was the currently evaluating proposal
                    if self.state.is_evaluating_proposal_with_id(&proposal_id) {
                        self.state = State::Idle;
                    }
                }
                TwoPhaseMessage_ProposalResult::UNSET_RESULT => warn!(
                    "Ignoring improperly specified proposal result from {}",
                    consensus_msg.origin_id
                ),
            },
            TwoPhaseMessage_Type::UNSET_TYPE => warn!(
                "Ignoring improperly specified two-phase message from {}",
                consensus_msg.origin_id
            ),
        }

        Ok(())
    }

    fn handle_proposal_update(
        &mut self,
        update: ProposalUpdate,
        network_sender: &dyn ConsensusNetworkSender,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        let is_coordinator = self.is_coordinator();
        match update {
            ProposalUpdate::ProposalCreated(_) if !self.is_coordinator() => {
                warn!("Received ProposalCreated message, but this node is not the coordinator");
            }
            ProposalUpdate::ProposalCreated(_) if !self.state.is_awaiting_proposal() => {
                warn!("Received unexpected ProposalCreated message");
            }
            ProposalUpdate::ProposalCreated(None) => {
                if self.state.is_awaiting_proposal() {
                    self.state = State::Idle;
                }
            }
            ProposalUpdate::ProposalCreated(Some(proposal)) => {
                debug!("Proposal created, starting coordination: {}", proposal.id);
                self.start_coordination(proposal.id, network_sender, proposal_manager)?;
            }
            ProposalUpdate::ProposalReceived(_, peer_id) if &peer_id != self.coordinator_id() => {
                warn!(
                    "Received proposal from a node that is not the coordinator: {}",
                    peer_id
                );
            }
            ProposalUpdate::ProposalReceived(proposal, _) => {
                debug!("Proposal received: {}", proposal.id);
                self.proposals_received.insert(proposal.id);
            }
            ProposalUpdate::ProposalValid(proposal_id) => match &mut self.state {
                State::EvaluatingProposal(tpc_proposal)
                    if tpc_proposal.proposal_id() == &proposal_id =>
                {
                    debug!("Proposal valid: {}", proposal_id);

                    if is_coordinator {
                        tpc_proposal.add_verified_peer(self.id.clone());

                        debug!("Requesting verification of proposal {}", proposal_id);

                        let mut request = TwoPhaseMessage::new();
                        request
                            .set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST);
                        request.set_proposal_id(proposal_id.into());

                        network_sender.broadcast(request.write_to_bytes()?)?;
                    } else {
                        debug!("Sending verified response for proposal {}", proposal_id);

                        let mut response = TwoPhaseMessage::new();
                        response
                            .set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE);
                        response.set_proposal_id(proposal_id.into());
                        response.set_proposal_verification_response(
                            TwoPhaseMessage_ProposalVerificationResponse::VERIFIED,
                        );

                        network_sender
                            .send_to(self.coordinator_id(), response.write_to_bytes()?)?;
                    }
                }
                _ => warn!("Got valid message for unknown proposal: {}", proposal_id),
            },
            ProposalUpdate::ProposalInvalid(proposal_id) => {
                if self.state.is_evaluating_proposal_with_id(&proposal_id) {
                    debug!("Proposal invalid: {}", proposal_id);

                    if is_coordinator {
                        debug!("Rejecting proposal {}", proposal_id);
                        self.complete_coordination(
                            proposal_id,
                            TwoPhaseMessage_ProposalResult::REJECT,
                            network_sender,
                            proposal_manager,
                        )?;
                    } else {
                        debug!("Sending failed response for proposal {}", proposal_id);

                        let mut response = TwoPhaseMessage::new();
                        response
                            .set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE);
                        response.set_proposal_id(proposal_id.into());
                        response.set_proposal_verification_response(
                            TwoPhaseMessage_ProposalVerificationResponse::FAILED,
                        );

                        network_sender
                            .send_to(self.coordinator_id(), response.write_to_bytes()?)?;
                    }
                } else {
                    warn!("Got invalid message for unknown proposal: {}", proposal_id);
                }
            }
            ProposalUpdate::ProposalAccepted(proposal_id) => {
                info!("proposal accepted: {}", proposal_id);
            }
            ProposalUpdate::ProposalAcceptFailed(proposal_id, err) => {
                error!(
                    "failed to accept proposal {} due to error: {}",
                    proposal_id, err
                );
            }
            other => {
                debug!("ignoring update: {:?}", other);
            }
        }

        Ok(())
    }

    fn start_coordination(
        &mut self,
        proposal_id: ProposalId,
        network_sender: &dyn ConsensusNetworkSender,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        debug!("Checking proposal {}", proposal_id);
        match proposal_manager.check_proposal(&proposal_id) {
            Ok(_) => {
                self.state = State::EvaluatingProposal(TwoPhaseProposal::new(proposal_id));
                self.coordinator_timeout.start();
            }
            Err(err) => {
                debug!(
                    "Rejecting proposal {}; failed to check proposal due to err: {}",
                    proposal_id, err
                );
                self.complete_coordination(
                    proposal_id,
                    TwoPhaseMessage_ProposalResult::REJECT,
                    network_sender,
                    proposal_manager,
                )?;
            }
        }
        Ok(())
    }

    fn complete_coordination(
        &mut self,
        proposal_id: ProposalId,
        proposal_result: TwoPhaseMessage_ProposalResult,
        network_sender: &dyn ConsensusNetworkSender,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        match proposal_result {
            TwoPhaseMessage_ProposalResult::APPLY => {
                proposal_manager.accept_proposal(&proposal_id, None)?;
            }
            TwoPhaseMessage_ProposalResult::REJECT => {
                proposal_manager.reject_proposal(&proposal_id)?;
            }
            TwoPhaseMessage_ProposalResult::UNSET_RESULT => {
                warn!(
                    "Unset proposal result when completing proposal {}",
                    proposal_id
                );
                return Ok(());
            }
        }

        self.state = State::Idle;
        self.coordinator_timeout.stop();

        let mut result = TwoPhaseMessage::new();
        result.set_message_type(TwoPhaseMessage_Type::PROPOSAL_RESULT);
        result.set_proposal_id(proposal_id.into());
        result.set_proposal_result(proposal_result);

        network_sender.broadcast(result.write_to_bytes()?)?;

        Ok(())
    }

    /// If the coordinator timeout has expired, abort the current proposal.
    fn abort_proposal_if_timed_out(
        &mut self,
        network_sender: &dyn ConsensusNetworkSender,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        if let State::EvaluatingProposal(ref tpc_proposal) = self.state {
            if self.coordinator_timeout.check_expired() {
                warn!(
                    "Proposal timed out; rejecting: {}",
                    tpc_proposal.proposal_id()
                );
                let proposal_id = tpc_proposal.proposal_id().clone();
                self.complete_coordination(
                    proposal_id,
                    TwoPhaseMessage_ProposalResult::REJECT,
                    network_sender,
                    proposal_manager,
                )?;
            }
        }

        Ok(())
    }

    /// If not doing anything, see if there are any backlogged verification requests that this node
    /// has received a proposal for, and evaluate that proposal.
    fn handle_backlogged_verification_request(
        &mut self,
        proposal_manager: &dyn ProposalManager,
    ) -> Result<(), ConsensusEngineError> {
        if self.state.is_idle() {
            if let Some(idx) = self
                .verification_request_backlog
                .iter()
                .position(|proposal_id| self.proposals_received.contains(proposal_id))
            {
                let proposal_id = self.verification_request_backlog.remove(idx).unwrap();
                self.proposals_received.remove(&proposal_id);

                debug!("Checking proposal from backlog: {}", proposal_id);
                proposal_manager.check_proposal(&proposal_id)?;
                self.state = State::EvaluatingProposal(TwoPhaseProposal::new(proposal_id));
            }
        }

        Ok(())
    }

    /// If this node is the coordinator and it's not doing anything, try to get the next proposal.
    fn get_next_proposal(&mut self, proposal_manager: &dyn ProposalManager) {
        if self.is_coordinator() && self.state.is_idle() {
            match proposal_manager.create_proposal(None, vec![]) {
                Ok(()) => self.state = State::AwaitingProposal,
                Err(err) => error!("Error while creating proposal: {}", err),
            }
        }
    }
}

impl ConsensusEngine for TwoPhaseEngine {
    fn name(&self) -> &str {
        "two-phase"
    }

    fn version(&self) -> &str {
        "0.1"
    }

    fn additional_protocols(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn run(
        &mut self,
        consensus_messages: Receiver<ConsensusMessage>,
        proposal_updates: Receiver<ProposalUpdate>,
        network_sender: Box<dyn ConsensusNetworkSender>,
        proposal_manager: Box<dyn ProposalManager>,
        startup_state: StartupState,
    ) -> Result<(), ConsensusEngineError> {
        let message_timeout = Duration::from_millis(MESSAGE_RECV_TIMEOUT_MILLIS);
        let proposal_timeout = Duration::from_millis(PROPOSAL_RECV_TIMEOUT_MILLIS);

        self.id = startup_state.id;
        self.verifiers.insert(self.id.clone()); // This node is a verifier

        for id in startup_state.peer_ids {
            self.verifiers.insert(id);
        }

        loop {
            if let Err(err) = self.abort_proposal_if_timed_out(&*network_sender, &*proposal_manager)
            {
                error!("Failed to abort timed-out proposal: {}", err);
            }

            if let Err(err) = self.handle_backlogged_verification_request(&*proposal_manager) {
                error!("Failed to handle backlogged verification request: {}", err);
            }

            self.get_next_proposal(&*proposal_manager);

            // Get and handle a consensus message if there is one
            match consensus_messages.recv_timeout(message_timeout) {
                Ok(consensus_message) => {
                    if let Err(err) = self.handle_consensus_msg(
                        consensus_message,
                        &*network_sender,
                        &*proposal_manager,
                    ) {
                        error!("error while handling consensus message: {}", err);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    info!("consensus message receiver disconnected");
                    break;
                }
            }

            // Get and handle a proposal update if there is one
            match proposal_updates.recv_timeout(proposal_timeout) {
                Ok(ProposalUpdate::Shutdown) => {
                    info!("received shutdown");
                    break;
                }
                Ok(update) => {
                    if let Err(err) =
                        self.handle_proposal_update(update, &*network_sender, &*proposal_manager)
                    {
                        error!("error while handling proposal update: {}", err);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    info!("proposal update receiver disconnected");
                    break;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::iter::FromIterator;
    use std::sync::mpsc::channel;

    use crate::consensus::tests::{MockConsensusNetworkSender, MockProposalManager};
    use crate::consensus::Proposal;

    const COORDINATOR_TIMEOUT_MILLIS: u64 = 5000;

    /// Verify that the engine properly shuts down when it receives the Shutdown update.
    #[test]
    fn test_shutdown() {
        let (update_tx, update_rx) = channel();
        let (_, consensus_msg_rx) = channel();

        let manager = MockProposalManager::new(update_tx.clone());
        let network = MockConsensusNetworkSender::new();
        let startup_state = StartupState {
            id: vec![0].into(),
            peer_ids: vec![vec![1].into()],
            last_proposal: None,
        };

        let mut engine = TwoPhaseEngine::new(Duration::from_millis(COORDINATOR_TIMEOUT_MILLIS));
        let thread = std::thread::spawn(move || {
            engine
                .run(
                    consensus_msg_rx,
                    update_rx,
                    Box::new(network),
                    Box::new(manager),
                    startup_state,
                )
                .expect("engine failed")
        });

        update_tx
            .send(ProposalUpdate::Shutdown)
            .expect("failed to send shutdown");
        thread.join().expect("failed to join engine thread");
    }

    /// Verify the `coordinator_id` and `is_coordinator` methods work correctly.
    #[test]
    fn test_coordinator_check() {
        let peer_ids: Vec<PeerId> = vec![vec![0].into(), vec![1].into(), vec![2].into()];
        let peer_ids_hashset = HashSet::from_iter(peer_ids.iter().cloned());

        let coordinator = TwoPhaseEngine {
            id: peer_ids[0].clone(),
            verifiers: peer_ids_hashset.clone(),
            state: State::Idle,
            coordinator_timeout: Timeout::new(Duration::from_millis(COORDINATOR_TIMEOUT_MILLIS)),
            proposals_received: HashSet::new(),
            verification_request_backlog: VecDeque::new(),
        };
        assert_eq!(coordinator.coordinator_id(), &peer_ids[0]);
        assert!(coordinator.is_coordinator());

        let other_node = TwoPhaseEngine {
            id: peer_ids[1].clone(),
            verifiers: peer_ids_hashset,
            state: State::Idle,
            coordinator_timeout: Timeout::new(Duration::from_millis(COORDINATOR_TIMEOUT_MILLIS)),
            proposals_received: HashSet::new(),
            verification_request_backlog: VecDeque::new(),
        };
        assert_eq!(other_node.coordinator_id(), &peer_ids[0]);
        assert!(!other_node.is_coordinator());
    }

    /// Test the coordinator (leader) of a 3 node network by simulating the flow of a valid
    /// proposal (both participants verify the proposal) and a failed proposal (one participant
    /// fails the proposal).
    #[test]
    fn test_coordinator() {
        let (update_tx, update_rx) = channel();
        let (consensus_msg_tx, consensus_msg_rx) = channel();

        let manager = MockProposalManager::new(update_tx.clone());
        let network = MockConsensusNetworkSender::new();
        let startup_state = StartupState {
            id: vec![0].into(),
            peer_ids: vec![vec![1].into(), vec![2].into()],
            last_proposal: None,
        };

        let mut engine = TwoPhaseEngine::new(Duration::from_millis(COORDINATOR_TIMEOUT_MILLIS));
        let network_clone = network.clone();
        let manager_clone = manager.clone();
        let thread = std::thread::spawn(move || {
            engine
                .run(
                    consensus_msg_rx,
                    update_rx,
                    Box::new(network_clone),
                    Box::new(manager_clone),
                    startup_state,
                )
                .expect("engine failed")
        });

        // Check that verification request is sent for the first proposal
        loop {
            if let Some(msg) = network.broadcast_messages().get(0) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST
                );
                assert_eq!(msg.get_proposal_id(), vec![1].as_slice());
                break;
            }
        }

        // Receive the verification responses
        let mut response = TwoPhaseMessage::new();
        response.set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE);
        response.set_proposal_id(vec![1]);
        response.set_proposal_verification_response(
            TwoPhaseMessage_ProposalVerificationResponse::VERIFIED,
        );
        let message_bytes = response
            .write_to_bytes()
            .expect("failed to write failed response to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes.clone(), vec![1].into()))
            .expect("failed to send 1st response");
        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![2].into()))
            .expect("failed to send 2nd response");

        // Verify the Apply message is sent for the proposal
        loop {
            if let Some(msg) = network.broadcast_messages().get(1) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_RESULT
                );
                assert_eq!(
                    msg.get_proposal_result(),
                    TwoPhaseMessage_ProposalResult::APPLY
                );
                assert_eq!(msg.get_proposal_id(), vec![1].as_slice());
                break;
            }
        }

        // Verify the proposal was accepted
        loop {
            if let Some((id, _)) = manager.accepted_proposals().get(0) {
                assert_eq!(id, &vec![1].into());
                break;
            }
        }

        // Check that verification request is sent for the second proposal
        loop {
            if let Some(msg) = network.broadcast_messages().get(2) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST
                );
                assert_eq!(msg.get_proposal_id(), vec![2].as_slice());
                break;
            }
        }

        // Receive the verification responses
        let mut response = TwoPhaseMessage::new();
        response.set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE);
        response.set_proposal_id(vec![2]);
        response.set_proposal_verification_response(
            TwoPhaseMessage_ProposalVerificationResponse::VERIFIED,
        );
        let message_bytes = response
            .write_to_bytes()
            .expect("failed to write failed response to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![1].into()))
            .expect("failed to send 1st response");

        let mut response = TwoPhaseMessage::new();
        response.set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE);
        response.set_proposal_id(vec![2]);
        response.set_proposal_verification_response(
            TwoPhaseMessage_ProposalVerificationResponse::FAILED,
        );
        let message_bytes = response
            .write_to_bytes()
            .expect("failed to write failed response to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![2].into()))
            .expect("failed to send 2nd response");

        // Verify the Reject message is sent for the proposal
        loop {
            if let Some(msg) = network.broadcast_messages().get(3) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_RESULT
                );
                assert_eq!(
                    msg.get_proposal_result(),
                    TwoPhaseMessage_ProposalResult::REJECT
                );
                assert_eq!(msg.get_proposal_id(), vec![2].as_slice());
                break;
            }
        }

        // Verify the proposal was rejected
        loop {
            if let Some(id) = manager.rejected_proposals().get(0) {
                assert_eq!(id, &vec![2].into());
                break;
            }
        }

        update_tx
            .send(ProposalUpdate::Shutdown)
            .expect("failed to send shutdown");
        thread.join().expect("failed to join engine thread");
    }

    /// Test a participant (follower) by simulating the flow of a valid and a failed proposal.
    #[test]
    fn test_participant() {
        let (update_tx, update_rx) = channel();
        let (consensus_msg_tx, consensus_msg_rx) = channel();

        let manager = MockProposalManager::new(update_tx.clone());
        manager.set_return_proposal(false);
        let network = MockConsensusNetworkSender::new();
        let startup_state = StartupState {
            id: vec![1].into(),
            peer_ids: vec![vec![0].into()],
            last_proposal: None,
        };

        let mut engine = TwoPhaseEngine::new(Duration::from_millis(COORDINATOR_TIMEOUT_MILLIS));
        let network_clone = network.clone();
        let manager_clone = manager.clone();
        let thread = std::thread::spawn(move || {
            engine
                .run(
                    consensus_msg_rx,
                    update_rx,
                    Box::new(network_clone),
                    Box::new(manager_clone),
                    startup_state,
                )
                .expect("engine failed")
        });

        // Receive the first proposal
        let mut proposal = Proposal::default();
        proposal.id = vec![1].into();
        update_tx
            .send(ProposalUpdate::ProposalReceived(proposal, vec![0].into()))
            .expect("failed to send 1st proposal");

        // Receive the first verification request
        let mut request = TwoPhaseMessage::new();
        request.set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST);
        request.set_proposal_id(vec![1]);
        let message_bytes = request
            .write_to_bytes()
            .expect("failed to write request to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![0].into()))
            .expect("failed to send 1st verification request");

        // Check that the Verified verification response is sent
        loop {
            if let Some((msg, peer_id)) = network.sent_messages().get(0) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(peer_id, &vec![0].into());
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE
                );
                assert_eq!(
                    msg.get_proposal_verification_response(),
                    TwoPhaseMessage_ProposalVerificationResponse::VERIFIED
                );
                assert_eq!(msg.get_proposal_id(), vec![1].as_slice());
                break;
            }
        }

        // Receive the Apply result
        let mut result = TwoPhaseMessage::new();
        result.set_message_type(TwoPhaseMessage_Type::PROPOSAL_RESULT);
        result.set_proposal_id(vec![1]);
        result.set_proposal_result(TwoPhaseMessage_ProposalResult::APPLY);
        let message_bytes = result
            .write_to_bytes()
            .expect("failed to write apply result to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![0].into()))
            .expect("failed to send apply result");

        // Verify the proposal was accepted
        loop {
            if let Some((id, _)) = manager.accepted_proposals().get(0) {
                assert_eq!(id, &vec![1].into());
                break;
            }
        }

        // Receive the second proposal
        let mut proposal = Proposal::default();
        proposal.id = vec![2].into();
        update_tx
            .send(ProposalUpdate::ProposalReceived(proposal, vec![0].into()))
            .expect("failed to send 2nd proposal");

        // Receive the second verification request (the manager will say this proposal is invalid)
        manager.set_next_proposal_valid(false);

        let mut request = TwoPhaseMessage::new();
        request.set_message_type(TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST);
        request.set_proposal_id(vec![2]);
        let message_bytes = request
            .write_to_bytes()
            .expect("failed to write request to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![0].into()))
            .expect("failed to send 2nd verification request");

        // Check that the Failed verification response is sent
        loop {
            if let Some((msg, peer_id)) = network.sent_messages().get(1) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(peer_id, &vec![0].into());
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_RESPONSE
                );
                assert_eq!(
                    msg.get_proposal_verification_response(),
                    TwoPhaseMessage_ProposalVerificationResponse::FAILED
                );
                assert_eq!(msg.get_proposal_id(), vec![2].as_slice());
                break;
            }
        }

        // Receive the Reject result
        let mut result = TwoPhaseMessage::new();
        result.set_message_type(TwoPhaseMessage_Type::PROPOSAL_RESULT);
        result.set_proposal_id(vec![2]);
        result.set_proposal_result(TwoPhaseMessage_ProposalResult::REJECT);
        let message_bytes = result
            .write_to_bytes()
            .expect("failed to write reject result to bytes");

        consensus_msg_tx
            .send(ConsensusMessage::new(message_bytes, vec![0].into()))
            .expect("failed to send reject result");

        // Verify the proposal was rejected
        loop {
            if let Some(id) = manager.rejected_proposals().get(0) {
                assert_eq!(id, &vec![2].into());
                break;
            }
        }

        update_tx
            .send(ProposalUpdate::Shutdown)
            .expect("failed to send shutdown");
        thread.join().expect("failed to join engine thread");
    }

    /// Test that the coordinator will abort a commit if the coordinator timeout expires while
    /// evaluating the commit.
    #[test]
    fn test_coordinator_timeout() {
        let (update_tx, update_rx) = channel();
        let (_consensus_msg_tx, consensus_msg_rx) = channel();

        let manager = MockProposalManager::new(update_tx.clone());
        let network = MockConsensusNetworkSender::new();
        let startup_state = StartupState {
            id: vec![0].into(),
            peer_ids: vec![vec![1].into(), vec![2].into()],
            last_proposal: None,
        };

        // Start engine with a very short coordinator timeout
        let mut engine = TwoPhaseEngine::new(Duration::from_millis(10));
        let network_clone = network.clone();
        let manager_clone = manager.clone();
        let thread = std::thread::spawn(move || {
            engine
                .run(
                    consensus_msg_rx,
                    update_rx,
                    Box::new(network_clone),
                    Box::new(manager_clone),
                    startup_state,
                )
                .expect("engine failed")
        });

        // Check that a proposal verification request is sent
        loop {
            if let Some(msg) = network.broadcast_messages().get(0) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_VERIFICATION_REQUEST
                );
                assert_eq!(msg.get_proposal_id(), vec![1].as_slice());
                break;
            }
        }

        // Verify the Reject message is sent for the proposal (due to the timeout)
        loop {
            if let Some(msg) = network.broadcast_messages().get(1) {
                let msg: TwoPhaseMessage =
                    Message::parse_from_bytes(msg).expect("failed to parse message");
                assert_eq!(
                    msg.get_message_type(),
                    TwoPhaseMessage_Type::PROPOSAL_RESULT
                );
                assert_eq!(
                    msg.get_proposal_result(),
                    TwoPhaseMessage_ProposalResult::REJECT
                );
                assert_eq!(msg.get_proposal_id(), vec![1].as_slice());
                break;
            }
        }

        // Verify the proposal was rejected
        loop {
            if let Some(id) = manager.rejected_proposals().get(0) {
                assert_eq!(id, &vec![1].into());
                break;
            }
        }

        update_tx
            .send(ProposalUpdate::Shutdown)
            .expect("failed to send shutdown");
        thread.join().expect("failed to join engine thread");
    }
}
