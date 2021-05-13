// Copyright 2018-2021 Cargill Incorporated
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Websocket-backed implementation of the AdminServiceEventClient.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError, TrySendError};
use std::sync::Arc;

use crate::admin::client::event::{
    AdminServiceEvent, AdminServiceEventClient, EventType, NextEventError, PublicKey,
};
use crate::admin::client::{
    CircuitMembers, CircuitService, ProposalCircuitSlice, ProposalSlice, VoteRecord,
};
use crate::admin::messages;
use crate::error::{InternalError, InvalidStateError};
use crate::events::{
    Igniter, ParseBytes, ParseError, Reactor, WebSocketClient, WebSocketError, WsResponse,
};
use crate::hex;
use crate::threading::lifecycle::ShutdownHandle;

enum WsRuntime {
    Reactor(Option<Reactor>),
    Igniter(Igniter),
}

/// Constructs a new AwcAdminServiceEventClient.
#[derive(Default)]
pub struct AwcAdminServiceEventClientBuilder {
    ws_runtime: Option<WsRuntime>,
    root_url: Option<String>,
    event_type: Option<String>,
    authorization: Option<String>,
    last_event_id: Option<u64>,
}

impl AwcAdminServiceEventClientBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the event reactor to use with this client instance.
    ///
    /// This enables multiple clients to be created on the same reactor.
    pub fn with_reactor(mut self, reactor: &Reactor) -> Self {
        self.ws_runtime = Some(WsRuntime::Igniter(reactor.igniter()));
        self
    }

    /// Sets the base Splinter REST API URL.
    ///
    /// This field is required by the final AwcAdminServiceEventClient.
    pub fn with_splinter_url(mut self, splinter_url: String) -> Self {
        self.root_url = Some(splinter_url);
        self
    }

    /// Sets the event type to receive.
    ///
    /// This field is required by the final AwcAdminServiceEventClient.
    pub fn with_event_type(mut self, event_type: String) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Sets the authorization value that will be sent with any REST API requests.
    ///
    /// This field is required by the final AwcAdminServiceEventClient.
    pub fn with_authorization(mut self, authorization: String) -> Self {
        self.authorization = Some(authorization);
        self
    }

    /// Sets the last event id.  This allows the client to start at a given event id, vs starting
    /// from the beginning of time.
    pub fn with_last_event_id(mut self, last_event_id: Option<u64>) -> Self {
        self.last_event_id = last_event_id;
        self
    }

    /// Build the runnable (but not started) AwcAdminServiceEventClient.
    ///
    /// # Errors
    ///
    /// Returns an InvalidStateError if any required fields are missing.
    pub fn build(self) -> Result<RunnableAwcAdminServiceEventClient, InvalidStateError> {
        let root_url = self
            .root_url
            .ok_or_else(|| InvalidStateError::with_message("A splinter url is required.".into()))?;
        let event_type = self
            .event_type
            .ok_or_else(|| InvalidStateError::with_message("An event type is required.".into()))?;
        let authorization = self.authorization.ok_or_else(|| {
            InvalidStateError::with_message("An authorization field is required.".into())
        })?;

        let ws_runtime = self
            .ws_runtime
            .unwrap_or_else(|| WsRuntime::Reactor(Some(Reactor::new())));
        let last_event_id = self.last_event_id;

        Ok(RunnableAwcAdminServiceEventClient {
            ws_runtime,
            root_url,
            event_type,
            authorization,
            last_event_id,
        })
    }
}

/// A configured, but not yet started AwcAdminServiceEventClient.
pub struct RunnableAwcAdminServiceEventClient {
    ws_runtime: WsRuntime,
    root_url: String,
    event_type: String,
    authorization: String,
    last_event_id: Option<u64>,
}

impl RunnableAwcAdminServiceEventClient {
    /// Starts the AwcAdminServiceEventClient.
    ///
    /// # Errors
    ///
    /// Returns an InternalError if the client is unable to start.
    pub fn run(self) -> Result<AwcAdminServiceEventClient, InternalError> {
        let Self {
            ws_runtime,
            root_url,
            event_type,
            authorization,
            last_event_id,
        } = self;

        let full_url = if let Some(id) = last_event_id.as_ref() {
            format!(
                "{}/ws/admin/register/{}?last={}",
                &root_url, &event_type, id
            )
        } else {
            format!("{}/ws/admin/register/{}", &root_url, &event_type,)
        };

        let (event_sender, event_receiver) = sync_channel(256);
        let last_event_id = Arc::new(AtomicU64::new(last_event_id.unwrap_or(0)));
        let received_id = last_event_id.clone();
        let received_sender = event_sender.clone();
        let mut ws_client = WebSocketClient::new(
            &full_url,
            &authorization,
            move |_, event: AdminServiceEvent| {
                let event_id = *event.event_id();
                match received_sender.try_send(Ok(event)) {
                    // This will block.  An async sleep would be better here, but we don't have a
                    // way of doing that, as this closure is hiding the fact that this closure is
                    // executed in a future.
                    Err(TrySendError::Full(evt)) => {
                        if received_sender.send(evt).is_err() {
                            error!("Receiver was dropped without shutting down the reactor.");
                            return WsResponse::Close;
                        }
                    }
                    Err(TrySendError::Disconnected(_evt_res)) => {
                        error!("Receiver was dropped without shutting down the reactor.");
                        return WsResponse::Close;
                    }
                    Ok(()) => (),
                }
                received_id.store(event_id, Ordering::SeqCst);
                WsResponse::Empty
            },
        );

        ws_client.header(
            "SplinterProtocolVersion",
            crate::protocol::ADMIN_SERVICE_PROTOCOL_VERSION.to_string(),
        );

        ws_client.set_reconnect(true);
        ws_client.set_reconnect_limit(10);
        ws_client.set_timeout(60);

        ws_client.on_error(move |err, _| {
            match event_sender.try_send(Err(err)) {
                // This will block.  An async sleep would be better here, but we don't have a
                // way of doing that, as this closure is hiding the fact that this closure is
                // executed in a future.
                Err(TrySendError::Full(e)) => {
                    if event_sender.send(e).is_err() {
                        error!("Receiver was dropped without shutting down the reactor.");
                    }
                }
                Err(TrySendError::Disconnected(_)) => {
                    error!("Receiver was dropped without shutting down the reactor.");
                }
                Ok(()) => (),
            }
            Ok(())
        });

        ws_client.on_reconnect(move |ws| {
            let last_seen_id = last_event_id.load(Ordering::SeqCst);
            let full_url = format!(
                "{}/ws/admin/register/{}?last={}",
                root_url, event_type, last_seen_id
            );
            ws.set_url(&full_url);
        });

        let igniter = match &ws_runtime {
            WsRuntime::Reactor(Some(reactor)) => reactor.igniter(),
            // This state cannot be reached at this point, as nothing can replace the value of this
            // option with None until the running client is shutdown.
            WsRuntime::Reactor(None) => unreachable!(),
            WsRuntime::Igniter(igniter) => igniter.clone(),
        };
        igniter
            .start_ws(&ws_client)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        Ok(AwcAdminServiceEventClient {
            ws_runtime,
            event_receiver,
        })
    }
}

pub struct AwcAdminServiceEventClient {
    ws_runtime: WsRuntime,
    event_receiver: Receiver<Result<AdminServiceEvent, WebSocketError>>,
}

impl ShutdownHandle for AwcAdminServiceEventClient {
    fn signal_shutdown(&mut self) {
        if let WsRuntime::Reactor(Some(reactor)) = &self.ws_runtime {
            if let Err(err) = reactor.shutdown_signaler().signal_shutdown() {
                error!(
                    "unable to signal event reactor to cleanly shutdown: {}",
                    err
                );
            }
        }
    }

    fn wait_for_shutdown(mut self) -> Result<(), InternalError> {
        match &mut self.ws_runtime {
            WsRuntime::Reactor(reactor) => {
                if let Some(reactor) = reactor.take() {
                    reactor
                        .wait_for_shutdown()
                        .map_err(|e| InternalError::from_source(Box::new(e)))
                } else {
                    // Calling this function will have consumed this object, so we don't have any
                    // alternative branches
                    unreachable!()
                }
            }
            _ => Ok(()),
        }
    }
}

impl Drop for AwcAdminServiceEventClient {
    fn drop(&mut self) {
        self.signal_shutdown();
    }
}

impl AdminServiceEventClient for AwcAdminServiceEventClient {
    /// Non-blocking
    fn try_next_event(&self) -> Result<Option<AdminServiceEvent>, NextEventError> {
        let evt_result = match self.event_receiver.try_recv() {
            Ok(res) => res,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => return Err(NextEventError::Disconnected),
        };

        evt_result
            .map(Some)
            .map_err(|e| NextEventError::InternalError(InternalError::from_source(Box::new(e))))
    }

    /// Blocking
    fn next_event(&self) -> Result<AdminServiceEvent, NextEventError> {
        let evt_result = self
            .event_receiver
            .recv()
            .map_err(|_| NextEventError::Disconnected)?;
        evt_result
            .map_err(|e| NextEventError::InternalError(InternalError::from_source(Box::new(e))))
    }
}

impl ParseBytes<AdminServiceEvent> for AdminServiceEvent {
    fn from_bytes(bytes: &[u8]) -> Result<AdminServiceEvent, ParseError> {
        let json_event: Event = serde_json::from_slice(bytes)
            .map_err(|err| ParseError::MalformedMessage(Box::new(err)))?;

        use messages::AdminServiceEvent::*;
        let (proposal, event_type) = match json_event.admin_event {
            ProposalSubmitted(proposal) => (proposal, EventType::ProposalSubmitted),
            ProposalVote((proposal, pub_key_bytes)) => (
                proposal,
                EventType::ProposalVote {
                    requester: PublicKey(pub_key_bytes),
                },
            ),
            ProposalAccepted((proposal, pub_key_bytes)) => (
                proposal,
                EventType::ProposalAccepted {
                    requester: PublicKey(pub_key_bytes),
                },
            ),
            ProposalRejected((proposal, pub_key_bytes)) => (
                proposal,
                EventType::ProposalRejected {
                    requester: PublicKey(pub_key_bytes),
                },
            ),
            CircuitReady(proposal) => (proposal, EventType::CircuitReady),
            CircuitDisbanded(proposal) => (proposal, EventType::CircuitDisbanded),
        };

        Ok(AdminServiceEvent {
            event_id: json_event.event_id,
            event_type,
            proposal: proposal.into(),
        })
    }
}

#[derive(Deserialize, Debug)]
struct Event {
    event_id: u64,

    #[serde(flatten)]
    admin_event: messages::AdminServiceEvent,
}

impl From<messages::CircuitProposal> for ProposalSlice {
    fn from(proposal: messages::CircuitProposal) -> Self {
        use messages::ProposalType::*;
        let proposal_type = match proposal.proposal_type {
            Create => "Create",
            UpdateRoster => "UpdateRoster",
            AddNode => "AddNode",
            RemoveNode => "RemoveNode",
            Disband => "Disband",
        }
        .to_owned();

        Self {
            proposal_type,
            circuit_id: proposal.circuit_id,
            circuit_hash: proposal.circuit_hash,
            circuit: proposal.circuit.into(),
            votes: proposal.votes.into_iter().map(VoteRecord::from).collect(),
            requester: hex::to_hex(&proposal.requester),
            requester_node_id: proposal.requester_node_id,
        }
    }
}

impl From<messages::CreateCircuit> for ProposalCircuitSlice {
    fn from(create_circuit: messages::CreateCircuit) -> Self {
        Self {
            circuit_id: create_circuit.circuit_id,
            members: create_circuit
                .members
                .into_iter()
                .map(CircuitMembers::from)
                .collect(),
            roster: create_circuit
                .roster
                .into_iter()
                .map(CircuitService::from)
                .collect(),
            management_type: create_circuit.circuit_management_type,
            comments: create_circuit.comments,
            display_name: create_circuit.display_name,
        }
    }
}

impl From<messages::VoteRecord> for VoteRecord {
    fn from(vote_record: messages::VoteRecord) -> Self {
        Self {
            public_key: hex::to_hex(&vote_record.public_key),
            vote: match vote_record.vote {
                messages::Vote::Accept => "Accept",
                messages::Vote::Reject => "Reject",
            }
            .into(),
            voter_node_id: vote_record.voter_node_id,
        }
    }
}

impl From<messages::SplinterNode> for CircuitMembers {
    fn from(splinter_node: messages::SplinterNode) -> Self {
        Self {
            node_id: splinter_node.node_id,
            endpoints: splinter_node.endpoints,
            #[cfg(feature = "challenge-authorization")]
            public_key: splinter_node
                .public_key
                .as_ref()
                .map(|public_key| hex::to_hex(&public_key)),
        }
    }
}

impl From<messages::SplinterService> for CircuitService {
    fn from(splinter_service: messages::SplinterService) -> Self {
        Self {
            service_id: splinter_service.service_id,
            service_type: splinter_service.service_type,
            node_id: splinter_service
                .allowed_nodes
                .into_iter()
                .next()
                .unwrap_or_else(|| String::from("<NONE>")),
            arguments: splinter_service
                .arguments
                .into_iter()
                .map(|(k, v)| vec![k, v])
                .collect(),
        }
    }
}
