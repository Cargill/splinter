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

mod builder;
mod error;
#[cfg(feature = "rest-api")]
mod rest_api;
mod runnable;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use protobuf::Message;

use crate::channel;
use crate::error::InternalError;
use crate::mesh::{Envelope, Mesh, RecvTimeoutError as MeshRecvTimeoutError};
use crate::network::reply::InboundRouter;
use crate::protos::circuit::{
    AdminDirectMessage, CircuitDirectMessage, CircuitError, CircuitMessage, CircuitMessageType,
    ServiceConnectResponse, ServiceDisconnectResponse,
};
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::service::{
    Service, ServiceFactory, ServiceMessageContext, StandardServiceNetworkRegistry,
};
#[cfg(feature = "shutdown")]
use crate::threading::shutdown::ShutdownHandle;
use crate::transport::Connection;

pub use self::builder::ServiceOrchestratorBuilder;
pub use self::error::{
    AddServiceError, InitializeServiceError, ListServicesError, NewOrchestratorError,
    OrchestratorError, ShutdownServiceError,
};
pub use self::runnable::RunnableServiceOrchestrator;

// Recv timeout in secs
const TIMEOUT_SEC: u64 = 2;

/// Identifies a unique service instance from the perspective of the orchestrator
#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub struct ServiceDefinition {
    pub circuit: String,
    pub service_id: String,
    pub service_type: String,
}

impl std::fmt::Display for ServiceDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}::{} ({})",
            self.circuit, self.service_id, self.service_type
        )
    }
}

/// Stores a service and other structures that are used to manage it
struct ManagedService {
    pub service: Box<dyn Service>,
    pub registry: StandardServiceNetworkRegistry,
}

/// The `ServiceOrchestrator` manages initialization and shutdown of services.
pub struct ServiceOrchestrator {
    /// A (ServiceDefinition, ManagedService) map
    services: Arc<Mutex<HashMap<ServiceDefinition, ManagedService>>>,
    /// Factories used to create new services.
    service_factories: Vec<Box<dyn ServiceFactory>>,
    supported_service_types: Vec<String>,
    /// `network_sender` and `inbound_router` are used to create services' senders.
    network_sender: Sender<Vec<u8>>,
    inbound_router: InboundRouter<CircuitMessageType>,
    /// A (ServiceDefinition, ManagedService) map of services that have been stopped, but yet to
    /// be completely destroyed
    stopped_services: Arc<Mutex<HashMap<ServiceDefinition, Box<dyn Service>>>>,

    /// `running` and `join_handles` are used to shutdown the orchestrator's background threads
    running: Arc<AtomicBool>,
    join_handles: Option<JoinHandles<Result<(), OrchestratorError>>>,
}

impl ServiceOrchestrator {
    /// Create a new `ServiceOrchestrator`. This starts up 3 threads for relaying messages to and
    /// from services. Returns the `ServiceOrchestrator` and the threads `JoinHandles`
    #[deprecated(
        since = "0.5.1",
        note = "please use `ServiceOrchestratorBuilder` instead"
    )]
    pub fn new(
        service_factories: Vec<Box<dyn ServiceFactory>>,
        connection: Box<dyn Connection>,
        incoming_capacity: usize,
        outgoing_capacity: usize,
        channel_capacity: usize,
    ) -> Result<Self, NewOrchestratorError> {
        let mut builder = builder::ServiceOrchestratorBuilder::new()
            .with_connection(connection)
            .with_incoming_capacity(incoming_capacity)
            .with_outgoing_capacity(outgoing_capacity)
            .with_channel_capacity(channel_capacity);

        for service_factory in service_factories.into_iter() {
            builder = builder.with_service_factory(service_factory);
        }

        builder
            .build()
            .map_err(|e| NewOrchestratorError(Box::new(e)))?
            .run()
            .map_err(|e| NewOrchestratorError(Box::new(e)))
    }

    #[cfg(not(feature = "shutdown"))]
    pub fn take_join_handles(&mut self) -> Option<JoinHandles<Result<(), OrchestratorError>>> {
        self.join_handles.take()
    }

    #[cfg(feature = "shutdown")]
    pub fn take_shutdown_handle(&mut self) -> Option<Box<dyn ShutdownHandle>> {
        let join_handles = self.join_handles.take()?;
        Some(Box::new(ServiceOrchestratorShutdownHandle {
            services: Arc::clone(&self.services),
            join_handles: Some(join_handles),
            running: Arc::clone(&self.running),
        }) as Box<dyn ShutdownHandle>)
    }

    /// Initialize (create and start) a service according to the specified definition. The
    /// arguments provided must match those required to create the service.
    pub fn initialize_service(
        &self,
        service_definition: ServiceDefinition,
        args: HashMap<String, String>,
    ) -> Result<(), InitializeServiceError> {
        // Get the factory that can create this service.
        let factory = self
            .service_factories
            .iter()
            .find(|factory| {
                factory
                    .available_service_types()
                    .contains(&service_definition.service_type)
            })
            .ok_or(InitializeServiceError::UnknownType)?;

        // Create the service.
        let mut service = factory.create(
            service_definition.service_id.clone(),
            service_definition.service_type.as_str(),
            service_definition.circuit.as_str(),
            args,
        )?;

        // Start the service.
        let registry = StandardServiceNetworkRegistry::new(
            service_definition.circuit.clone(),
            self.network_sender.clone(),
            self.inbound_router.clone(),
        );

        service
            .start(&registry)
            .map_err(|err| InitializeServiceError::InitializationFailed(Box::new(err)))?;

        // Save the service.
        self.services
            .lock()
            .map_err(|_| InitializeServiceError::LockPoisoned)?
            .insert(service_definition, ManagedService { service, registry });

        Ok(())
    }

    /// Stop the specified service.
    pub fn stop_service(
        &self,
        service_definition: &ServiceDefinition,
    ) -> Result<(), ShutdownServiceError> {
        let ManagedService {
            mut service,
            registry,
        } = self
            .services
            .lock()
            .map_err(|_| ShutdownServiceError::LockPoisoned)?
            .remove(service_definition)
            .ok_or(ShutdownServiceError::UnknownService)?;

        service.stop(&registry).map_err(|err| {
            ShutdownServiceError::ShutdownFailed((service_definition.clone(), Box::new(err)))
        })?;

        self.stopped_services
            .lock()
            .map_err(|_| ShutdownServiceError::LockPoisoned)?
            .insert(service_definition.clone(), service);

        Ok(())
    }

    /// Purge the specified service state, based on its service implementation.
    pub fn purge_service(
        &self,
        service_definition: &ServiceDefinition,
    ) -> Result<(), InternalError> {
        if let Some(mut service) = self
            .stopped_services
            .lock()
            .map_err(|_| {
                InternalError::with_message("Orchestrator stopped service lock was poisoned".into())
            })?
            .remove(service_definition)
        {
            service.purge()
        } else {
            Ok(())
        }
    }

    /// Shut down (stop and destroy) all services managed by this `ServiceOrchestrator` and single
    /// the `ServiceOrchestrator` to shutdown
    pub fn shutdown_all_services(&self) -> Result<(), ShutdownServiceError> {
        let mut services = self
            .services
            .lock()
            .map_err(|_| ShutdownServiceError::LockPoisoned)?;

        for (service_definition, managed_service) in services.drain() {
            let ManagedService {
                mut service,
                registry,
            } = managed_service;
            service.stop(&registry).map_err(|err| {
                ShutdownServiceError::ShutdownFailed((service_definition.clone(), Box::new(err)))
            })?;
            service.destroy().map_err(|err| {
                ShutdownServiceError::ShutdownFailed((service_definition, Box::new(err)))
            })?;
        }
        self.running.store(false, Ordering::SeqCst);

        Ok(())
    }

    /// List services managed by this `ServiceOrchestrator`; filters may be provided to only show
    /// services on specified circuit(s) and of given service type(s).
    pub fn list_services(
        &self,
        circuits: Vec<String>,
        service_types: Vec<String>,
    ) -> Result<Vec<ServiceDefinition>, ListServicesError> {
        Ok(self
            .services
            .lock()
            .map_err(|_| ListServicesError::LockPoisoned)?
            .iter()
            .filter_map(|(service, _)| {
                if (circuits.is_empty() || circuits.contains(&service.circuit))
                    && (service_types.is_empty() || service_types.contains(&service.service_type))
                {
                    Some(service)
                } else {
                    None
                }
            })
            .cloned()
            .collect())
    }

    /// Create a service that has previously been stopped according to the specified definition.
    /// The arguments provided must match those required to create the service.
    pub fn add_stopped_service(
        &self,
        service_definition: ServiceDefinition,
        args: HashMap<String, String>,
    ) -> Result<(), AddServiceError> {
        // Get the factory that can create this service.
        let factory = self
            .service_factories
            .iter()
            .find(|factory| {
                factory
                    .available_service_types()
                    .contains(&service_definition.service_type)
            })
            .ok_or(AddServiceError::UnknownType)?;

        // Create the previously stopped service.
        let service = factory.create(
            service_definition.service_id.clone(),
            service_definition.service_type.as_str(),
            service_definition.circuit.as_str(),
            args,
        )?;

        // Save the service to `stopped_services`.
        self.stopped_services
            .lock()
            .map_err(|_| AddServiceError::LockPoisoned)?
            .insert(service_definition, service);

        Ok(())
    }

    pub fn supported_service_types(&self) -> &[String] {
        &self.supported_service_types
    }
}

pub struct JoinHandles<T> {
    join_handles: Vec<JoinHandle<T>>,
}

impl<T> JoinHandles<T> {
    fn new(join_handles: Vec<JoinHandle<T>>) -> Self {
        Self { join_handles }
    }

    pub fn join_all(self) -> thread::Result<Vec<T>> {
        let mut res = Vec::with_capacity(self.join_handles.len());

        for jh in self.join_handles.into_iter() {
            res.push(jh.join()?);
        }

        Ok(res)
    }
}

#[cfg(feature = "shutdown")]
struct ServiceOrchestratorShutdownHandle {
    services: Arc<Mutex<HashMap<ServiceDefinition, ManagedService>>>,
    join_handles: Option<JoinHandles<Result<(), OrchestratorError>>>,
    running: Arc<AtomicBool>,
}

#[cfg(feature = "shutdown")]
impl ShutdownHandle for ServiceOrchestratorShutdownHandle {
    fn signal_shutdown(&mut self) {
        match self.services.lock() {
            Ok(mut services) => {
                for (service_definition, managed_service) in services.drain() {
                    let ManagedService {
                        mut service,
                        registry,
                    } = managed_service;
                    if let Err(err) = service.stop(&registry) {
                        error!("Unable to stop service {}: {}", service_definition, err);
                    }
                    if let Err(err) = service.destroy() {
                        error!("Unable to destroy service {}: {}", service_definition, err);
                    }
                }
            }
            Err(_) => {
                error!("Service orchestrator service lock was poisoned; unable to cleanly shutdown")
            }
        }
        self.running.store(false, Ordering::SeqCst);
    }

    fn wait_for_shutdown(&mut self) -> Result<(), InternalError> {
        if let Some(join_handles) = self.join_handles.take() {
            match join_handles.join_all() {
                Ok(results) => {
                    results
                        .into_iter()
                        .filter(Result::is_err)
                        .map(Result::unwrap_err)
                        .for_each(|err| {
                            error!("{}", err);
                        });
                }
                Err(_) => {
                    return Err(crate::error::InternalError::with_message(
                        "Unable to join service processor threads".into(),
                    ));
                }
            }
        }

        Ok(())
    }
}

fn run_incoming_loop(
    incoming_mesh: Mesh,
    incoming_running: Arc<AtomicBool>,
    mut inbound_router: InboundRouter<CircuitMessageType>,
) -> Result<(), OrchestratorError> {
    while incoming_running.load(Ordering::SeqCst) {
        let timeout = Duration::from_secs(TIMEOUT_SEC);
        let message_bytes = match incoming_mesh.recv_timeout(timeout) {
            Ok(envelope) => Vec::from(envelope),
            Err(MeshRecvTimeoutError::Timeout) => continue,
            Err(MeshRecvTimeoutError::Disconnected) => {
                error!("Mesh Disconnected");
                break;
            }
            Err(MeshRecvTimeoutError::PoisonedLock) => {
                error!("Mesh lock was poisoned");
                break;
            }
            Err(MeshRecvTimeoutError::Shutdown) => {
                error!("Mesh has shutdown");
                break;
            }
        };

        let msg: NetworkMessage = Message::parse_from_bytes(&message_bytes)
            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;

        // if a service is waiting on a reply the inbound router will
        // route back the reponse to the service based on the correlation id in
        // the message, otherwise it will be sent to the inbound thread
        match msg.get_message_type() {
            NetworkMessageType::CIRCUIT => {
                let mut circuit_msg: CircuitMessage = Message::parse_from_bytes(&msg.get_payload())
                    .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;

                match circuit_msg.get_message_type() {
                    CircuitMessageType::ADMIN_DIRECT_MESSAGE => {
                        let admin_direct_message: AdminDirectMessage =
                            Message::parse_from_bytes(circuit_msg.get_payload())
                                .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                        inbound_router
                            .route(
                                admin_direct_message.get_correlation_id(),
                                Ok((
                                    CircuitMessageType::ADMIN_DIRECT_MESSAGE,
                                    circuit_msg.take_payload(),
                                )),
                            )
                            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                    }
                    CircuitMessageType::CIRCUIT_DIRECT_MESSAGE => {
                        let direct_message: CircuitDirectMessage =
                            Message::parse_from_bytes(circuit_msg.get_payload())
                                .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                        inbound_router
                            .route(
                                direct_message.get_correlation_id(),
                                Ok((
                                    CircuitMessageType::CIRCUIT_DIRECT_MESSAGE,
                                    circuit_msg.take_payload(),
                                )),
                            )
                            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                    }
                    CircuitMessageType::SERVICE_CONNECT_RESPONSE => {
                        let response: ServiceConnectResponse =
                            Message::parse_from_bytes(circuit_msg.get_payload())
                                .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                        inbound_router
                            .route(
                                response.get_correlation_id(),
                                Ok((
                                    CircuitMessageType::SERVICE_CONNECT_RESPONSE,
                                    circuit_msg.take_payload(),
                                )),
                            )
                            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                    }
                    CircuitMessageType::SERVICE_DISCONNECT_RESPONSE => {
                        let response: ServiceDisconnectResponse =
                            Message::parse_from_bytes(circuit_msg.get_payload())
                                .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                        inbound_router
                            .route(
                                response.get_correlation_id(),
                                Ok((
                                    CircuitMessageType::SERVICE_DISCONNECT_RESPONSE,
                                    circuit_msg.take_payload(),
                                )),
                            )
                            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                    }
                    CircuitMessageType::CIRCUIT_ERROR_MESSAGE => {
                        let response: CircuitError =
                            Message::parse_from_bytes(circuit_msg.get_payload())
                                .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
                        warn!("Received circuit error message {:?}", response);
                    }
                    msg_type => warn!("Received unimplemented message: {:?}", msg_type),
                }
            }
            NetworkMessageType::NETWORK_HEARTBEAT => trace!("Received network heartbeat"),
            _ => warn!("Received unimplemented message"),
        }
    }

    Ok(())
}

fn run_inbound_loop(
    services: Arc<Mutex<HashMap<ServiceDefinition, ManagedService>>>,
    inbound_receiver: Receiver<Result<(CircuitMessageType, Vec<u8>), channel::RecvError>>,
    inbound_running: Arc<AtomicBool>,
) -> Result<(), OrchestratorError> {
    let timeout = Duration::from_secs(TIMEOUT_SEC);
    while inbound_running.load(Ordering::SeqCst) {
        let service_message = match inbound_receiver.recv_timeout(timeout) {
            Ok(msg) => msg,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(err) => {
                debug!("inbound sender dropped; ending inbound message thread");
                return Err(OrchestratorError::Internal(Box::new(err)));
            }
        }
        .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;

        match service_message {
            (CircuitMessageType::ADMIN_DIRECT_MESSAGE, msg) => {
                let mut admin_direct_message: AdminDirectMessage = Message::parse_from_bytes(&msg)
                    .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;

                let services = services
                    .lock()
                    .map_err(|_| OrchestratorError::LockPoisoned)?;

                match services.iter().find_map(|(service_def, managed_service)| {
                    if service_def.circuit == admin_direct_message.get_circuit()
                        && service_def.service_id == admin_direct_message.get_recipient()
                    {
                        Some(&managed_service.service)
                    } else {
                        None
                    }
                }) {
                    Some(service) => {
                        let msg_context = ServiceMessageContext {
                            sender: admin_direct_message.take_sender(),
                            circuit: admin_direct_message.take_circuit(),
                            correlation_id: admin_direct_message.take_correlation_id(),
                        };

                        if let Err(err) =
                            service.handle_message(admin_direct_message.get_payload(), &msg_context)
                        {
                            error!("unable to handle admin direct message: {}", err);
                        }
                    }
                    None => warn!(
                        "Service with id {} does not exist on circuit {}; ignoring message",
                        admin_direct_message.get_recipient(),
                        admin_direct_message.get_circuit(),
                    ),
                }
            }
            (CircuitMessageType::CIRCUIT_DIRECT_MESSAGE, msg) => {
                let mut circuit_direct_message: CircuitDirectMessage =
                    Message::parse_from_bytes(&msg)
                        .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;

                let services = services
                    .lock()
                    .map_err(|_| OrchestratorError::LockPoisoned)?;

                match services.iter().find_map(|(service_def, managed_service)| {
                    if service_def.circuit == circuit_direct_message.get_circuit()
                        && service_def.service_id == circuit_direct_message.get_recipient()
                    {
                        Some(&managed_service.service)
                    } else {
                        None
                    }
                }) {
                    Some(service) => {
                        let msg_context = ServiceMessageContext {
                            sender: circuit_direct_message.take_sender(),
                            circuit: circuit_direct_message.take_circuit(),
                            correlation_id: circuit_direct_message.take_correlation_id(),
                        };

                        if let Err(err) = service
                            .handle_message(circuit_direct_message.get_payload(), &msg_context)
                        {
                            error!("unable to handle direct message: {}", err);
                        }
                    }
                    None => warn!(
                        "Service with id {} does not exist on circuit {}; ignoring message",
                        circuit_direct_message.get_recipient(),
                        circuit_direct_message.get_circuit(),
                    ),
                }
            }
            (msg_type, _) => warn!(
                "Received message ({:?}) that does not have a correlation id",
                msg_type
            ),
        }
    }
    Ok(())
}

fn run_outgoing_loop(
    outgoing_mesh: Mesh,
    outgoing_running: Arc<AtomicBool>,
    outgoing_receiver: Receiver<Vec<u8>>,
    mesh_id: String,
) -> Result<(), OrchestratorError> {
    while outgoing_running.load(Ordering::SeqCst) {
        let timeout = Duration::from_secs(TIMEOUT_SEC);
        let message_bytes = match outgoing_receiver.recv_timeout(timeout) {
            Ok(msg) => msg,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(err) => {
                error!("channel dropped while handling outgoing messages: {}", err);
                break;
            }
        };

        // Send message to splinter node
        outgoing_mesh
            .send(Envelope::new(mesh_id.to_string(), message_bytes))
            .map_err(|err| OrchestratorError::Internal(Box::new(err)))?;
    }
    Ok(())
}
