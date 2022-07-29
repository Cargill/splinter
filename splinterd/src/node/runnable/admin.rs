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

//! Contains the implementation of `NodeBuilder`.

use std::time::Duration;

use cylinder::Verifier;
use scabbard::service::ScabbardFactory;
use splinter::admin::service::{AdminCommands, AdminServiceBuilder, AdminServiceStatus};
use splinter::circuit::routing::RoutingTableWriter;
use splinter::error::InternalError;
use splinter::events::Reactor;
use splinter::peer::PeerManagerConnector;
use splinter::public_key::PublicKey;
use splinter::registry::{LocalYamlRegistry, RegistryReader, UnifiedRegistry};
use splinter::runtime::service::instance::{ServiceOrchestratorBuilder, ServiceProcessorBuilder};
use splinter::store::StoreFactory;
use splinter::transport::{inproc::InprocTransport, Transport};
use splinter_rest_api_actix_web_1::admin::{AdminServiceRestProvider, CircuitResourceProvider};
use splinter_rest_api_actix_web_1::framework::RestResourceProvider as _;
use splinter_rest_api_actix_web_1::registry::RwRegistryRestResourceProvider;
use splinter_rest_api_actix_web_1::service::ServiceOrchestratorRestResourceProviderBuilder;

use crate::node::builder::admin::AdminServiceEventClientVariant;
use crate::node::running::admin::{self as running_admin, AdminSubsystem};

// These multiplied will max out at 5 min
const MAX_STARTUP_WAIT_MILLIS: u64 = 500;
const MAX_STARTUP_WAIT_ATTEMPTS: u64 = 600;

pub struct RunnableAdminSubsystem {
    pub node_id: String,
    pub admin_timeout: Duration,
    pub store_factory: Box<dyn StoreFactory>,
    pub peer_connector: PeerManagerConnector,
    pub routing_writer: Box<dyn RoutingTableWriter>,
    pub service_transport: InprocTransport,
    pub admin_service_verifier: Box<dyn Verifier>,
    pub scabbard_service_factory: Option<ScabbardFactory>,
    pub registries: Option<Vec<String>>,
    pub admin_service_event_client_variant: AdminServiceEventClientVariant,
    pub public_keys: Vec<PublicKey>,
}

impl RunnableAdminSubsystem {
    pub fn run(self) -> Result<AdminSubsystem, InternalError> {
        let node_id = self.node_id;
        let store_factory = self.store_factory;
        let admin_timeout = self.admin_timeout;
        let peer_connector = self.peer_connector;
        let mut service_transport = self.service_transport;
        let routing_writer = self.routing_writer;

        let mut registry = store_factory.get_registry_store();

        if let Some(external_registries) = self.registries {
            let read_only_registries = external_registries
                .iter()
                .map(|registry| {
                    let mut iter = registry.splitn(2, "://");
                    match (iter.next(), iter.next()) {
                        (Some(scheme), Some(path)) => match scheme {
                            "file" => {
                                debug!(
                                    "Attempting to add local read-only registry from file: {}",
                                    path
                                );
                                match LocalYamlRegistry::new(path) {
                                    Ok(registry) => {
                                        Ok(Box::new(registry) as Box<dyn RegistryReader>)
                                    }
                                    Err(err) => Err(InternalError::from_source_with_message(
                                        Box::new(err),
                                        format!(
                                            "Failed to add read-only LocalYamlRegistry '{}'",
                                            path
                                        ),
                                    )),
                                }
                            }
                            _ => Err(InternalError::with_message(format!(
                                "Invalid registry provided ({}): must be valid 'file://' URI",
                                registry
                            ))),
                        },
                        (Some(_), None) => Err(InternalError::with_message(
                            "Failed to parse registry argument: no URI scheme provided".to_string(),
                        )),
                        _ => unreachable!(), // splitn always returns at least one item
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            registry = Box::new(UnifiedRegistry::new(registry, read_only_registries));
        }

        let orchestrator_connection = service_transport
            .connect("inproc://orchestator")
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut orchestrator_builder =
            ServiceOrchestratorBuilder::new().with_connection(orchestrator_connection);
        if let Some(scabbard_service_factory) = self.scabbard_service_factory {
            orchestrator_builder =
                orchestrator_builder.with_service_factory(Box::new(scabbard_service_factory));
        }

        let orchestrator = orchestrator_builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?
            .run()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        let orchestrator_rest_provider = ServiceOrchestratorRestResourceProviderBuilder::new()
            .with_endpoint_factory(
                scabbard::service::SERVICE_TYPE,
                Box::new(splinter_rest_api_actix_web_1::scabbard::ScabbardServiceEndpointProvider::default()),
            )
            .build(&orchestrator);

        let mut admin_service_builder = AdminServiceBuilder::new();

        admin_service_builder = admin_service_builder
            .with_node_id(node_id)
            .with_lifecycle_dispatch(vec![Box::new(orchestrator)])
            .with_peer_manager_connector(peer_connector.clone())
            .with_admin_service_store(store_factory.get_admin_service_store())
            .with_admin_event_store(store_factory.get_admin_service_store())
            .with_signature_verifier(self.admin_service_verifier)
            .with_admin_key_verifier(Box::new(registry.clone_box_as_reader()))
            .with_key_permission_manager(Box::new(
                splinter::keys::insecure::AllowAllKeyPermissionManager,
            ))
            .with_coordinator_timeout(admin_timeout)
            .with_routing_table_writer(routing_writer)
            .with_admin_event_store(store_factory.get_admin_service_store())
            .with_public_keys(self.public_keys.to_vec());

        let circuit_resource_provider =
            CircuitResourceProvider::new(store_factory.get_admin_service_store());

        let admin_service = admin_service_builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let commands = admin_service.commands();

        let mut actix1_resources = vec![];

        actix1_resources.append(&mut AdminServiceRestProvider::new(&admin_service).resources());
        actix1_resources.append(&mut circuit_resource_provider.resources());
        actix1_resources.append(&mut RwRegistryRestResourceProvider::new(&registry).resources());
        actix1_resources.append(&mut orchestrator_rest_provider.resources());

        // set up inproc connections
        let admin_connection = service_transport
            .connect("inproc://admin-service")
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let admin_service_processor = ServiceProcessorBuilder::new()
            .with_connection(admin_connection)
            .with_circuit("admin".into())
            .with_service(Box::new(admin_service))
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        let admin_service_shutdown = admin_service_processor
            .start()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        let running_admin_service_event_client_variant =
            match self.admin_service_event_client_variant {
                AdminServiceEventClientVariant::ActixWebClient => {
                    running_admin::AdminServiceEventClientVariant::ActixWebClient(Reactor::new())
                }
            };

        let mut attempts = 0..MAX_STARTUP_WAIT_ATTEMPTS;

        loop {
            let status = commands
                .admin_service_status()
                .map_err(|e| InternalError::from_source(Box::new(e)))?;

            if status == AdminServiceStatus::Running {
                break;
            }
            if attempts.next().is_none() {
                return Err(InternalError::with_message(format!(
                    "Admin service failed to complete startup after {} secs",
                    (MAX_STARTUP_WAIT_MILLIS * MAX_STARTUP_WAIT_ATTEMPTS) / 1000
                )));
            }
            std::thread::sleep(Duration::from_millis(MAX_STARTUP_WAIT_MILLIS))
        }

        Ok(AdminSubsystem {
            registry_writer: registry.clone_box_as_writer(),
            admin_service_shutdown,
            actix1_resources,
            store_factory,
            admin_service_event_client_variant: running_admin_service_event_client_variant,
            peer_connector,
        })
    }
}
