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

pub mod builder;
mod error;
mod registry;
mod store;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
#[cfg(feature = "authorization-handler-allow-keys")]
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread;
use std::time::Duration;

use cylinder::{secp256k1::Secp256k1Context, Signer, SigningError, VerifierFactory};
#[cfg(feature = "health-service")]
use health::HealthService;
use scabbard::service::ScabbardArgValidator;
use scabbard::service::ScabbardFactoryBuilder;
use splinter::admin::rest_api::CircuitResourceProvider;
use splinter::admin::service::{admin_service_id, AdminService, AdminServiceBuilder};
#[cfg(feature = "biome-credentials")]
use splinter::biome::credentials::rest_api::BiomeCredentialsRestResourceProviderBuilder;
#[cfg(feature = "biome-key-management")]
use splinter::biome::key_management::rest_api::BiomeKeyManagementRestResourceProvider;
#[cfg(feature = "biome-profile")]
use splinter::biome::profile::rest_api::BiomeProfileRestResourceProvider;
use splinter::circuit::handlers::{
    AdminDirectMessageHandler, CircuitDirectMessageHandler, CircuitErrorHandler,
    CircuitMessageHandler, ServiceConnectRequestHandler, ServiceDisconnectRequestHandler,
};
use splinter::circuit::routing::{memory::RoutingTable, RoutingTableReader, RoutingTableWriter};
use splinter::keys::insecure::AllowAllKeyPermissionManager;
use splinter::mesh::Mesh;
use splinter::network::auth::AuthorizationManager;
use splinter::network::connection_manager::{
    authorizers::Authorizers, authorizers::InprocAuthorizer, ConnectionManager, Connector,
};
use splinter::network::dispatch::{
    dispatch_channel, DispatchLoopBuilder, DispatchMessageSender, Dispatcher,
};
use splinter::network::handlers::{NetworkEchoHandler, NetworkHeartbeatHandler};
use splinter::orchestrator::ServiceOrchestratorBuilder;
use splinter::peer::interconnect::NetworkMessageSender;
use splinter::peer::interconnect::PeerInterconnectBuilder;
use splinter::peer::PeerAuthorizationToken;
use splinter::peer::PeerManager;
use splinter::protos::circuit::CircuitMessageType;
use splinter::protos::network::NetworkMessageType;
use splinter::public_key::PublicKey;
use splinter::registry::{
    LocalYamlRegistry, RegistryReader, RemoteYamlRegistry, RwRegistry, UnifiedRegistry,
};
#[cfg(feature = "authorization-handler-allow-keys")]
use splinter::rest_api::auth::authorization::allow_keys::AllowKeysAuthorizationHandler;
#[cfg(feature = "authorization-handler-maintenance")]
use splinter::rest_api::auth::authorization::maintenance::MaintenanceModeAuthorizationHandler;
#[cfg(feature = "authorization-handler-rbac")]
use splinter::rest_api::auth::authorization::rbac::{
    rest_api::RoleBasedAuthorizationResourceProvider, RoleBasedAuthorizationHandler,
};
#[cfg(any(
    feature = "authorization-handler-rbac",
    feature = "authorization-handler-maintenance",
    feature = "authorization-handler-allow-keys"
))]
use splinter::rest_api::auth::authorization::AuthorizationHandler;
#[cfg(feature = "authorization")]
use splinter::rest_api::auth::authorization::Permission;
#[cfg(feature = "oauth")]
use splinter::rest_api::OAuthConfig;
use splinter::rest_api::{AuthConfig, Method, Resource, RestApiBuilder, RestResourceProvider};
use splinter::service;
use splinter::service::validation::ServiceArgValidator;
use splinter::threading::lifecycle::ShutdownHandle;
use splinter::transport::{
    inproc::InprocTransport, multi::MultiTransport, AcceptError, Connection, Incoming, Listener,
    Transport,
};

use crate::node_id::get_node_id;
use crate::routes;

pub use error::StartError;
use registry::RegistryShutdownHandle;
pub use store::ConnectionUri;

const ADMIN_SERVICE_PROCESSOR_INCOMING_CAPACITY: usize = 8;
const ADMIN_SERVICE_PROCESSOR_OUTGOING_CAPACITY: usize = 8;
const ADMIN_SERVICE_PROCESSOR_CHANNEL_CAPACITY: usize = 8;

#[cfg(feature = "health-service")]
const HEALTH_SERVICE_PROCESSOR_INCOMING_CAPACITY: usize = 8;
#[cfg(feature = "health-service")]
const HEALTH_SERVICE_PROCESSOR_OUTGOING_CAPACITY: usize = 8;
#[cfg(feature = "health-service")]
const HEALTH_SERVICE_PROCESSOR_CHANNEL_CAPACITY: usize = 8;

pub struct SplinterDaemon {
    #[cfg(feature = "authorization-handler-allow-keys")]
    config_dir: String,
    state_dir: String,
    #[cfg(feature = "service-endpoint")]
    service_endpoint: String,
    network_endpoints: Vec<String>,
    advertised_endpoints: Vec<String>,
    initial_peers: Vec<String>,
    mesh: Mesh,
    node_id: Option<String>,
    display_name: Option<String>,
    rest_api_endpoint: String,
    #[cfg(feature = "https-bind")]
    rest_api_ssl_settings: Option<(String, String)>,
    db_url: ConnectionUri,
    registries: Vec<String>,
    registry_auto_refresh: u64,
    registry_forced_refresh: u64,
    admin_timeout: Duration,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "biome-credentials")]
    enable_biome_credentials: bool,
    #[cfg(feature = "oauth")]
    oauth_provider: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_id: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_secret: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_redirect_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_auth_params: Option<Vec<(String, String)>>,
    #[cfg(feature = "oauth")]
    oauth_openid_scopes: Option<Vec<String>>,
    heartbeat: u64,
    strict_ref_counts: bool,
    signers: Vec<Box<dyn Signer>>,
    peering_token: PeerAuthorizationToken,
    #[cfg(feature = "config-allow-keys")]
    allow_keys_file: String,
    enable_lmdb_state: bool,
    enable_state_autocleanup: bool,
}

impl SplinterDaemon {
    pub fn start(&mut self, mut transport: MultiTransport) -> Result<(), StartError> {
        // Setup up ctrlc handling
        let running = Arc::new(AtomicBool::new(true));

        let mut service_transport = InprocTransport::default();
        transport.add_transport(Box::new(service_transport.clone()));

        let connection_pool = store::create_connection_pool(&self.db_url).map_err(|err| {
            StartError::StorageError(format!("Failed to initialize connection pool: {}", err))
        })?;
        let store_factory = store::create_store_factory(&connection_pool).map_err(|err| {
            StartError::StorageError(format!("Failed to initialize store factory: {}", err))
        })?;

        let circuits_location = Path::new(&self.state_dir).join("circuits.yaml");
        let proposals_location = Path::new(&self.state_dir).join("circuit_proposals.yaml");

        let circuits_location_exists = circuits_location.exists();
        let proposals_location_exists = proposals_location.exists();

        if circuits_location_exists || proposals_location_exists {
            if circuits_location_exists {
                error!(
                    "Found outdated circuit state file: {}",
                    fs::canonicalize(&circuits_location)
                        .unwrap_or(circuits_location)
                        .to_string_lossy()
                );
            }

            if proposals_location_exists {
                error!(
                    "Found outdated proposals state file: {}",
                    fs::canonicalize(&proposals_location)
                        .unwrap_or(proposals_location)
                        .to_string_lossy()
                );
            }

            return Err(StartError::StorageError(format!(
                "Run the `splinter upgrade` command to update outdated state files to \
                        a Splinter {}.{} database",
                env!("CARGO_PKG_VERSION_MAJOR", "unknown"),
                env!("CARGO_PKG_VERSION_MINOR", "unknown")
            )));
        }

        let table = RoutingTable::default();
        let routing_reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
        let routing_writer: Box<dyn RoutingTableWriter> = Box::new(table);

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

        #[cfg(feature = "service-endpoint")]
        let service_listener = transport.listen(&self.service_endpoint)?;
        #[cfg(feature = "service-endpoint")]
        debug!(
            "Listening for service connections on {}",
            service_listener.endpoint()
        );

        let internal_service_listeners = vec![
            transport.listen("inproc://admin-service")?,
            transport.listen("inproc://orchestator")?,
            #[cfg(feature = "health-service")]
            transport.listen("inproc://health_service")?,
        ];

        let secp256k1_context: Box<dyn VerifierFactory> = Box::new(Secp256k1Context::new());
        let admin_service_verifier = secp256k1_context.new_verifier();
        let auth_config_verifier = secp256k1_context.new_verifier();
        let signing_context = Arc::new(Mutex::new(secp256k1_context));
        let node_id: String = get_node_id(
            self.node_id.as_ref().map(|s| s.to_string()),
            store_factory.get_node_id_store(),
        )?;

        info!("Starting SpinterNode with ID {}", &node_id);
        let authorization_manager = AuthorizationManager::new(
            node_id.to_string(),
            self.signers.clone(),
            signing_context.clone(),
        )
        .map_err(|err| {
            StartError::NetworkError(format!("Unable to create authorization manager: {}", err))
        })?;

        // Allowing unused_mut because inproc_ids must be mutable if feature health is enabled
        #[allow(unused_mut)]
        let mut inproc_ids = vec![
            (
                "inproc://orchestator".to_string(),
                format!("orchestator::{}", &node_id),
            ),
            (
                "inproc://admin-service".to_string(),
                admin_service_id(&node_id),
            ),
        ];

        #[cfg(feature = "health-service")]
        inproc_ids.push((
            "inproc://health-service".to_string(),
            format!("health::{}", &node_id),
        ));

        let inproc_authorizer = InprocAuthorizer::new(inproc_ids, node_id.clone());

        let mut authorizers = Authorizers::new();
        authorizers.add_authorizer("inproc", inproc_authorizer);
        authorizers.add_authorizer("", authorization_manager.authorization_connector());

        let mut connection_manager = ConnectionManager::builder()
            .with_authorizer(Box::new(authorizers))
            .with_matrix_life_cycle(self.mesh.get_life_cycle())
            .with_matrix_sender(self.mesh.get_sender())
            .with_transport(Box::new(transport))
            .with_heartbeat_interval(self.heartbeat)
            .start()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to start connection manager: {}", err))
            })?;
        let connection_connector = connection_manager.connector();

        let mut peer_manager = PeerManager::builder()
            .with_connector(connection_connector.clone())
            .with_identity(node_id.to_string())
            .with_strict_ref_counts(self.strict_ref_counts)
            .start()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to start peer manager: {}", err))
            })?;

        let peer_connector = peer_manager.connector();

        // Listen for services
        Self::listen_for_services(
            connection_connector.clone(),
            internal_service_listeners,
            #[cfg(feature = "service-endpoint")]
            service_listener,
        );

        let orchestrator_connection =
            service_transport
                .connect("inproc://orchestator")
                .map_err(|err| {
                    StartError::TransportError(format!(
                        "unable to initiate orchestrator connection: {:?}",
                        err
                    ))
                })?;

        // set up inproc connections
        let admin_connection = service_transport
            .connect("inproc://admin-service")
            .map_err(|err| {
                StartError::AdminServiceError(format!(
                    "unable to initiate admin service connection: {:?}",
                    err
                ))
            })?;

        #[cfg(feature = "health-service")]
        let health_connection = service_transport
            .connect("inproc://health_service")
            .map_err(|err| {
                StartError::HealthServiceError(format!(
                    "unable to initiate health service connection: {:?}",
                    err
                ))
            })?;

        let (network_dispatcher_sender, network_dispatch_receiver) = dispatch_channel();
        let mut interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_connector.clone())
            .with_message_receiver(self.mesh.get_receiver())
            .with_message_sender(self.mesh.get_sender())
            .with_network_dispatcher_sender(network_dispatcher_sender.clone())
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create peer interconnect: {}", err))
            })?;

        let network_sender = interconnect.new_network_sender();

        // Set up the Circuit dispatcher
        let circuit_dispatcher = set_up_circuit_dispatcher(
            network_sender.clone(),
            &node_id,
            routing_reader.clone(),
            routing_writer.clone(),
            self.signers
                .iter()
                .map(|signer| Ok(signer.public_key()?.into()))
                .collect::<Result<Vec<PublicKey>, SigningError>>()
                .map_err(|err| {
                    StartError::AdminServiceError(format!(
                        "Unable to get public keys from signer for Admin message handler:
                            {}",
                        err
                    ))
                })?,
        );
        let mut circuit_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(circuit_dispatcher)
            .with_thread_name("CircuitDispatchLoop".to_string())
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create circuit dispatch loop: {}", err))
            })?;
        let circuit_dispatch_sender = circuit_dispatch_loop.new_dispatcher_sender();

        // Set up the Network dispatcher
        let network_dispatcher =
            set_up_network_dispatcher(network_sender, &node_id, circuit_dispatch_sender);

        let mut network_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(network_dispatcher)
            .with_thread_name("NetworkDispatchLoop".to_string())
            .with_dispatch_channel((network_dispatcher_sender, network_dispatch_receiver))
            .build()
            .map_err(|err| {
                StartError::NetworkError(format!("Unable to create network dispatch loop: {}", err))
            })?;

        // setup threads to listen on the network ports and add incoming connections to the network
        // these threads will just be dropped on shutdown
        let _ = network_listeners
            .into_iter()
            .map(|mut network_listener| {
                let connection_connector_clone = connection_connector.clone();
                thread::Builder::new()
                    .name(format!(
                        "NetworkIncomingListener-{}",
                        network_listener.endpoint()
                    ))
                    .spawn(move || {
                        let endpoint = network_listener.endpoint();
                        for connection_result in network_listener.incoming() {
                            let connection = match connection_result {
                                Ok(connection) => connection,
                                Err(AcceptError::ProtocolError(msg)) => {
                                    warn!("Failed to accept connection on {}: {}", endpoint, msg);
                                    continue;
                                }
                                Err(AcceptError::IoError(err)) => {
                                    warn!("Failed to accept connection on {}: {}", endpoint, err);
                                    continue;
                                }
                            };
                            debug!("Received connection from {}", connection.remote_endpoint());
                            if let Err(err) =
                                connection_connector_clone.add_inbound_connection(connection)
                            {
                                error!(
                                    "Unable to add inbound connection to connection manager: {}",
                                    err
                                );
                                error!("Exiting listener thread for {}", endpoint);
                                break;
                            }
                        }
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                StartError::NetworkError(format!(
                    "Unable to start NetworkIncomingListener threads: {}",
                    err
                ))
            })?;

        // hold on to peer refs for the peers provided to ensure the connections are kept around
        let mut peer_refs = vec![];
        for endpoint in self.initial_peers.iter() {
            let (endpoint, token) = parse_peer_endpoint(endpoint, &self.peering_token, &node_id);
            match peer_connector.add_unidentified_peer(endpoint, token) {
                Ok(peer_ref) => peer_refs.push(peer_ref),
                Err(err) => error!("Connect Error: {}", err),
            }
        }

        let mut scabbard_factory_builder =
            ScabbardFactoryBuilder::new().with_signature_verifier_factory(signing_context);

        match connection_pool {
            #[cfg(feature = "database-postgres")]
            store::ConnectionPool::Postgres { pool } => {
                scabbard_factory_builder =
                    scabbard_factory_builder.with_storage_configuration(pool.into());
            }
            #[cfg(feature = "database-sqlite")]
            store::ConnectionPool::Sqlite { pool } => {
                scabbard_factory_builder =
                    scabbard_factory_builder.with_storage_configuration(pool.into());
            }
            // This will have failed in create_store_factory above, but we return () to make
            // the compiler/linter happy under the following conditions
            #[cfg(not(any(feature = "database-postgres", feature = "database-sqlite")))]
            store::ConnectionPool::Unsupported => (),
        }

        scabbard_factory_builder = scabbard_factory_builder
            .with_lmdb_state_db_dir(self.state_dir.to_string())
            .with_lmdb_state_enabled(self.enable_lmdb_state)
            .with_state_autocleanup_enabled(self.enable_state_autocleanup);

        let scabbard_factory = scabbard_factory_builder
            .build()
            .map_err(|err| StartError::UserError(err.to_string()))?;

        let mut orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .with_service_factory(Box::new(scabbard_factory))
            .build()
            .map_err(|err| {
                StartError::OrchestratorError(format!("failed to create new orchestrator: {}", err))
            })?
            .run()
            .map_err(|err| {
                StartError::OrchestratorError(format!("failed to start orchestrator: {}", err))
            })?;

        let orchestrator_resources = orchestrator.resources();
        let mut orchestator_shutdown_handle =
            orchestrator.take_shutdown_handle().ok_or_else(|| {
                StartError::OrchestratorError(
                    "Orchestrator shutdown handle was taken more than once".into(),
                )
            })?;

        let (registry, mut registry_shutdown) = create_registry(
            &self.state_dir,
            &self.registries,
            self.registry_auto_refresh,
            self.registry_forced_refresh,
            &*store_factory,
        );

        let mut admin_service_builder = AdminServiceBuilder::new();

        admin_service_builder = admin_service_builder
            .with_node_id(node_id.clone())
            .with_service_orchestrator(orchestrator)
            .with_peer_manager_connector(peer_connector)
            .with_admin_service_store(store_factory.get_admin_service_store())
            .with_signature_verifier(admin_service_verifier)
            .with_admin_key_verifier(Box::new(registry.clone_box_as_reader()))
            .with_key_permission_manager(Box::new(AllowAllKeyPermissionManager))
            .with_coordinator_timeout(self.admin_timeout)
            .with_routing_table_writer(routing_writer.clone())
            .with_admin_event_store(store_factory.get_admin_service_store())
            .with_public_keys(
                self.signers
                    .iter()
                    .map(|signer| Ok(signer.public_key()?.into()))
                    .collect::<Result<Vec<PublicKey>, SigningError>>()
                    .map_err(|err| {
                        StartError::AdminServiceError(format!(
                            "Unable to get public keys from signer for Admin message handler:
                            {}",
                            err
                        ))
                    })?,
            );

        let mut validators: HashMap<String, Box<dyn ServiceArgValidator + Send>> = HashMap::new();
        validators.insert("scabbard".into(), Box::new(ScabbardArgValidator));

        admin_service_builder = admin_service_builder.with_service_arg_validators(validators);

        let admin_service = admin_service_builder.build().map_err(|err| {
            StartError::AdminServiceError(format!("unable to create admin service: {}", err))
        })?;

        #[cfg(feature = "authorization")]
        let node_id_clone = node_id.clone();
        let display_name: String = self
            .display_name
            .to_owned()
            .or_else(|| Some(format!("Node: {}", node_id)))
            .unwrap();
        #[cfg(feature = "service-endpoint")]
        let service_endpoint = self.service_endpoint.clone();
        let network_endpoints = self.network_endpoints.clone();
        let advertised_endpoints = self.advertised_endpoints.clone();

        let circuit_resource_provider = CircuitResourceProvider::new(
            #[allow(clippy::redundant_clone)]
            node_id.to_owned(),
            store_factory.get_admin_service_store(),
        );

        #[cfg(not(feature = "https-bind"))]
        let bind = self
            .rest_api_endpoint
            .strip_prefix("http://")
            .unwrap_or(&self.rest_api_endpoint);

        #[cfg(feature = "https-bind")]
        let bind = self.build_rest_api_bind()?;

        // Allowing unused_mut because rest_api_builder must be mutable if feature biome is enabled
        #[allow(unused_mut)]
        let mut rest_api_builder = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(registry.resources())
            .add_resources(admin_service.resources())
            .add_resources(orchestrator_resources)
            .add_resources(circuit_resource_provider.resources());

        #[cfg(feature = "authorization")]
        {
            // Allowing unused_mut because authorization_handlers must be mutable if
            // `authorization-handler-allow-keys` or `auth-handler-maintenance` are enabled
            #[allow(unused_mut)]
            let mut authorization_handlers = vec![
                #[cfg(feature = "authorization-handler-allow-keys")]
                create_allow_keys_authorization_handler(
                    create_allow_keys_path(
                        &self.config_dir,
                        #[cfg(feature = "config-allow-keys")]
                        &self.allow_keys_file,
                        #[cfg(not(feature = "config-allow-keys"))]
                        "allow_keys",
                    )
                    .to_str()
                    .expect("path built from &str cannot be invalid"),
                )?,
            ];

            #[cfg(feature = "authorization-handler-rbac")]
            let rbac_store = store_factory.get_role_based_authorization_store();

            #[cfg(feature = "authorization-handler-maintenance")]
            {
                #[cfg(feature = "authorization-handler-rbac")]
                let maintenance_mode_auth_handler =
                    MaintenanceModeAuthorizationHandler::new(Some(rbac_store.clone()));
                #[cfg(not(feature = "authorization-handler-rbac"))]
                let maintenance_mode_auth_handler = MaintenanceModeAuthorizationHandler::default();
                rest_api_builder =
                    rest_api_builder.add_resources(maintenance_mode_auth_handler.resources());
                authorization_handlers.push(Box::new(maintenance_mode_auth_handler));
            }

            #[cfg(feature = "authorization-handler-rbac")]
            {
                authorization_handlers
                    .push(Box::new(RoleBasedAuthorizationHandler::new(rbac_store)));
                rest_api_builder = rest_api_builder.add_resources(
                    RoleBasedAuthorizationResourceProvider::new(
                        store_factory.get_role_based_authorization_store(),
                    )
                    .resources(),
                );
            }

            rest_api_builder = rest_api_builder
                .with_authorization_handlers(authorization_handlers)
                .add_resource(Resource::build("/openapi.yaml").add_method(
                    Method::Get,
                    Permission::AllowAuthenticated,
                    routes::get_openapi,
                ))
                .add_resource(Resource::build("/status").add_method(
                    Method::Get,
                    routes::STATUS_READ_PERMISSION,
                    move |_, _| {
                        routes::get_status(
                            node_id_clone.clone(),
                            display_name.clone(),
                            #[cfg(feature = "service-endpoint")]
                            service_endpoint.clone(),
                            network_endpoints.clone(),
                            advertised_endpoints.clone(),
                        )
                    },
                ));
        }
        #[cfg(not(feature = "authorization"))]
        {
            rest_api_builder = rest_api_builder
                .add_resource(
                    Resource::build("/openapi.yaml").add_method(Method::Get, routes::get_openapi),
                )
                .add_resource(
                    Resource::build("/status").add_method(Method::Get, move |_, _| {
                        routes::get_status(
                            node_id.clone(),
                            display_name.clone(),
                            #[cfg(feature = "service-endpoint")]
                            service_endpoint.clone(),
                            network_endpoints.clone(),
                            advertised_endpoints.clone(),
                        )
                    }),
                );
        }

        #[cfg(feature = "rest-api-cors")]
        {
            if let Some(list) = &self.whitelist {
                debug!("Whitelisted domains added to CORS");
                rest_api_builder = rest_api_builder.with_whitelist(list.to_vec());
            }
        }

        #[allow(unused_mut)]
        let mut auth_configs = vec![
            // Add Cylinder JWT as an auth provider
            AuthConfig::Cylinder {
                verifier: auth_config_verifier,
            },
        ];

        // Add Biome credentials as an auth provider if it's enabled
        #[cfg(feature = "biome-credentials")]
        if self.enable_biome_credentials {
            let mut biome_credentials_builder: BiomeCredentialsRestResourceProviderBuilder =
                Default::default();

            biome_credentials_builder = biome_credentials_builder
                .with_refresh_token_store(store_factory.get_biome_refresh_token_store())
                .with_credentials_store(store_factory.get_biome_credentials_store());

            #[cfg(feature = "biome-key-management")]
            {
                biome_credentials_builder =
                    biome_credentials_builder.with_key_store(store_factory.get_biome_key_store())
            }

            let biome_credentials_resource_provider =
                biome_credentials_builder.build().map_err(|err| {
                    StartError::RestApiError(format!(
                        "Unable to build Biome credentials REST routes: {}",
                        err
                    ))
                })?;

            auth_configs.push(AuthConfig::Biome {
                biome_credentials_resource_provider,
            });
        }

        #[cfg(feature = "oauth")]
        {
            // Handle OAuth config. If no OAuth config values are provided, just skip this;
            // otherwise, require that all are set.
            let any_oauth_args_provided = self.oauth_provider.is_some()
                || self.oauth_client_id.is_some()
                || self.oauth_client_secret.is_some()
                || self.oauth_redirect_url.is_some();
            if any_oauth_args_provided {
                let oauth_provider = self.oauth_provider.as_deref().ok_or_else(|| {
                    StartError::RestApiError("missing OAuth provider configuration".into())
                })?;
                let client_id = self.oauth_client_id.clone().ok_or_else(|| {
                    StartError::RestApiError("missing OAuth client ID configuration".into())
                })?;
                let client_secret = self.oauth_client_secret.clone().ok_or_else(|| {
                    StartError::RestApiError("missing OAuth client secret configuration".into())
                })?;
                let redirect_url = self.oauth_redirect_url.clone().ok_or_else(|| {
                    StartError::RestApiError("missing OAuth redirect URL configuration".into())
                })?;
                let oauth_config = match oauth_provider {
                    "azure" => OAuthConfig::Azure {
                        client_id,
                        client_secret,
                        redirect_url,
                        oauth_openid_url: self.oauth_openid_url.clone().ok_or_else(|| {
                            StartError::RestApiError(
                                "missing OAuth OpenID discovery document URL configuration".into(),
                            )
                        })?,
                        inflight_request_store: store_factory.get_oauth_inflight_request_store(),
                    },
                    "github" => OAuthConfig::GitHub {
                        client_id,
                        client_secret,
                        redirect_url,
                        inflight_request_store: store_factory.get_oauth_inflight_request_store(),
                    },
                    "google" => OAuthConfig::Google {
                        client_id,
                        client_secret,
                        redirect_url,
                        inflight_request_store: store_factory.get_oauth_inflight_request_store(),
                    },
                    "openid" => OAuthConfig::OpenId {
                        client_id,
                        client_secret,
                        redirect_url,
                        oauth_openid_url: self.oauth_openid_url.clone().ok_or_else(|| {
                            StartError::RestApiError(
                                "missing OAuth OpenID discovery document URL configuration".into(),
                            )
                        })?,
                        auth_params: self.oauth_openid_auth_params.clone(),
                        scopes: self.oauth_openid_scopes.clone(),
                        inflight_request_store: store_factory.get_oauth_inflight_request_store(),
                    },
                    other_provider => {
                        return Err(StartError::RestApiError(format!(
                            "invalid OAuth provider: {}",
                            other_provider
                        )))
                    }
                };

                auth_configs.push(AuthConfig::OAuth {
                    oauth_config,
                    oauth_user_session_store: store_factory.get_biome_oauth_user_session_store(),
                    #[cfg(feature = "biome-profile")]
                    user_profile_store: store_factory.get_biome_user_profile_store(),
                });
            }
        }

        rest_api_builder = rest_api_builder.with_auth_configs(auth_configs);

        #[cfg(feature = "biome-key-management")]
        {
            rest_api_builder = rest_api_builder.add_resources(
                BiomeKeyManagementRestResourceProvider::new(Arc::new(
                    store_factory.get_biome_key_store(),
                ))
                .resources(),
            );
        }

        #[cfg(feature = "biome-profile")]
        {
            rest_api_builder = rest_api_builder.add_resources(
                BiomeProfileRestResourceProvider::new(Arc::new(
                    store_factory.get_biome_user_profile_store(),
                ))
                .resources(),
            );
        }

        #[cfg(feature = "health-service")]
        let mut health_service_shutdown_handle = {
            let health_service = HealthService::new(&node_id);
            rest_api_builder = rest_api_builder.add_resources(health_service.resources());

            start_health_service(health_connection, health_service)?
        };

        let (rest_api_shutdown_handle, rest_api_join_handle) = rest_api_builder.build()?.run()?;

        let mut admin_shutdown_handle = Self::start_admin_service(admin_connection, admin_service)?;

        let (shutdown_tx, shutdown_rx) = channel();
        ctrlc::set_handler(move || {
            if shutdown_tx.send(()).is_err() {
                // This was the second ctrl-c (as the receiver is dropped after the first one).
                std::process::exit(0);
            }
        })
        .expect("Error setting Ctrl-C handler");

        // recv that value, ignoring the result.
        let _ = shutdown_rx.recv();
        drop(shutdown_rx);
        info!("Initiating graceful shutdown (press Ctrl+C again to force)");

        running.store(false, Ordering::SeqCst);

        admin_shutdown_handle.signal_shutdown();
        orchestator_shutdown_handle.signal_shutdown();
        #[cfg(feature = "health-service")]
        health_service_shutdown_handle.signal_shutdown();

        if let Err(err) = admin_shutdown_handle.wait_for_shutdown() {
            error!("Unable to cleanly shut down Admin service: {}", err);
        }

        if let Err(err) = orchestator_shutdown_handle.wait_for_shutdown() {
            error!("Unable to cleanly shut down Orchestrator service: {}", err);
        }
        #[cfg(feature = "health-service")]
        if let Err(err) = health_service_shutdown_handle.wait_for_shutdown() {
            error!("Unable to cleanly shut down Health service: {}", err);
        }

        if let Err(err) = rest_api_shutdown_handle.shutdown() {
            error!("Unable to cleanly shut down REST API server: {}", err);
        }
        circuit_dispatch_loop.signal_shutdown();
        network_dispatch_loop.signal_shutdown();

        if let Err(err) = circuit_dispatch_loop.wait_for_shutdown() {
            error!("Unable to cleanly shut down circuit dispatch loop: {}", err);
        }

        if let Err(err) = network_dispatch_loop.wait_for_shutdown() {
            error!("Unable to cleanly shut down network dispatch loop: {}", err);
        }

        registry_shutdown.signal_shutdown();
        if let Err(err) = registry_shutdown.wait_for_shutdown() {
            error!("Unable to cleanly shut down network dispatch loop: {}", err);
        }

        interconnect.signal_shutdown();

        // Join threads and shutdown network components
        let _ = rest_api_join_handle.join();

        peer_manager.signal_shutdown();
        if let Err(err) = peer_manager.wait_for_shutdown() {
            error!("Unable to cleanly shut down PeerManager: {}", err);
        }

        connection_manager.signal_shutdown();
        if let Err(err) = connection_manager.wait_for_shutdown() {
            error!("Unable to cleanly shut down ConnectionManager: {}", err);
        }

        self.mesh.signal_shutdown();
        if let Err(err) = interconnect.wait_for_shutdown() {
            error!("Unable to cleanly shut down peer interconnect: {}", err);
        }
        if let Err(err) = self.mesh.clone().wait_for_shutdown() {
            error!("Unable to cleanly shut down Mesh: {}", err);
        }
        Ok(())
    }

    #[cfg(feature = "https-bind")]
    fn build_rest_api_bind(&self) -> Result<splinter::rest_api::BindConfig, StartError> {
        match self.rest_api_endpoint.strip_prefix("http://") {
            Some(insecure_endpoint) => Ok(splinter::rest_api::BindConfig::Http(
                insecure_endpoint.into(),
            )),
            None => {
                if let Some((rest_api_server_cert, rest_api_server_key)) =
                    self.rest_api_ssl_settings.as_ref()
                {
                    Ok(splinter::rest_api::BindConfig::Https {
                        bind: self
                            .rest_api_endpoint
                            .strip_prefix("https://")
                            .or(Some(&self.rest_api_endpoint))
                            .map(String::from)
                            .expect("There should be a value, due to the above or"),
                        cert_path: rest_api_server_cert.clone(),
                        key_path: rest_api_server_key.clone(),
                    })
                } else {
                    Err(StartError::RestApiError(
                        "The REST API has been configured for HTTPS, \
                        but no certificate and key was provided."
                            .into(),
                    ))
                }
            }
        }
    }

    fn listen_for_services(
        connection_connector: Connector,
        internal_service_listeners: Vec<Box<dyn Listener>>,
        #[cfg(feature = "service-endpoint")] mut external_service_listener: Box<dyn Listener>,
    ) {
        // this thread will just be dropped on shutdown
        let _ = thread::Builder::new()
            .name("ServiceIncomingConnectionListener".into())
            .spawn(move || {
                // accept the internal service connections
                for mut listener in internal_service_listeners.into_iter() {
                    match listener.incoming().next() {
                        Some(Ok(connection)) => {
                            let remote_endpoint = connection.remote_endpoint();
                            if let Err(err) =
                                connection_connector.add_inbound_connection(connection)
                            {
                                error!("Unable to add peer {}: {}", remote_endpoint, err)
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

                #[cfg(feature = "service-endpoint")]
                {
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
                        if let Err(err) = connection_connector.add_inbound_connection(connection) {
                            error!("Unable to add inbound service connection: {}", err);
                        }
                    }
                }

                Ok(())
            })
            .expect("Unable to create ServiceIncomingConnectionListener thread");
    }

    fn start_admin_service(
        connection: Box<dyn Connection>,
        admin_service: AdminService,
    ) -> Result<service::ServiceProcessorShutdownHandle, StartError> {
        let mut admin_service_processor = service::ServiceProcessor::new(
            connection,
            "admin".into(),
            ADMIN_SERVICE_PROCESSOR_INCOMING_CAPACITY,
            ADMIN_SERVICE_PROCESSOR_OUTGOING_CAPACITY,
            ADMIN_SERVICE_PROCESSOR_CHANNEL_CAPACITY,
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
    }
}

#[cfg(feature = "health-service")]
fn start_health_service(
    connection: Box<dyn Connection>,
    health_service: HealthService,
) -> Result<service::ServiceProcessorShutdownHandle, StartError> {
    let mut health_service_processor = service::ServiceProcessor::new(
        connection,
        "health".into(),
        HEALTH_SERVICE_PROCESSOR_INCOMING_CAPACITY,
        HEALTH_SERVICE_PROCESSOR_OUTGOING_CAPACITY,
        HEALTH_SERVICE_PROCESSOR_CHANNEL_CAPACITY,
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

    health_service_processor.start().map_err(|err| {
        StartError::HealthServiceError(format!("unable to health service processor: {}", err))
    })
}

fn set_up_network_dispatcher(
    network_sender: NetworkMessageSender,
    node_id: &str,
    circuit_sender: DispatchMessageSender<CircuitMessageType>,
) -> Dispatcher<NetworkMessageType> {
    let mut dispatcher = Dispatcher::<NetworkMessageType>::new(Box::new(network_sender));

    let network_echo_handler = NetworkEchoHandler::new(node_id.to_string());
    dispatcher.set_handler(Box::new(network_echo_handler));

    let network_heartbeat_handler = NetworkHeartbeatHandler::new();
    // do not add auth guard
    dispatcher.set_handler(Box::new(network_heartbeat_handler));

    let circuit_message_handler = CircuitMessageHandler::new(circuit_sender);
    dispatcher.set_handler(Box::new(circuit_message_handler));

    dispatcher
}

fn set_up_circuit_dispatcher(
    network_sender: NetworkMessageSender,
    node_id: &str,
    routing_reader: Box<dyn RoutingTableReader>,
    routing_writer: Box<dyn RoutingTableWriter>,
    public_keys: Vec<PublicKey>,
) -> Dispatcher<CircuitMessageType> {
    let mut dispatcher = Dispatcher::<CircuitMessageType>::new(Box::new(network_sender));

    let service_connect_request_handler = ServiceConnectRequestHandler::new(
        node_id.to_string(),
        routing_reader.clone(),
        routing_writer.clone(),
    );
    dispatcher.set_handler(Box::new(service_connect_request_handler));

    let service_disconnect_request_handler =
        ServiceDisconnectRequestHandler::new(routing_reader.clone(), routing_writer.clone());
    dispatcher.set_handler(Box::new(service_disconnect_request_handler));

    let direct_message_handler =
        CircuitDirectMessageHandler::new(node_id.to_string(), routing_reader.clone());
    dispatcher.set_handler(Box::new(direct_message_handler));

    let circuit_error_handler =
        CircuitErrorHandler::new(node_id.to_string(), routing_reader.clone());
    dispatcher.set_handler(Box::new(circuit_error_handler));

    // Circuit Admin handlers
    let admin_direct_message_handler =
        AdminDirectMessageHandler::new(node_id.to_string(), routing_reader, public_keys);
    dispatcher.set_handler(Box::new(admin_direct_message_handler));

    dispatcher
}

fn create_registry(
    state_dir: &str,
    registries: &[String],
    auto_refresh_interval: u64,
    forced_refresh_interval: u64,
    store_factory: &dyn splinter::store::StoreFactory,
) -> (Box<dyn RwRegistry>, RegistryShutdownHandle) {
    let mut registry_shutdown_handle = RegistryShutdownHandle::new();

    let local_registry = store_factory.get_registry_store();

    let read_only_registries = registries
        .iter()
        .filter_map(|registry| {
            let (scheme, path) = parse_registry_arg(registry);

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
                    state_dir,
                    auto_refresh_interval,
                    forced_refresh_interval,
                ) {
                    Ok(mut registry) => {
                        // this should alwasy return some
                        if let Some(shutdown_handle) = registry.take_shutdown_handle() {
                            registry_shutdown_handle
                                .add_remote_yaml_shutdown_handle(shutdown_handle)
                        }

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
                    "Invalid registry URI scheme provided ({}): must be file, http, or https",
                    registry
                );
                None
            }
        })
        .collect();

    let unified_registry = Box::new(UnifiedRegistry::new(local_registry, read_only_registries));

    (unified_registry, registry_shutdown_handle)
}

// Parses a registry argument, returning the uri scheme (defaulting to file) and remaining uri data
fn parse_registry_arg(registry: &str) -> (&str, &str) {
    let mut iter = registry.splitn(2, "://");
    match (iter.next(), iter.next()) {
        (Some(scheme), Some(data)) => (scheme, data),
        (Some(path), None) => ("file", path),
        _ => unreachable!(), // splitn will always return at least one item, and never a second item without a first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_registry_arg() {
        assert_eq!(
            parse_registry_arg("registry.yaml"),
            ("file", "registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("./registry.yaml"),
            ("file", "./registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("/registry.yaml"),
            ("file", "/registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("file://registry.yaml"),
            ("file", "registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("file://./registry.yaml"),
            ("file", "./registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("file:///registry.yaml"),
            ("file", "/registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("file:///home/user/registry.yaml"),
            ("file", "/home/user/registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("https://server/registry.yaml"),
            ("https", "server/registry.yaml")
        );
        assert_eq!(
            parse_registry_arg("http://server/registry.yaml"),
            ("http", "server/registry.yaml")
        );
    }

    #[cfg(feature = "authorization-handler-allow-keys")]
    #[test]
    fn test_create_allow_keys_path_absolute_path() {
        assert_eq!(
            create_allow_keys_path("/config/path", "/absolute/path"),
            Path::new("/absolute/path")
        );
    }

    #[cfg(feature = "authorization-handler-allow-keys")]
    #[test]
    fn test_create_allow_keys_path_relative_path() {
        assert_eq!(
            create_allow_keys_path("/config/path", "relative/path"),
            Path::new("/config/path/relative/path")
        );
    }
}

#[cfg(feature = "authorization-handler-allow-keys")]
fn create_allow_keys_authorization_handler(
    allow_keys_path: &str,
) -> Result<Box<dyn AuthorizationHandler>, StartError> {
    debug!(
        "Reading allow keys authorization handler file: {:?}",
        allow_keys_path
    );

    Ok(Box::new(
        AllowKeysAuthorizationHandler::new(allow_keys_path).map_err(|err| {
            StartError::StorageError(format!(
                "Failed to initialize allow keys authorization handler: {}",
                err
            ))
        })?,
    ))
}

#[cfg(feature = "authorization-handler-allow-keys")]
fn create_allow_keys_path(config_path: &str, allow_keys_file: &str) -> PathBuf {
    let allow_keys_path = Path::new(allow_keys_file);
    if allow_keys_path.is_relative() {
        Path::new(config_path).join(allow_keys_file)
    } else {
        allow_keys_path.to_path_buf()
    }
}

/// Parse the peer endpoint that we want to connect to regardless of a circuit. The endpoint will
/// either be in normal form impling it should use the configured peer authorization token for
/// peering (usally challenge, unless no keys were provided) or includes +trust after the
/// transport type which means a trust token should be used
fn parse_peer_endpoint(
    endpoint: &str,
    peering_token: &PeerAuthorizationToken,
    node_id: &str,
) -> (String, PeerAuthorizationToken) {
    // if endpoint is in the form tcp+trust://ipaddr:port Trust authorization must be used
    if endpoint.contains("+trust://") {
        // set endpoint to the form tcp://ipaddr:port, removing the +trust and return a trust token
        let endpoint = endpoint.replace("+trust://", "://");
        (endpoint, PeerAuthorizationToken::from_peer_id(node_id))
    } else {
        (endpoint.to_string(), peering_token.clone())
    }
}
