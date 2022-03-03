// Copyright 2018-2022 Cargill Incorporated
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

mod builder;
mod consensus;
pub(crate) mod error;
pub(crate) mod messages;
pub(super) mod proposal_store;
mod shared;
mod subscriber;

use std::any::Any;
use std::collections::HashMap;
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cylinder::Verifier as SignatureVerifier;
use openssl::hash::{hash, MessageDigest};
use protobuf::{self, Message};

use crate::admin::store::{self, AdminServiceStore};
use crate::admin::token::PeerAuthorizationTokenReader;
use crate::circuit::routing::{self, RoutingTableWriter};
use crate::consensus::Proposal;
use crate::hex::to_hex;
use crate::keys::KeyPermissionManager;
use crate::orchestrator::{ServiceDefinition, ServiceOrchestrator};
use crate::peer::{PeerManagerConnector, PeerManagerNotification, PeerTokenPair};
use crate::protos::admin::{
    AdminMessage, AdminMessage_Type, CircuitManagementPayload, ServiceProtocolVersionResponse,
};
#[cfg(feature = "registry")]
use crate::registry::RegistryReader;
use crate::service::validation::ServiceArgValidator;
use crate::service::{
    error::{ServiceDestroyError, ServiceError, ServiceStartError, ServiceStopError},
    instance::ServiceInstance,
    ServiceMessageContext, ServiceNetworkRegistry,
};

use self::consensus::AdminConsensusManager;
use self::error::{AdminError, AdminSharedError, Sha256Error};
use self::proposal_store::{AdminServiceProposals, ProposalStore};
use self::shared::{get_peer_token_from_service_id, AdminServiceShared, PeerNodePair};

pub use self::builder::AdminServiceBuilder;
pub use self::error::AdminKeyVerifierError;
pub use self::error::AdminServiceError;
pub use self::error::AdminSubscriberError;
pub use self::shared::AdminServiceStatus;
pub use self::subscriber::AdminServiceEventSubscriber;

const ADMIN_SERVICE_PROTOCOL_MIN: u32 = 1;
pub(crate) const ADMIN_SERVICE_PROTOCOL_VERSION: u32 = 2;

pub trait AdminCommands: Send + Sync {
    fn submit_circuit_change(
        &self,
        circuit_change: CircuitManagementPayload,
    ) -> Result<(), AdminServiceError>;

    fn add_event_subscriber(
        &self,
        event_type: &str,
        subscriber: Box<dyn AdminServiceEventSubscriber>,
    ) -> Result<(), AdminServiceError>;

    fn get_events_since(
        &self,
        since_event_id: &i64,
        event_type: &str,
    ) -> Result<Events, AdminServiceError>;

    fn admin_service_status(&self) -> Result<AdminServiceStatus, AdminServiceError>;

    fn clone_boxed(&self) -> Box<dyn AdminCommands>;
}

impl Clone for Box<dyn AdminCommands> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Verifies that a key has permission to act as admin on behalf of a node.
pub trait AdminKeyVerifier: Send + Sync {
    /// Check if the given `key` is permitted as an admin for the given node.
    fn is_permitted(&self, node_id: &str, key: &[u8]) -> Result<bool, AdminKeyVerifierError>;
}

#[cfg(feature = "registry")]
impl AdminKeyVerifier for dyn RegistryReader {
    /// The key is permitted if and only if the node with the given `node_id` exists in the
    /// registry and the node has the given key. Otherwise, the key is not permitted.
    fn is_permitted(&self, node_id: &str, key: &[u8]) -> Result<bool, AdminKeyVerifierError> {
        let node_opt = self.get_node(node_id).map_err(|err| {
            AdminKeyVerifierError::new_with_source(
                &format!("Failed to lookup node '{}' in registry", node_id),
                Box::new(err),
            )
        })?;
        Ok(match node_opt {
            Some(node) => node.has_key(&to_hex(key)),
            None => false,
        })
    }
}

#[cfg(feature = "registry")]
impl AdminKeyVerifier for Box<dyn RegistryReader> {
    fn is_permitted(&self, node_id: &str, key: &[u8]) -> Result<bool, AdminKeyVerifierError> {
        (**self).is_permitted(node_id, key)
    }
}

pub struct Events {
    inner: Box<dyn ExactSizeIterator<Item = store::AdminServiceEvent> + Send>,
}

impl Iterator for Events {
    type Item = store::AdminServiceEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct AdminService {
    service_id: String,
    node_id: String,
    admin_service_shared: Arc<Mutex<AdminServiceShared>>,
    orchestrator: Arc<Mutex<ServiceOrchestrator>>,
    /// The coordinator timeout for the two-phase commit consensus engine
    coordinator_timeout: Duration,
    consensus: Option<AdminConsensusManager>,
    peer_connector: PeerManagerConnector,

    peer_notification_run_state: Option<(usize, JoinHandle<()>)>,
}

impl AdminService {
    #![allow(clippy::too_many_arguments)]
    #[deprecated(since = "0.5.1", note = "please use `AdminServiceBuilder` instead")]
    pub fn new(
        node_id: &str,
        orchestrator: ServiceOrchestrator,
        service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
        peer_connector: PeerManagerConnector,
        admin_store: Box<dyn AdminServiceStore>,
        signature_verifier: Box<dyn SignatureVerifier>,
        key_verifier: Box<dyn AdminKeyVerifier>,
        key_permission_manager: Box<dyn KeyPermissionManager>,
        // The coordinator timeout for the two-phase commit consensus engine; if `None`, the
        // default value will be used (30 seconds).
        coordinator_timeout: Option<Duration>,
        routing_table_writer: Box<dyn RoutingTableWriter>,
        event_store: Box<dyn AdminServiceStore>,
    ) -> Result<Self, ServiceError> {
        let mut builder = builder::AdminServiceBuilder::new()
            .with_node_id(node_id.to_string())
            .with_service_orchestrator(orchestrator)
            .with_peer_manager_connector(peer_connector)
            .with_admin_service_store(admin_store)
            .with_signature_verifier(signature_verifier)
            .with_admin_key_verifier(key_verifier)
            .with_key_permission_manager(key_permission_manager)
            .with_routing_table_writer(routing_table_writer)
            .with_admin_event_store(event_store)
            .with_service_arg_validators(service_arg_validators);

        if let Some(coordinator_timeout) = coordinator_timeout {
            builder = builder.with_coordinator_timeout(coordinator_timeout);
        }

        builder
            .build()
            .map_err(|e| ServiceError::UnableToCreate(Box::new(e)))
    }

    pub fn commands(&self) -> impl AdminCommands + Clone {
        AdminServiceCommands {
            shared: Arc::clone(&self.admin_service_shared),
        }
    }

    pub fn proposals(&self) -> impl ProposalStore {
        AdminServiceProposals::new(&self.admin_service_shared)
    }

    /// On restart of a splinter node, all services that this node should run on the existing
    /// circuits should be initialized using the service orchestrator. This may not include all
    /// services if they are not supported locally. It is expected that some services will be
    /// started externally.
    /// Furthermore, this adds services from circuits that have previously stopped their services
    /// to the orchestrator using the `add_stopped_service` which allows for the service data to
    /// be removed if the inactive circuit is purged. The separate handling of the inactive circuits
    /// is necessary in order to avoid adding networking functionality to these circuits, as they
    /// have already had this functionality removed through disbanding or abandoning.
    ///
    /// Also adds peer references for members of the circuits and proposals.
    fn re_initialize_circuits(&self) -> Result<(), ServiceStartError> {
        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .update_metrics()
            .map_err(|err| {
                ServiceStartError::Internal(format!("Unable to update metrics: {}", err))
            })?;

        let mut active_circuits = vec![];
        let mut inactive_circuits = vec![];
        for circuit in self
            .admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .get_circuits()
            .map_err(|err| {
                ServiceStartError::Internal(format!("Unable to get circuits: {}", err))
            })?
        {
            if circuit.circuit_status() == &store::CircuitStatus::Active {
                active_circuits.push(circuit);
            } else {
                inactive_circuits.push(circuit);
            }
        }

        let orchestrator = self.orchestrator.lock().map_err(|_| {
            ServiceStartError::PoisonedLock("the admin orchestrator lock was poisoned".into())
        })?;
        let mut peer_refs = vec![];
        // start all services of the supported types
        let mut writer = self
            .admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .routing_table_writer();

        let mut token_to_peer = HashMap::new();
        for circuit in active_circuits {
            let local_required_auth = circuit
                .get_node_token(&self.node_id)
                .map_err(|err| {
                    ServiceStartError::Internal(format!("Unable to get local nodes token: {}", err))
                })?
                .ok_or_else(|| {
                    ServiceStartError::Internal("Circuit does not have the local node".to_string())
                })?;

            let is_local = self
                .admin_service_shared
                .lock()
                .map_err(|_| {
                    ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
                })?
                .is_local_node(&local_required_auth);

            if !is_local {
                return Err(ServiceStartError::Internal(format!(
                    "Circuit {} contains unsupported token for \
                    local node: {}",
                    circuit.circuit_id(),
                    local_required_auth
                )));
            }

            let members = circuit.list_nodes().map_err(|err| {
                ServiceStartError::Internal(format!(
                    "Unable to get peer tokens for members: {}",
                    err
                ))
            })?;

            // restart all peer in the circuit
            for member in members {
                if member.node_id != self.node_id {
                    let peer_ref = self.peer_connector.add_peer_ref(
                        member.token.clone(),
                        member.endpoints.to_vec(),
                        local_required_auth.clone(),
                    );
                    if let Ok(peer_ref) = peer_ref {
                        peer_refs.push(peer_ref);
                    } else {
                        info!("Unable to peer with {} at this time", member.node_id);
                    }

                    token_to_peer.insert(
                        PeerTokenPair::new(member.token.clone(), local_required_auth.clone()),
                        PeerNodePair {
                            peer_node: member.clone(),
                            local_peer_token: local_required_auth.clone(),
                        },
                    );
                }
            }

            // Get all services this node is allowed to run and the orchestrator has a factory for

            let routing_services = circuit
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
                .collect();

            let services = circuit
                .roster()
                .iter()
                .filter(|service| {
                    service.node_id() == self.node_id
                        && orchestrator
                            .supported_service_types()
                            .contains(&service.service_type().to_string())
                })
                .collect::<Vec<_>>();

            let routing_members = circuit
                .members()
                .iter()
                .map(|node| {
                    routing::CircuitNode::new(
                        node.node_id().to_string(),
                        node.endpoints().to_vec(),
                        node.public_key().clone(),
                    )
                })
                .collect::<Vec<routing::CircuitNode>>();

            writer
                .add_circuit(
                    circuit.circuit_id().to_string(),
                    routing::Circuit::new(
                        circuit.circuit_id().to_string(),
                        routing_services,
                        circuit
                            .members()
                            .iter()
                            .map(|node| node.node_id().to_string())
                            .collect(),
                        circuit.authorization_type().into(),
                    ),
                    routing_members,
                )
                .map_err(|err| ServiceStartError::Internal(err.reduce_to_string()))?;

            // Start all services
            for service in services {
                let service_definition = ServiceDefinition {
                    circuit: circuit.circuit_id().into(),
                    service_id: service.service_id().into(),
                    service_type: service.service_type().into(),
                };

                let service_arguments = service
                    .arguments()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();

                if let Err(err) =
                    orchestrator.initialize_service(service_definition.clone(), service_arguments)
                {
                    error!(
                        "Unable to start service {} on circuit {}: {}",
                        service.service_id(),
                        circuit.circuit_id(),
                        err
                    );
                }
            }
        }

        for circuit in inactive_circuits {
            // Get all services this node is allowed to run and the orchestrator has a factory for
            let services = circuit
                .roster()
                .iter()
                .filter(|service| {
                    service.node_id() == self.node_id
                        && orchestrator
                            .supported_service_types()
                            .contains(&service.service_type().to_string())
                })
                .collect::<Vec<_>>();
            // Add all services from the inactive circuits to the orchestrator
            for service in services {
                let service_definition = ServiceDefinition {
                    circuit: circuit.circuit_id().into(),
                    service_id: service.service_id().into(),
                    service_type: service.service_type().into(),
                };

                let service_arguments = service
                    .arguments()
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();

                if let Err(err) =
                    orchestrator.add_stopped_service(service_definition.clone(), service_arguments)
                {
                    error!(
                        "Unable to add service {} from circuit {}: {}",
                        service.service_id(),
                        circuit.circuit_id(),
                        err
                    );
                }
            }
        }

        let proposals = self
            .admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .get_proposals(&[])
            .map_err(|err| {
                ServiceStartError::Internal(format!("Unable to get circuit proposals: {}", err))
            })?;

        for proposal in proposals {
            let local_required_auth = proposal
                .circuit()
                .get_node_token(&self.node_id)
                .map_err(|err| {
                    ServiceStartError::Internal(format!("Unable to get local nodes token: {}", err))
                })?
                .ok_or_else(|| {
                    ServiceStartError::Internal("Circuit does not have the local node".to_string())
                })?;

            let is_local = self
                .admin_service_shared
                .lock()
                .map_err(|_| {
                    ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
                })?
                .is_local_node(&local_required_auth);

            if !is_local {
                return Err(ServiceStartError::Internal(format!(
                    "Proposal {} contains unsupported token for \
                    local node: {}",
                    proposal.circuit_id(),
                    local_required_auth
                )));
            }

            let members = proposal.circuit().list_nodes().map_err(|err| {
                ServiceStartError::Internal(format!(
                    "Unable to get peer tokens for members: {}",
                    err
                ))
            })?;

            // connect to all peers in the circuit proposal
            for member in members.iter() {
                if member.node_id != self.node_id {
                    let peer_ref = self.peer_connector.add_peer_ref(
                        member.token.clone(),
                        member.endpoints.to_vec(),
                        local_required_auth.clone(),
                    );

                    if let Ok(peer_ref) = peer_ref {
                        peer_refs.push(peer_ref);
                    } else {
                        info!("Unable to peer with {} at this time", member.node_id);
                    }

                    token_to_peer.insert(
                        PeerTokenPair::new(member.token.clone(), local_required_auth.clone()),
                        PeerNodePair {
                            peer_node: member.clone(),
                            local_peer_token: local_required_auth.clone(),
                        },
                    );
                }
            }
        }

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .add_peer_refs(peer_refs);

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .set_token_to_peer(token_to_peer);

        Ok(())
    }
}

impl ServiceInstance for AdminService {
    fn service_id(&self) -> &str {
        &self.service_id
    }

    fn service_type(&self) -> &str {
        "admin"
    }

    fn start(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStartError> {
        if self.consensus.is_some() {
            return Err(ServiceStartError::AlreadyStarted);
        }

        let network_sender = service_registry.connect(&self.service_id)?;

        {
            let mut admin_service_shared = self.admin_service_shared.lock().map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?;

            admin_service_shared.set_network_sender(Some(network_sender));
        }

        let (sender, receiver) = channel();
        let peer_subscriber_id = self
            .peer_connector
            .subscribe_sender(sender)
            .map_err(|err| ServiceStartError::Internal(err.to_string()))?;

        let peer_admin_shared = self.admin_service_shared.clone();

        debug!("Starting admin service's peer manager notification receiver");
        let notification_join_handle = thread::Builder::new()
            .name("PeerManagerNotification Receiver".into())
            .spawn(move || loop {
                let notification = match receiver.recv() {
                    Ok(notification) => notification,
                    Err(_) => {
                        warn!(
                            "Admin service received an error while listening to peer manager \
                            notifications, indicating remote thread has shutdown"
                        );
                        break;
                    }
                };

                if let Ok(mut admin_shared) = peer_admin_shared.lock() {
                    handle_peer_manager_notification(notification, &mut *admin_shared);
                } else {
                    error!("the admin shared lock was poisoned");
                    break;
                }
            })
            .map_err(|err| ServiceStartError::Internal(err.to_string()))?;

        self.peer_notification_run_state = Some((peer_subscriber_id, notification_join_handle));

        // Setup consensus
        let consensus = AdminConsensusManager::new(
            self.service_id().into(),
            self.admin_service_shared.clone(),
            self.coordinator_timeout,
        )
        .map_err(|err| {
            ServiceStartError::Internal(format!("Unable to start consensus: {}", err))
        })?;

        let proposal_sender = consensus.proposal_update_sender();

        self.consensus = Some(consensus);

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .set_proposal_sender(Some(proposal_sender));

        self.re_initialize_circuits()?;

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStartError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .change_status();

        Ok(())
    }

    fn stop(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStopError> {
        service_registry.disconnect(&self.service_id)?;

        // Shutdown consensus
        self.consensus
            .take()
            .ok_or(ServiceStopError::NotStarted)?
            .shutdown()
            .map_err(|err| ServiceStopError::Internal(Box::new(err)))?;

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStopError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .remove_all_event_subscribers();

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStopError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .change_status();

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStopError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .set_network_sender(None);

        self.orchestrator
            .lock()
            .map_err(|_| {
                ServiceStopError::PoisonedLock("the admin orchestrator lock was poisoned".into())
            })?
            .shutdown_all_services()
            .map_err(|err| ServiceStopError::Internal(Box::new(err)))?;

        self.admin_service_shared
            .lock()
            .map_err(|_| {
                ServiceStopError::PoisonedLock("the admin shared lock was poisoned".into())
            })?
            .change_status();

        if let Some((peer_subscriber_id, peer_notfication_join_handle)) =
            self.peer_notification_run_state.take()
        {
            if let Err(err) = self.peer_connector.unsubscribe(peer_subscriber_id) {
                warn!(
                    "Unable to unsubscribe from peer manager notifications: {:?}",
                    err
                );
            }

            if let Err(err) = peer_notfication_join_handle.join() {
                error!("Failed to join peer notification thread: {:?}", err);
            }
        }

        info!("Admin service stopped and disconnected");

        Ok(())
    }

    fn destroy(self: Box<Self>) -> Result<(), ServiceDestroyError> {
        if self.consensus.is_some() {
            Err(ServiceDestroyError::NotStopped)
        } else {
            Ok(())
        }
    }

    fn purge(&mut self) -> Result<(), crate::error::InternalError> {
        Ok(())
    }

    fn handle_message(
        &self,
        message_bytes: &[u8],
        message_context: &ServiceMessageContext,
    ) -> Result<(), ServiceError> {
        let admin_message: AdminMessage = Message::parse_from_bytes(message_bytes)
            .map_err(|err| ServiceError::InvalidMessageFormat(Box::new(err)))?;

        debug!("received admin message {:?}", admin_message);
        match admin_message.get_message_type() {
            AdminMessage_Type::CONSENSUS_MESSAGE => self
                .consensus
                .as_ref()
                .ok_or(ServiceError::NotStarted)?
                .handle_message(admin_message.get_consensus_message())
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err))),
            AdminMessage_Type::PROPOSED_CIRCUIT => {
                let proposed_circuit = admin_message.get_proposed_circuit();

                let expected_hash = proposed_circuit.get_expected_hash().to_vec();
                let circuit_payload = proposed_circuit.get_circuit_payload();
                let required_verifiers = proposed_circuit.get_required_verifiers();
                let proposal = Proposal {
                    id: sha256(circuit_payload)
                        .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?
                        .as_bytes()
                        .into(),
                    summary: expected_hash,
                    consensus_data: required_verifiers.to_vec(),
                    ..Default::default()
                };

                let mut admin_service_shared = self.admin_service_shared.lock().map_err(|_| {
                    ServiceError::PoisonedLock("the admin shared lock was poisoned".into())
                })?;

                admin_service_shared.handle_proposed_circuit(
                    proposal,
                    circuit_payload.clone(),
                    message_context.sender.to_string(),
                )
            }
            AdminMessage_Type::MEMBER_READY => {
                let member_ready = admin_message.get_member_ready();
                let circuit_id = member_ready.get_circuit_id();
                let member_node_id = member_ready.get_member_node_id();

                let mut shared = self.admin_service_shared.lock().map_err(|_| {
                    ServiceError::PoisonedLock("the admin shared lock was poisoned".into())
                })?;

                shared
                    .add_ready_member(circuit_id, member_node_id.into())
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
            }
            AdminMessage_Type::SERVICE_PROTOCOL_VERSION_REQUEST => {
                let request = admin_message.get_protocol_request();
                let protocol =
                    supported_protocol_version(request.protocol_min, request.protocol_max);

                let mut response = ServiceProtocolVersionResponse::new();
                response.set_protocol(protocol);

                let mut msg = AdminMessage::new();
                msg.set_message_type(AdminMessage_Type::SERVICE_PROTOCOL_VERSION_RESPONSE);
                msg.set_protocol_response(response);
                let envelope_bytes = msg
                    .write_to_bytes()
                    .map_err(|err| ServiceError::InvalidMessageFormat(Box::new(err)))?;

                let mut admin_service_shared = self.admin_service_shared.lock().map_err(|_| {
                    ServiceError::PoisonedLock("the admin shared lock was poisoned".into())
                })?;

                // Need to set the sender of this message to this nodes admin service id
                // the default can't be used here incase the authorization type is challenge,
                // the resulting sender will either be set to admin::<node_id> or
                // admin::public_key::<remote_key>::public_key::<local_key>
                let sender_peer_token =
                    get_peer_token_from_service_id(&message_context.sender, &self.node_id)
                        .map_err(|err| {
                            ServiceError::UnableToHandleMessage(Box::new(
                                AdminSharedError::ServiceProtocolError(format!(
                                    "Unable to verify peer token for service id: {}",
                                    err
                                )),
                            ))
                        })?;
                let local_sender = admin_service_id(
                    // the id for the sender from the local nodes perspective
                    &PeerTokenPair::new(
                        sender_peer_token.local_id().clone(),
                        sender_peer_token.peer_id().clone(),
                    )
                    .id_as_string(),
                );

                admin_service_shared
                    .network_sender()
                    .clone()
                    .ok_or(ServiceError::NotStarted)?
                    .send_with_sender(&message_context.sender, &envelope_bytes, &local_sender)
                    .map_err(|err| ServiceError::UnableToSendMessage(Box::new(err)))?;
                admin_service_shared
                    .on_protocol_agreement(&message_context.sender, protocol)
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
            }
            AdminMessage_Type::SERVICE_PROTOCOL_VERSION_RESPONSE => {
                let request = admin_message.get_protocol_response();
                let protocol = request.get_protocol();

                let mut admin_service_shared = self.admin_service_shared.lock().map_err(|_| {
                    ServiceError::PoisonedLock("the admin shared lock was poisoned".into())
                })?;

                if !(ADMIN_SERVICE_PROTOCOL_MIN..=ADMIN_SERVICE_PROTOCOL_VERSION)
                    .contains(&protocol)
                {
                    warn!(
                        "Received service protocol version is not supported: {}",
                        protocol
                    );

                    admin_service_shared
                        .on_protocol_agreement(&message_context.sender, 0)
                        .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
                } else {
                    admin_service_shared
                        .on_protocol_agreement(&message_context.sender, protocol)
                        .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;
                }

                Ok(())
            }
            AdminMessage_Type::ABANDONED_CIRCUIT => {
                let abandoned_circuit = admin_message.get_abandoned_circuit();
                let circuit_id = abandoned_circuit.get_circuit_id();
                let member_node_id = abandoned_circuit.get_member_node_id();

                warn!(
                    "Member {} has abandoned circuit {}",
                    member_node_id, circuit_id
                );
                Ok(())
            }
            AdminMessage_Type::REMOVED_PROPOSAL => {
                let removed_proposal = admin_message.get_removed_proposal();
                let circuit_id = removed_proposal.get_circuit_id();

                warn!(
                    "A prospective member has removed the proposal for circuit {}",
                    circuit_id
                );
                Ok(())
            }
            AdminMessage_Type::UNSET => Err(ServiceError::InvalidMessageFormat(Box::new(
                AdminError::MessageTypeUnset,
            ))),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn handle_peer_manager_notification(
    notification: PeerManagerNotification,
    admin_shared: &mut AdminServiceShared,
) {
    match notification {
        PeerManagerNotification::Connected { peer } => {
            debug!("Peer {} has connected", peer);
            if let Err(err) = admin_shared.on_peer_connected(&peer) {
                error!("Error occurred while handling Connected: {}", err);
            }
        }
        PeerManagerNotification::Disconnected { peer } => {
            debug!("Peer {} has disconnected", peer);
            admin_shared.on_peer_disconnected(peer);
        }
    }
}

#[derive(Clone)]
struct AdminServiceCommands {
    shared: Arc<Mutex<AdminServiceShared>>,
}

impl AdminCommands for AdminServiceCommands {
    fn submit_circuit_change(
        &self,
        circuit_change: CircuitManagementPayload,
    ) -> Result<(), AdminServiceError> {
        self.shared
            .lock()
            .map_err(|_| AdminServiceError::general_error("Admin shared lock was lock poisoned"))?
            .submit(circuit_change)?;

        Ok(())
    }

    fn add_event_subscriber(
        &self,
        event_type: &str,
        subscriber: Box<dyn AdminServiceEventSubscriber>,
    ) -> Result<(), AdminServiceError> {
        self.shared
            .lock()
            .map_err(|_| AdminServiceError::general_error("Admin shared lock was lock poisoned"))?
            .add_subscriber(event_type.into(), subscriber)
            .map_err(|err| {
                AdminServiceError::general_error_with_source(
                    "Unable to add event subscriber",
                    Box::new(err),
                )
            })
    }

    fn get_events_since(
        &self,
        since_event_id: &i64,
        event_type: &str,
    ) -> Result<Events, AdminServiceError> {
        self.shared
            .lock()
            .map_err(|_| AdminServiceError::general_error("Admin shared lock was lock poisoned"))?
            .get_events_since(since_event_id, event_type)
            .map_err(|err| {
                AdminServiceError::general_error_with_source("Unable to get events", Box::new(err))
            })
    }

    fn admin_service_status(&self) -> Result<AdminServiceStatus, AdminServiceError> {
        Ok(self
            .shared
            .lock()
            .map_err(|_| AdminServiceError::general_error("Admin shared lock was lock poisoned"))?
            .admin_service_status())
    }

    fn clone_boxed(&self) -> Box<dyn AdminCommands> {
        Box::new(self.clone())
    }
}

pub fn admin_service_id(node_id: &str) -> String {
    format!("admin::{}", node_id)
}

fn sha256<T>(message: &T) -> Result<String, Sha256Error>
where
    T: Message,
{
    let bytes = message
        .write_to_bytes()
        .map_err(|err| Sha256Error(Box::new(err)))?;
    hash(MessageDigest::sha256(), &bytes)
        .map(|digest| to_hex(&*digest))
        .map_err(|err| Sha256Error(Box::new(err)))
}

fn supported_protocol_version(min: u32, max: u32) -> u32 {
    if max < min {
        info!("Received invalid ServiceProtocolVersionRequest: min cannot be greater than max");
        return 0;
    }

    if min > ADMIN_SERVICE_PROTOCOL_VERSION {
        info!(
            "Request requires newer version than can be provided: {}",
            min
        );
        return 0;
    } else if max < ADMIN_SERVICE_PROTOCOL_MIN {
        info!(
            "Request requires older version than can be provided: {}",
            max
        );
        return 0;
    }

    if max >= ADMIN_SERVICE_PROTOCOL_VERSION {
        ADMIN_SERVICE_PROTOCOL_VERSION
    } else if max > ADMIN_SERVICE_PROTOCOL_MIN {
        max
    } else if min > ADMIN_SERVICE_PROTOCOL_MIN {
        min
    } else {
        ADMIN_SERVICE_PROTOCOL_MIN
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;

    use std::sync::mpsc::{channel, Sender};
    use std::time::{Duration, Instant};

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use diesel::{
        r2d2::{ConnectionManager as DieselConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    use crate::admin::store::diesel::DieselAdminServiceStore;
    use crate::circuit::routing::memory::RoutingTable;
    use crate::keys::insecure::AllowAllKeyPermissionManager;
    use crate::mesh::Mesh;
    use crate::migrations::run_sqlite_migrations;
    use crate::network::auth::AuthorizationManager;
    use crate::network::connection_manager::authorizers::{Authorizers, InprocAuthorizer};
    use crate::network::connection_manager::ConnectionManager;
    use crate::orchestrator::ServiceOrchestratorBuilder;
    use crate::peer::PeerManager;
    use crate::protos::admin;
    use crate::service::{error, ServiceNetworkRegistry, ServiceNetworkSender};
    use crate::threading::lifecycle::ShutdownHandle;
    use crate::transport::{inproc::InprocTransport, Transport};

    /// Test that a circuit creation creates the correct connections and sends the appropriate
    /// messages.
    #[test]
    fn test_propose_circuit() {
        let mut transport = InprocTransport::default();
        let mut orchestrator_transport = transport.clone();

        let _listener = transport
            .listen("inproc://otherplace:8000")
            .expect("Unable to get listener");
        let _orchestator_listener = transport
            .listen("inproc://orchestator")
            .expect("Unable to get listener");

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
            ],
            "test-node".to_string(),
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

        let mut mesh = Mesh::new(2, 2);
        let mut cm = ConnectionManager::builder()
            .with_authorizer(Box::new(authorizers))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(Box::new(transport.clone()))
            .start()
            .expect("Unable to start Connection Manager");
        let connector = cm.connector();

        let mut peer_manager = PeerManager::builder()
            .with_connector(connector)
            .with_retry_interval(1)
            .with_identity("test-node".to_string())
            .with_strict_ref_counts(true)
            .start()
            .expect("Cannot start peer_manager");
        let peer_connector = peer_manager.connector();

        let connection_manager = DieselConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        let orchestrator_connection = orchestrator_transport
            .connect("inproc://orchestator")
            .expect("failed to create connection");
        let orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .expect("failed to create orchestrator")
            .run()
            .expect("failed to start orchestrator");

        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let signer = context.new_signer(private_key);
        let signature_verifier = context.new_verifier();

        let table = RoutingTable::default();
        let writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());
        let store = Box::new(DieselAdminServiceStore::new(pool));
        let event_store = store.clone_boxed();

        let mut admin_service_builder = AdminServiceBuilder::new();

        admin_service_builder = admin_service_builder
            .with_node_id("test-node".into())
            .with_service_orchestrator(orchestrator)
            .with_peer_manager_connector(peer_connector)
            .with_admin_service_store(store)
            .with_signature_verifier(signature_verifier)
            .with_admin_key_verifier(Box::new(MockAdminKeyVerifier))
            .with_key_permission_manager(Box::new(AllowAllKeyPermissionManager))
            .with_routing_table_writer(writer)
            .with_admin_event_store(event_store);

        let mut admin_service = admin_service_builder
            .build()
            .expect("Service should have been created correctly");

        let (tx, rx) = channel();
        admin_service
            .start(&MockNetworkRegistry { tx })
            .expect("Service should have started correctly");

        let mut proposed_circuit = admin::Circuit::new();
        proposed_circuit.set_circuit_id("01234-ABCDE".into());
        proposed_circuit
            .set_authorization_type(admin::Circuit_AuthorizationType::TRUST_AUTHORIZATION);
        proposed_circuit.set_persistence(admin::Circuit_PersistenceType::ANY_PERSISTENCE);
        proposed_circuit.set_routes(admin::Circuit_RouteType::ANY_ROUTE);
        proposed_circuit.set_durability(admin::Circuit_DurabilityType::NO_DURABILITY);
        proposed_circuit.set_circuit_management_type("test app auth handler".into());
        proposed_circuit.set_comments("test circuit".into());
        proposed_circuit.set_display_name("test_display".into());

        proposed_circuit.set_members(protobuf::RepeatedField::from_vec(vec![
            splinter_node("test-node", &["inproc://someplace:8000".into()]),
            splinter_node("other-node", &["inproc://otherplace:8000".into()]),
        ]));
        proposed_circuit.set_roster(protobuf::RepeatedField::from_vec(vec![
            splinter_service("0123", "sabre", "test-node"),
            splinter_service("ABCD", "sabre", "other-node"),
        ]));

        let mut request = admin::CircuitCreateRequest::new();
        request.set_circuit(proposed_circuit.clone());

        let mut header = admin::CircuitManagementPayload_Header::new();
        header.set_action(admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST);
        header.set_requester(
            signer
                .public_key()
                .expect("Failed to get signer's public key")
                .into_bytes(),
        );
        header.set_requester_node_id("test-node".to_string());

        let mut payload = admin::CircuitManagementPayload::new();

        payload.set_signature(signer.sign(&payload.header).unwrap().take_bytes());
        payload.set_header(protobuf::Message::write_to_bytes(&header).unwrap());
        payload.set_circuit_create_request(request);

        admin_service
            .admin_service_shared
            .lock()
            .unwrap()
            .propose_circuit(payload, "test".to_string())
            .expect("The proposal was not handled correctly");

        // wait up to 60 second for the service protocol version request
        let message;
        let start = Instant::now();
        loop {
            if Instant::now().duration_since(start) > Duration::from_secs(60) {
                panic!("Failed to receive service protocol version request in time");
            }

            if let Ok((_, m)) = rx.recv_timeout(Duration::from_millis(100)) {
                message = m;
                break;
            }
        }

        let admin_envelope: admin::AdminMessage =
            Message::parse_from_bytes(&message).expect("The message could not be parsed");

        assert_eq!(
            admin::AdminMessage_Type::SERVICE_PROTOCOL_VERSION_REQUEST,
            admin_envelope.get_message_type()
        );

        // add agreement for protocol version for other-node
        admin_service
            .admin_service_shared
            .lock()
            .unwrap()
            .on_protocol_agreement("admin::other-node", ADMIN_SERVICE_PROTOCOL_VERSION)
            .expect("Unable to set protocol agreement");

        // wait up to 60 second for the proposed circuit message
        let recipient;
        let message;
        let start = Instant::now();
        loop {
            if Instant::now().duration_since(start) > Duration::from_secs(60) {
                panic!("Failed to receive proposed circuit message in time");
            }

            if let Ok((r, m)) = rx.recv_timeout(Duration::from_millis(100)) {
                recipient = r;
                message = m;
                break;
            }
        }

        assert_eq!("admin::other-node".to_string(), recipient);

        let mut admin_envelope: admin::AdminMessage =
            Message::parse_from_bytes(&message).expect("The message could not be parsed");

        assert_eq!(
            admin::AdminMessage_Type::PROPOSED_CIRCUIT,
            admin_envelope.get_message_type()
        );

        let mut envelope = admin_envelope
            .take_proposed_circuit()
            .take_circuit_payload();

        let header: admin::CircuitManagementPayload_Header =
            Message::parse_from_bytes(envelope.get_header()).unwrap();
        assert_eq!(
            admin::CircuitManagementPayload_Action::CIRCUIT_CREATE_REQUEST,
            header.get_action()
        );
        assert_eq!(
            proposed_circuit,
            envelope.take_circuit_create_request().take_circuit()
        );

        peer_manager.signal_shutdown();
        peer_manager
            .wait_for_shutdown()
            .expect("Unable to shutdown peer manager");
        cm.signal_shutdown();
        cm.wait_for_shutdown()
            .expect("Unable to shutdown connection manager");
        mesh.signal_shutdown();
        mesh.wait_for_shutdown().expect("Unable to shutdown mesh");
    }

    fn splinter_node(node_id: &str, endpoints: &[String]) -> admin::SplinterNode {
        let mut node = admin::SplinterNode::new();
        node.set_node_id(node_id.into());
        node.set_endpoints(endpoints.into());
        node
    }

    fn splinter_service(
        service_id: &str,
        service_type: &str,
        allowed_node: &str,
    ) -> admin::SplinterService {
        let mut service = admin::SplinterService::new();
        service.set_service_id(service_id.into());
        service.set_service_type(service_type.into());
        service.set_allowed_nodes(vec![allowed_node.into()].into());
        service
    }

    struct MockNetworkRegistry {
        tx: Sender<(String, Vec<u8>)>,
    }

    impl ServiceNetworkRegistry for MockNetworkRegistry {
        fn connect(
            &self,
            _service_id: &str,
        ) -> Result<Box<dyn ServiceNetworkSender>, error::ServiceConnectionError> {
            Ok(Box::new(MockNetworkSender {
                tx: self.tx.clone(),
            }))
        }

        fn disconnect(&self, _service_id: &str) -> Result<(), error::ServiceDisconnectionError> {
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MockNetworkSender {
        tx: Sender<(String, Vec<u8>)>,
    }

    impl ServiceNetworkSender for MockNetworkSender {
        fn send(&self, recipient: &str, message: &[u8]) -> Result<(), error::ServiceSendError> {
            self.tx
                .send((recipient.to_string(), message.to_vec()))
                .expect("Unable to send test message");

            Ok(())
        }

        fn send_and_await(
            &self,
            _recipient: &str,
            _message: &[u8],
        ) -> Result<Vec<u8>, error::ServiceSendError> {
            panic!("MockNetworkSender.send_and_await unexpectedly called")
        }

        fn reply(
            &self,
            _message_origin: &ServiceMessageContext,
            _message: &[u8],
        ) -> Result<(), error::ServiceSendError> {
            panic!("MockNetworkSender.reply unexpectedly called")
        }

        fn clone_box(&self) -> Box<dyn ServiceNetworkSender> {
            Box::new(self.clone())
        }

        fn send_with_sender(
            &mut self,
            recipient: &str,
            message: &[u8],
            _sender: &str,
        ) -> Result<(), error::ServiceSendError> {
            self.tx
                .send((recipient.to_string(), message.to_vec()))
                .expect("Unable to send test message");
            Ok(())
        }
    }

    struct MockAdminKeyVerifier;

    impl AdminKeyVerifier for MockAdminKeyVerifier {
        fn is_permitted(&self, _node_id: &str, _key: &[u8]) -> Result<bool, AdminKeyVerifierError> {
            Ok(true)
        }
    }
}
