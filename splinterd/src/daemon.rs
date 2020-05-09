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

#[cfg(feature = "service-arg-validation")]
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(feature = "health")]
use health::HealthService;
use splinter::admin::rest_api::CircuitResourceProvider;
use splinter::admin::service::{admin_service_id, AdminService};
#[cfg(feature = "biome")]
use splinter::biome::rest_api::{BiomeRestResourceManager, BiomeRestResourceManagerBuilder};
#[cfg(feature = "biome-key-management")]
use splinter::biome::DieselKeyStore;
#[cfg(feature = "biome")]
use splinter::biome::DieselUserStore;
#[cfg(feature = "biome-credentials")]
use splinter::biome::{DieselCredentialsStore, DieselRefreshTokenStore};
use splinter::circuit::directory::CircuitDirectory;
use splinter::circuit::handlers::{
    AdminDirectMessageHandler, CircuitDirectMessageHandler, CircuitErrorHandler,
    CircuitMessageHandler, ServiceConnectRequestHandler, ServiceDisconnectRequestHandler,
};
use splinter::circuit::{SplinterState, SplinterStateError};
#[cfg(feature = "biome")]
use splinter::database::{self, ConnectionPool};
use splinter::keys::insecure::AllowAllKeyPermissionManager;
use splinter::mesh::Mesh;
use splinter::network::auth::handlers::{
    create_authorization_dispatcher, AuthorizationMessageHandler, NetworkAuthGuardHandler,
};
use splinter::network::auth::AuthorizationManager;
use splinter::network::dispatch::{DispatchLoopBuilder, DispatchMessageSender, Dispatcher};
use splinter::network::handlers::{NetworkEchoHandler, NetworkHeartbeatHandler};
use splinter::network::peer::PeerConnector;
use splinter::network::{sender, sender::NetworkMessageSender};
use splinter::network::{ConnectionError, Network, PeerUpdateError, RecvTimeoutError, SendError};
use splinter::orchestrator::{NewOrchestratorError, ServiceOrchestrator};
use splinter::protos::authorization::AuthorizationMessageType;
use splinter::protos::circuit::CircuitMessageType;
use splinter::protos::network::{NetworkMessage, NetworkMessageType};
use splinter::registry::{
    LocalYamlRegistry, RegistryReader, RemoteYamlRegistry, RemoteYamlShutdownHandle, RwRegistry,
    UnifiedRegistry,
};
use splinter::rest_api::{
    Method, Resource, RestApiBuilder, RestApiServerError, RestResourceProvider,
};
#[cfg(feature = "service-arg-validation")]
use splinter::service::scabbard::ScabbardArgValidator;
use splinter::service::scabbard::ScabbardFactory;
#[cfg(feature = "service-arg-validation")]
use splinter::service::validation::ServiceArgValidator;
use splinter::service::{self, ServiceProcessor, ShutdownHandle};
use splinter::signing::sawtooth::SawtoothSecp256k1SignatureVerifier;
use splinter::storage::get_storage;
use splinter::transport::{
    multi::MultiTransport, AcceptError, ConnectError, Connection, Incoming, ListenError, Listener,
    Transport,
};

use crate::routes;

// Recv timeout in secs
const TIMEOUT_SEC: u64 = 2;
const INTERNAL_SERVICE_ADDRESS: &str = "inproc://internal-service";

const ORCHESTRATOR_INCOMING_CAPACITY: usize = 8;
const ORCHESTRATOR_OUTGOING_CAPACITY: usize = 8;
const ORCHESTRATOR_CHANNEL_CAPACITY: usize = 8;

const ADMIN_SERVICE_PROCESSOR_INCOMING_CAPACITY: usize = 8;
const ADMIN_SERVICE_PROCESSOR_OUTGOING_CAPACITY: usize = 8;
const ADMIN_SERVICE_PROCESSOR_CHANNEL_CAPACITY: usize = 8;

#[cfg(feature = "health")]
const HEALTH_SERVICE_PROCESSOR_INCOMING_CAPACITY: usize = 8;
#[cfg(feature = "health")]
const HEALTH_SERVICE_PROCESSOR_OUTGOING_CAPACITY: usize = 8;
#[cfg(feature = "health")]
const HEALTH_SERVICE_PROCESSOR_CHANNEL_CAPACITY: usize = 8;

type ServiceJoinHandle = service::JoinHandles<Result<(), service::error::ServiceProcessorError>>;

pub struct SplinterDaemon {
    storage_location: String,
    proposals_location: String,
    registry_directory: String,
    service_endpoint: String,
    network_endpoints: Vec<String>,
    advertised_endpoints: Vec<String>,
    initial_peers: Vec<String>,
    network: Network,
    node_id: String,
    display_name: String,
    rest_api_endpoint: String,
    #[cfg(feature = "database")]
    db_url: Option<String>,
    #[cfg(feature = "biome")]
    enable_biome: bool,
    registries: Vec<String>,
    registry_auto_refresh: u64,
    registry_forced_refresh: u64,
    admin_timeout: Duration,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
}

impl SplinterDaemon {
    pub fn start(&mut self, mut transport: MultiTransport) -> Result<(), StartError> {
        // Setup up ctrlc handling
        let running = Arc::new(AtomicBool::new(true));

        // Load initial state from the configured storage location and create the new
        // SplinterState from the retrieved circuit directory
        let storage = get_storage(&self.storage_location, CircuitDirectory::new)
            .map_err(StartError::StorageError)?;

        let circuit_directory = storage.read().clone();
        let state = SplinterState::new(self.storage_location.to_string(), circuit_directory);

        // set up the listeners on the transport. This will set up listeners for different
        // transports based on the protocol prefix of the endpoint.
        let network_listeners = self
            .network_endpoints
            .iter()
            .map(|endpoint| transport.listen(endpoint))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                StartError::TransportError(format!("Cannot create listener for endpoint: {}", err))
            })?;
        debug!(
            "Listening for peer connections on {:?}",
            network_listeners
                .iter()
                .map(|listener| listener.endpoint())
                .collect::<Vec<_>>(),
        );
        let service_listener = transport.listen(&self.service_endpoint)?;
        debug!(
            "Listening for service connections on {}",
            service_listener.endpoint()
        );
        let internal_service_listener = transport.listen(INTERNAL_SERVICE_ADDRESS)?;

        // Listen for services
        Self::listen_for_services(
            self.network.clone(),
            internal_service_listener,
            vec![
                format!("orchestator::{}", &self.node_id),
                admin_service_id(&self.node_id),
                format!("health::{}", &self.node_id),
            ],
            service_listener,
        );

        let orchestrator_connection =
            transport.connect(INTERNAL_SERVICE_ADDRESS).map_err(|err| {
                StartError::TransportError(format!(
                    "unable to initiate orchestrator connection: {:?}",
                    err
                ))
            })?;

        // set up inproc connections
        let admin_connection = transport.connect(INTERNAL_SERVICE_ADDRESS).map_err(|err| {
            StartError::AdminServiceError(format!(
                "unable to initiate admin service connection: {:?}",
                err
            ))
        })?;

        #[cfg(feature = "health")]
        let health_connection = transport.connect(INTERNAL_SERVICE_ADDRESS).map_err(|err| {
            StartError::HealthServiceError(format!(
                "unable to initiate health service connection: {:?}",
                err
            ))
        })?;

        let peer_connector = PeerConnector::new(self.network.clone(), Box::new(transport));
        let auth_manager = AuthorizationManager::new(self.network.clone(), self.node_id.clone());

        info!("Starting SpinterNode with ID {}", self.node_id);

        let network_shutdown = self.network.shutdown_signaler();
        let network = self.network.clone();
        let network_message_queue = sender::Builder::new()
            .with_network(network)
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!(
                    "Unable to create network message sender: {}",
                    err
                ))
            })?;
        let network_sender = network_message_queue.new_network_sender();
        let sender_shutdown_signaler = network_message_queue.shutdown_signaler();

        // Set up the Circuit dispatcher
        let circuit_dispatcher = set_up_circuit_dispatcher(
            network_sender.clone(),
            &self.node_id,
            &self.network_endpoints,
            state.clone(),
        );
        let circuit_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(circuit_dispatcher)
            .with_thread_name("CircuitDispatchLoop".to_string())
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create circuit dispatch loop: {}", err))
            })?;
        let circuit_dispatch_sender = circuit_dispatch_loop.new_dispatcher_sender();

        let circuit_dispatcher_shutdown = circuit_dispatch_loop.shutdown_signaler();

        // Set up the Auth dispatcher
        let auth_dispatcher =
            create_authorization_dispatcher(auth_manager.clone(), network_sender.clone());

        let auth_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(auth_dispatcher)
            .with_thread_name("AuthorizationDispatchLoop".to_string())
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create auth dispatch loop: {}", err))
            })?;
        let auth_dispatch_sender = auth_dispatch_loop.new_dispatcher_sender();
        let auth_dispatcher_shutdown = auth_dispatch_loop.shutdown_signaler();

        // Set up the Network dispatcher
        let network_dispatcher = set_up_network_dispatcher(
            network_sender,
            &self.node_id,
            auth_manager.clone(),
            circuit_dispatch_sender,
            auth_dispatch_sender,
        );
        let network_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(network_dispatcher)
            .with_thread_name("NetworkDispatchLoop".to_string())
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create network dispatch loop: {}", err))
            })?;

        let network_dispatch_send = network_dispatch_loop.new_dispatcher_sender();
        let network_dispatcher_shutdown = network_dispatch_loop.shutdown_signaler();

        // setup threads to listen on the network ports and add incoming connections to the network
        // these threads will just be dropped on shutdown
        let _ = network_listeners
            .into_iter()
            .map(|mut network_listener| {
                let network_clone = self.network.clone();
                thread::spawn(move || {
                    for connection_result in network_listener.incoming() {
                        let connection = match connection_result {
                            Ok(connection) => connection,
                            Err(AcceptError::ProtocolError(msg)) => {
                                warn!("Failed to accept connection due to {}", msg);
                                continue;
                            }
                            Err(AcceptError::IoError(err)) => {
                                error!("Unable to receive new connections; exiting: {:?}", err);
                                return Err(StartError::TransportError(format!(
                                    "Accept Error: {:?}",
                                    err
                                )));
                            }
                        };
                        debug!("Received connection from {}", connection.remote_endpoint());
                        match network_clone.add_connection(connection) {
                            Ok(peer_id) => debug!("Added connection with ID {}", peer_id),
                            Err(err) => error!("Failed to add connection to network: {}", err),
                        };
                    }
                    Ok(())
                })
            })
            .collect::<Vec<_>>();

        // For provided initial peers, try to connect to them
        for peer in self.initial_peers.iter() {
            if let Err(err) = peer_connector.connect_unidentified_peer(&peer) {
                error!("Connect Error: {}", err);
            }
        }

        // For each node in the circuit_directory, try to connect and add them to the network
        for (node_id, node) in state.nodes()?.iter() {
            if node_id != &self.node_id {
                if !node.endpoints().is_empty() {
                    if let Err(err) = peer_connector.connect_peer(node_id, node.endpoints()) {
                        debug!("Unable to connect to node: {} Error: {:?}", node_id, err);
                    }
                } else {
                    debug!("node {} has no known endpoints", node_id);
                }
            }
        }

        let timeout = Duration::from_secs(TIMEOUT_SEC);

        // start the recv loop
        let main_loop_network = self.network.clone();
        let main_loop_running = running.clone();
        let main_loop_join_handle = thread::Builder::new()
            .name("MainLoop".into())
            .spawn(move || {
                while main_loop_running.load(Ordering::SeqCst) {
                    match main_loop_network.recv_timeout(timeout) {
                        // This is where the message should be dispatched
                        Ok(message) => {
                            let mut msg: NetworkMessage =
                                match protobuf::parse_from_bytes(message.payload()) {
                                    Ok(msg) => msg,
                                    Err(err) => {
                                        warn!("Received invalid network message: {}", err);
                                        continue;
                                    }
                                };

                            trace!("Received message from {}: {:?}", message.peer_id(), msg);
                            match network_dispatch_send.send(
                                msg.get_message_type(),
                                msg.take_payload(),
                                message.peer_id().into(),
                            ) {
                                Ok(()) => (),
                                Err((message_type, _, _)) => {
                                    error!("Unable to dispatch message of type {:?}", message_type)
                                }
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => {
                            // if the receiver has disconnected, shutdown
                            warn!("Received Disconnected Error from Network");
                            break;
                        }
                        Err(RecvTimeoutError::Shutdown) => {
                            // if network has shutdown, shutdown
                            warn!("Received Shutdown from Network");
                            break;
                        }
                        Err(_) => {
                            // Timeout or NoPeerError are ignored
                            continue;
                        }
                    }
                }
                info!("Shutting down");
            })
            .map_err(|_| StartError::ThreadError("Unable to spawn main loop".into()))?;

        let orchestrator = ServiceOrchestrator::new(
            vec![Box::new(ScabbardFactory::new(
                None,
                None,
                None,
                None,
                Box::new(SawtoothSecp256k1SignatureVerifier::new()),
            ))],
            orchestrator_connection,
            ORCHESTRATOR_INCOMING_CAPACITY,
            ORCHESTRATOR_OUTGOING_CAPACITY,
            ORCHESTRATOR_CHANNEL_CAPACITY,
        )?;
        let orchestrator_resources = orchestrator.resources();

        let signature_verifier = SawtoothSecp256k1SignatureVerifier::new();

        let (registry, registry_shutdown) = create_registry(
            &self.registry_directory,
            &self.registries,
            self.registry_auto_refresh,
            self.registry_forced_refresh,
        )?;

        let admin_service = AdminService::new(
            &self.node_id,
            orchestrator,
            #[cfg(feature = "service-arg-validation")]
            {
                let mut validators: HashMap<String, Box<dyn ServiceArgValidator + Send>> =
                    HashMap::new();
                validators.insert("scabbard".into(), Box::new(ScabbardArgValidator));
                validators
            },
            peer_connector,
            Box::new(auth_manager),
            state.clone(),
            Box::new(signature_verifier),
            Box::new(registry.clone_box_as_reader()),
            Box::new(AllowAllKeyPermissionManager),
            &self.proposals_location,
            Some(self.admin_timeout),
        )
        .map_err(|err| {
            StartError::AdminServiceError(format!("unable to create admin service: {}", err))
        })?;

        let node_id = self.node_id.clone();
        let display_name = self.display_name.clone();
        let service_endpoint = self.service_endpoint.clone();
        let network_endpoints = self.network_endpoints.clone();
        let advertised_endpoints = self.advertised_endpoints.clone();

        let circuit_resource_provider =
            CircuitResourceProvider::new(self.node_id.to_string(), state);

        // Allowing unused_mut because rest_api_builder must be mutable if feature biome is enabled
        #[allow(unused_mut)]
        let mut rest_api_builder = RestApiBuilder::new()
            .with_bind(&self.rest_api_endpoint)
            .add_resource(
                Resource::build("/openapi.yml").add_method(Method::Get, routes::get_openapi),
            )
            .add_resource(
                Resource::build("/status").add_method(Method::Get, move |_, _| {
                    routes::get_status(
                        node_id.clone(),
                        display_name.clone(),
                        service_endpoint.clone(),
                        network_endpoints.clone(),
                        advertised_endpoints.clone(),
                    )
                }),
            )
            .add_resources(registry.resources())
            .add_resources(admin_service.resources())
            .add_resources(orchestrator_resources)
            .add_resources(circuit_resource_provider.resources());

        #[cfg(feature = "rest-api-cors")]
        {
            if let Some(list) = &self.whitelist {
                debug!("Whitelisted domains added to CORS");
                rest_api_builder = rest_api_builder.with_whitelist(list.to_vec());
            }
        }

        #[cfg(feature = "biome")]
        {
            if self.enable_biome {
                let db_url = self.db_url.as_ref().ok_or_else(|| {
                    StartError::StorageError(
                        "biome was enabled but the builder failed to require the db URL".into(),
                    )
                })?;
                let biome_resources = build_biome_routes(&db_url)?;
                rest_api_builder = rest_api_builder.add_resources(biome_resources.resources());
            }
        }

        let (rest_api_shutdown_handle, rest_api_join_handle) = rest_api_builder.build()?.run()?;

        let (admin_shutdown_handle, service_processor_join_handle) =
            Self::start_admin_service(admin_connection, admin_service, Arc::clone(&running))?;

        // Allowing possibly redundant clone of `running` since it will be needed again if the
        // `health` feature is enabled
        #[allow(clippy::redundant_clone)]
        let r = running.clone();
        ctrlc::set_handler(move || {
            info!("Received Shutdown");
            r.store(false, Ordering::SeqCst);

            if let Err(err) = admin_shutdown_handle.shutdown() {
                error!("Unable to cleanly shut down Admin service: {}", err);
            }

            if let Err(err) = rest_api_shutdown_handle.shutdown() {
                error!("Unable to cleanly shut down REST API server: {}", err);
            }

            auth_dispatcher_shutdown.shutdown();
            circuit_dispatcher_shutdown.shutdown();
            network_dispatcher_shutdown.shutdown();
            network_shutdown.shutdown();
            sender_shutdown_signaler.shutdown();
            registry_shutdown.shutdown();
        })
        .expect("Error setting Ctrl-C handler");

        #[cfg(feature = "health")]
        {
            let health_service = HealthService::new(&self.node_id);
            let health_service_processor_join_handle =
                start_health_service(health_connection, health_service, Arc::clone(&running))?;

            let _ = health_service_processor_join_handle.join_all();
        }

        main_loop_join_handle
            .join()
            .map_err(|_| StartError::ThreadError("Unable to join main loop".into()))?;

        // Join network sender and dispatcher threads
        let _ = rest_api_join_handle.join();
        let _ = service_processor_join_handle.join_all();

        Ok(())
    }

    fn listen_for_services(
        network: Network,
        mut internal_service_listener: Box<dyn Listener>,
        internal_service_peer_ids: Vec<String>,
        mut external_service_listener: Box<dyn Listener>,
    ) {
        // this thread will just be dropped on shutdown
        let _ = thread::spawn(move || {
            // accept the internal service connections
            for service_peer_id in internal_service_peer_ids.into_iter() {
                match internal_service_listener.incoming().next() {
                    Some(Ok(connection)) => {
                        if let Err(err) = network.add_peer(service_peer_id.clone(), connection) {
                            error!("Unable to add peer {}: {}", service_peer_id, err);
                        }
                    }
                    Some(Err(err)) => {
                        return Err(StartError::TransportError(format!(
                            "Accept Error: {:?}",
                            err
                        )));
                    }
                    None => {}
                }
            }

            for connection_result in external_service_listener.incoming() {
                let connection = match connection_result {
                    Ok(connection) => connection,
                    Err(err) => {
                        return Err(StartError::TransportError(format!(
                            "Accept Error: {:?}",
                            err
                        )));
                    }
                };
                debug!(
                    "Received service connection from {}",
                    connection.remote_endpoint()
                );
                if let Err(err) = network.add_connection(connection) {
                    error!("Unable to add inbound service connection: {}", err);
                }
            }
            Ok(())
        });
    }

    fn start_admin_service(
        connection: Box<dyn Connection>,
        admin_service: AdminService,
        running: Arc<AtomicBool>,
    ) -> Result<(ShutdownHandle, ServiceJoinHandle), StartError> {
        let start_admin: std::thread::JoinHandle<
            Result<(ShutdownHandle, ServiceJoinHandle), StartError>,
        > = thread::spawn(move || {
            let mut admin_service_processor = ServiceProcessor::new(
                connection,
                "admin".into(),
                ADMIN_SERVICE_PROCESSOR_INCOMING_CAPACITY,
                ADMIN_SERVICE_PROCESSOR_OUTGOING_CAPACITY,
                ADMIN_SERVICE_PROCESSOR_CHANNEL_CAPACITY,
                running,
            )
            .map_err(|err| {
                StartError::AdminServiceError(format!(
                    "unable to create admin service processor: {}",
                    err
                ))
            })?;

            admin_service_processor
                .add_service(Box::new(admin_service))
                .map_err(|err| {
                    StartError::AdminServiceError(format!(
                        "unable to add admin service to processor: {}",
                        err
                    ))
                })?;

            admin_service_processor.start().map_err(|err| {
                StartError::AdminServiceError(format!("unable to start service processor: {}", err))
            })
        });

        start_admin.join().map_err(|_| {
            StartError::AdminServiceError(
                "unable to start admin service, due to thread join error".into(),
            )
        })?
    }
}

#[cfg(feature = "health")]
fn start_health_service(
    connection: Box<dyn Connection>,
    health_service: HealthService,
    running: Arc<AtomicBool>,
) -> Result<service::JoinHandles<Result<(), service::error::ServiceProcessorError>>, StartError> {
    let start_health_service: std::thread::JoinHandle<
        Result<service::JoinHandles<Result<(), service::error::ServiceProcessorError>>, StartError>,
    > = thread::spawn(move || {
        let mut health_service_processor = ServiceProcessor::new(
            connection,
            "health".into(),
            HEALTH_SERVICE_PROCESSOR_INCOMING_CAPACITY,
            HEALTH_SERVICE_PROCESSOR_OUTGOING_CAPACITY,
            HEALTH_SERVICE_PROCESSOR_CHANNEL_CAPACITY,
            running,
        )
        .map_err(|err| {
            StartError::HealthServiceError(format!(
                "unable to create health service processor: {}",
                err
            ))
        })?;

        health_service_processor
            .add_service(Box::new(health_service))
            .map_err(|err| {
                StartError::HealthServiceError(format!(
                    "unable to add health service to processor: {}",
                    err
                ))
            })?;

        health_service_processor
            .start()
            .map(|(_, join_handles)| join_handles)
            .map_err(|err| {
                StartError::HealthServiceError(format!(
                    "unable to health service processor: {}",
                    err
                ))
            })
    });

    start_health_service.join().map_err(|_| {
        StartError::HealthServiceError(
            "unable to start health service, due to thread join error".into(),
        )
    })?
}

#[cfg(feature = "biome")]
fn build_biome_routes(db_url: &str) -> Result<BiomeRestResourceManager, StartError> {
    info!("Adding biome routes");
    let connection_pool: ConnectionPool =
        database::ConnectionPool::new_pg(db_url).map_err(|err| {
            StartError::RestApiError(format!(
                "Unable to connect to the Splinter database: {}",
                err
            ))
        })?;
    let mut biome_rest_provider_builder: BiomeRestResourceManagerBuilder = Default::default();
    biome_rest_provider_builder =
        biome_rest_provider_builder.with_user_store(DieselUserStore::new(connection_pool.clone()));
    #[cfg(feature = "biome-credentials")]
    {
        biome_rest_provider_builder = biome_rest_provider_builder
            .with_refresh_token_store(DieselRefreshTokenStore::new(connection_pool.clone()));
        biome_rest_provider_builder = biome_rest_provider_builder
            .with_credentials_store(DieselCredentialsStore::new(connection_pool.clone()));
    }
    #[cfg(feature = "biome-key-management")]
    {
        biome_rest_provider_builder =
            biome_rest_provider_builder.with_key_store(DieselKeyStore::new(connection_pool))
    }
    let biome_rest_provider = biome_rest_provider_builder.build().map_err(|err| {
        StartError::RestApiError(format!("Unable to build Biome REST routes: {}", err))
    })?;

    Ok(biome_rest_provider)
}

#[derive(Default)]
pub struct SplinterDaemonBuilder {
    storage_location: Option<String>,
    proposals_location: Option<String>,
    registry_directory: Option<String>,
    service_endpoint: Option<String>,
    network_endpoints: Option<Vec<String>>,
    advertised_endpoints: Option<Vec<String>>,
    initial_peers: Option<Vec<String>>,
    node_id: Option<String>,
    display_name: Option<String>,
    rest_api_endpoint: Option<String>,
    #[cfg(feature = "database")]
    db_url: Option<String>,
    #[cfg(feature = "biome")]
    enable_biome: bool,
    registries: Vec<String>,
    registry_auto_refresh: Option<u64>,
    registry_forced_refresh: Option<u64>,
    heartbeat: Option<u64>,
    admin_timeout: Duration,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
}

impl SplinterDaemonBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_storage_location(mut self, value: String) -> Self {
        self.storage_location = Some(value);
        self
    }

    pub fn with_proposals_location(mut self, value: String) -> Self {
        self.proposals_location = Some(value);
        self
    }

    pub fn with_registry_directory(mut self, value: String) -> Self {
        self.registry_directory = Some(value);
        self
    }

    pub fn with_service_endpoint(mut self, value: String) -> Self {
        self.service_endpoint = Some(value);
        self
    }

    pub fn with_network_endpoints(mut self, value: Vec<String>) -> Self {
        self.network_endpoints = Some(value);
        self
    }

    pub fn with_advertised_endpoints(mut self, value: Vec<String>) -> Self {
        self.advertised_endpoints = Some(value);
        self
    }

    pub fn with_initial_peers(mut self, value: Vec<String>) -> Self {
        self.initial_peers = Some(value);
        self
    }

    pub fn with_node_id(mut self, value: String) -> Self {
        self.node_id = Some(value);
        self
    }

    pub fn with_display_name(mut self, value: String) -> Self {
        self.display_name = Some(value);
        self
    }

    pub fn with_rest_api_endpoint(mut self, value: String) -> Self {
        self.rest_api_endpoint = Some(value);
        self
    }

    #[cfg(feature = "database")]
    pub fn with_db_url(mut self, value: Option<String>) -> Self {
        self.db_url = value;
        self
    }

    #[cfg(feature = "biome")]
    pub fn enable_biome(mut self, enabled: bool) -> Self {
        self.enable_biome = enabled;
        self
    }

    pub fn with_registries(mut self, registries: Vec<String>) -> Self {
        self.registries = registries;
        self
    }

    pub fn with_registry_auto_refresh(mut self, value: u64) -> Self {
        self.registry_auto_refresh = Some(value);
        self
    }

    pub fn with_registry_forced_refresh(mut self, value: u64) -> Self {
        self.registry_forced_refresh = Some(value);
        self
    }

    pub fn with_heartbeat(mut self, value: u64) -> Self {
        self.heartbeat = Some(value);
        self
    }

    pub fn with_admin_timeout(mut self, value: Duration) -> Self {
        self.admin_timeout = value;
        self
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn with_whitelist(mut self, value: Option<Vec<String>>) -> Self {
        self.whitelist = value;
        self
    }

    pub fn build(self) -> Result<SplinterDaemon, CreateError> {
        let heartbeat = self.heartbeat.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: heartbeat".to_string())
        })?;

        let mesh = Mesh::new(512, 128);
        let network = Network::new(mesh, heartbeat)
            .map_err(|err| CreateError::NetworkError(err.to_string()))?;

        let storage_location = self.storage_location.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: storage_location".to_string())
        })?;

        let proposals_location = self.proposals_location.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: proposals_location".to_string())
        })?;

        let registry_directory = self.registry_directory.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: registry_directory".to_string())
        })?;

        let service_endpoint = self.service_endpoint.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: service_location".to_string())
        })?;

        let network_endpoints = self.network_endpoints.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: network_endpoints".to_string())
        })?;

        let advertised_endpoints = self.advertised_endpoints.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: advertised_endpoints".to_string())
        })?;

        let initial_peers = self.initial_peers.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: initial_peers".to_string())
        })?;

        let node_id = self.node_id.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: node_id".to_string())
        })?;

        let display_name = self.display_name.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: display_name".to_string())
        })?;

        let rest_api_endpoint = self.rest_api_endpoint.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: rest_api_endpoint".to_string())
        })?;

        #[cfg(feature = "database")]
        let db_url = self.db_url;

        #[cfg(feature = "biome")]
        {
            if self.enable_biome && db_url.is_none() {
                return Err(CreateError::MissingRequiredField(
                    "db_url is required to enable biome features.".to_string(),
                ));
            }
        }

        let registry_auto_refresh = self.registry_auto_refresh.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: registry_auto_refresh".to_string())
        })?;

        let registry_forced_refresh = self.registry_forced_refresh.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: registry_forced_refresh".to_string())
        })?;

        Ok(SplinterDaemon {
            storage_location,
            proposals_location,
            service_endpoint,
            network_endpoints,
            advertised_endpoints,
            initial_peers,
            network,
            node_id,
            display_name,
            rest_api_endpoint,
            #[cfg(feature = "database")]
            db_url,
            #[cfg(feature = "biome")]
            enable_biome: self.enable_biome,
            registries: self.registries,
            registry_auto_refresh,
            registry_forced_refresh,
            registry_directory,
            admin_timeout: self.admin_timeout,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
        })
    }
}

fn set_up_network_dispatcher(
    network_sender: NetworkMessageSender,
    node_id: &str,
    auth_manager: AuthorizationManager,
    circuit_sender: DispatchMessageSender<CircuitMessageType>,
    auth_sender: DispatchMessageSender<AuthorizationMessageType>,
) -> Dispatcher<NetworkMessageType> {
    let mut dispatcher = Dispatcher::<NetworkMessageType>::new(Box::new(network_sender));

    let network_echo_handler = NetworkEchoHandler::new(node_id.to_string());
    dispatcher.set_handler(Box::new(NetworkAuthGuardHandler::new(
        auth_manager.clone(),
        Box::new(network_echo_handler),
    )));

    let network_heartbeat_handler = NetworkHeartbeatHandler::new();
    // do not add auth guard
    dispatcher.set_handler(Box::new(network_heartbeat_handler));

    let circuit_message_handler = CircuitMessageHandler::new(circuit_sender);
    dispatcher.set_handler(Box::new(NetworkAuthGuardHandler::new(
        auth_manager,
        Box::new(circuit_message_handler),
    )));

    let auth_message_handler = AuthorizationMessageHandler::new(auth_sender);
    dispatcher.set_handler(Box::new(auth_message_handler));

    dispatcher
}

fn set_up_circuit_dispatcher(
    network_sender: NetworkMessageSender,
    node_id: &str,
    endpoints: &[String],
    state: SplinterState,
) -> Dispatcher<CircuitMessageType> {
    let mut dispatcher = Dispatcher::<CircuitMessageType>::new(Box::new(network_sender));

    let service_connect_request_handler =
        ServiceConnectRequestHandler::new(node_id.to_string(), endpoints.to_vec(), state.clone());
    dispatcher.set_handler(Box::new(service_connect_request_handler));

    let service_disconnect_request_handler = ServiceDisconnectRequestHandler::new(state.clone());
    dispatcher.set_handler(Box::new(service_disconnect_request_handler));

    let direct_message_handler =
        CircuitDirectMessageHandler::new(node_id.to_string(), state.clone());
    dispatcher.set_handler(Box::new(direct_message_handler));

    let circuit_error_handler = CircuitErrorHandler::new(node_id.to_string(), state.clone());
    dispatcher.set_handler(Box::new(circuit_error_handler));

    // Circuit Admin handlers
    let admin_direct_message_handler = AdminDirectMessageHandler::new(node_id.to_string(), state);
    dispatcher.set_handler(Box::new(admin_direct_message_handler));

    dispatcher
}

fn create_registry(
    registry_directory: &str,
    registries: &[String],
    auto_refresh_interval: u64,
    forced_refresh_interval: u64,
) -> Result<(Box<dyn RwRegistry>, RegistryShutdownHandle), StartError> {
    let mut registry_shutdown_handle = RegistryShutdownHandle::new();

    let local_registry_path = Path::new(registry_directory)
        .join("local_registry.yaml")
        .to_str()
        .expect("path built from &str cannot be invalid")
        .to_string();
    debug!(
        "Creating local registry with registry file: {:?}",
        local_registry_path
    );
    let local_registry = Box::new(LocalYamlRegistry::new(&local_registry_path).map_err(|err| {
        StartError::RegistryError(format!(
            "Failed to initialize local LocalYamlRegistry: {}",
            err
        ))
    })?);

    let read_only_registries = registries
        .iter()
        .filter_map(|registry| {
            let (scheme, path) = parse_registry_arg(registry)
                .map_err(|err| error!("Failed to parse registry argument: {}", err))
                .ok()?;

            if scheme == "file" {
                debug!(
                    "Attempting to add local read-only registry from file: {}",
                    path
                );
                match LocalYamlRegistry::new(path) {
                    Ok(registry) => Some(Box::new(registry) as Box<dyn RegistryReader>),
                    Err(err) => {
                        error!(
                            "Failed to add read-only LocalYamlRegistry '{}': {}",
                            path, err
                        );
                        None
                    }
                }
            } else if scheme == "http" || scheme == "https" {
                debug!(
                    "Attempting to add remote read-only registry from URL: {}",
                    registry
                );
                let auto_refresh_interval = if auto_refresh_interval != 0 {
                    Some(Duration::from_secs(auto_refresh_interval))
                } else {
                    None
                };
                let forced_refresh_interval = if forced_refresh_interval != 0 {
                    Some(Duration::from_secs(forced_refresh_interval))
                } else {
                    None
                };
                match RemoteYamlRegistry::new(
                    registry,
                    registry_directory,
                    auto_refresh_interval,
                    forced_refresh_interval,
                ) {
                    Ok(registry) => {
                        registry_shutdown_handle
                            .add_remote_yaml_shutdown_handle(registry.shutdown_handle());
                        Some(Box::new(registry) as Box<dyn RegistryReader>)
                    }
                    Err(err) => {
                        error!(
                            "Failed to add read-only RemoteYamlRegistry '{}': {}",
                            registry, err
                        );
                        None
                    }
                }
            } else {
                error!(
                    "Invalid registry provided ({}): must be valid 'file://' URI",
                    registry
                );
                None
            }
        })
        .collect();

    let unified_registry = Box::new(UnifiedRegistry::new(local_registry, read_only_registries));

    Ok((unified_registry, registry_shutdown_handle))
}

fn parse_registry_arg(registry: &str) -> Result<(&str, &str), &str> {
    let mut iter = registry.splitn(2, "://");
    let scheme = iter
        .next()
        .expect("str::split cannot return an empty iterator");
    let path = iter.next().ok_or_else(|| "No URI scheme provided")?;
    Ok((scheme, path))
}

#[derive(Default)]
struct RegistryShutdownHandle {
    remote_yaml_shutdown_handles: Vec<RemoteYamlShutdownHandle>,
}

impl RegistryShutdownHandle {
    fn new() -> Self {
        Self::default()
    }

    fn add_remote_yaml_shutdown_handle(&mut self, handle: RemoteYamlShutdownHandle) {
        self.remote_yaml_shutdown_handles.push(handle);
    }

    fn shutdown(&self) {
        self.remote_yaml_shutdown_handles
            .iter()
            .for_each(|handle| handle.shutdown());
    }
}

#[derive(Debug)]
pub enum CreateError {
    MissingRequiredField(String),
    NetworkError(String),
}

impl Error for CreateError {}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CreateError::MissingRequiredField(msg) => write!(f, "missing required field: {}", msg),
            CreateError::NetworkError(msg) => write!(f, "network raised an error: {}", msg),
        }
    }
}

#[derive(Debug)]
pub enum StartError {
    TransportError(String),
    NetworkError(String),
    StorageError(String),
    ProtocolError(String),
    RestApiError(String),
    RegistryError(String),
    AdminServiceError(String),
    #[cfg(feature = "health")]
    HealthServiceError(String),
    OrchestratorError(String),
    ThreadError(String),
    StateError(String),
}

impl Error for StartError {}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StartError::TransportError(msg) => write!(f, "transport returned an error: {}", msg),
            StartError::NetworkError(msg) => write!(f, "network returned an error: {}", msg),
            StartError::StorageError(msg) => write!(f, "unable to set up storage: {}", msg),
            StartError::ProtocolError(msg) => write!(f, "unable to parse protocol: {}", msg),
            StartError::RestApiError(msg) => write!(f, "REST API encountered an error: {}", msg),
            StartError::RegistryError(msg) => write!(f, "unable to setup registry: {}", msg),
            StartError::AdminServiceError(msg) => {
                write!(f, "the admin service encountered an error: {}", msg)
            }
            #[cfg(feature = "health")]
            StartError::HealthServiceError(msg) => {
                write!(f, "the health service encountered an error: {}", msg)
            }
            StartError::OrchestratorError(msg) => {
                write!(f, "the orchestrator encountered an error: {}", msg)
            }
            StartError::ThreadError(msg) => write!(f, "a thread encountered an error: {}", msg),
            StartError::StateError(msg) => write!(f, "{}", msg),
        }
    }
}

impl From<RestApiServerError> for StartError {
    fn from(rest_api_error: RestApiServerError) -> Self {
        StartError::RestApiError(rest_api_error.to_string())
    }
}

impl From<ListenError> for StartError {
    fn from(listen_error: ListenError) -> Self {
        StartError::TransportError(format!("Listen Error: {:?}", listen_error))
    }
}

impl From<AcceptError> for StartError {
    fn from(accept_error: AcceptError) -> Self {
        StartError::TransportError(format!("Accept Error: {:?}", accept_error))
    }
}

impl From<ConnectError> for StartError {
    fn from(connect_error: ConnectError) -> Self {
        StartError::TransportError(format!("Connect Error: {:?}", connect_error))
    }
}

impl From<ConnectionError> for StartError {
    fn from(connection_error: ConnectionError) -> Self {
        StartError::NetworkError(connection_error.to_string())
    }
}

impl From<SendError> for StartError {
    fn from(send_error: SendError) -> Self {
        StartError::NetworkError(send_error.to_string())
    }
}

impl From<PeerUpdateError> for StartError {
    fn from(update_error: PeerUpdateError) -> Self {
        StartError::NetworkError(update_error.to_string())
    }
}

impl From<protobuf::ProtobufError> for StartError {
    fn from(err: protobuf::ProtobufError) -> Self {
        StartError::ProtocolError(err.to_string())
    }
}

impl From<NewOrchestratorError> for StartError {
    fn from(err: NewOrchestratorError) -> Self {
        StartError::OrchestratorError(format!("failed to create new orchestrator: {}", err))
    }
}

impl From<SplinterStateError> for StartError {
    fn from(err: SplinterStateError) -> Self {
        StartError::StateError(err.context())
    }
}
