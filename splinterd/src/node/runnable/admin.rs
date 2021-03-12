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

//! Contains the implementation of `NodeBuilder`.

use std::time::Duration;

use cylinder::{secp256k1::Secp256k1Context, VerifierFactory};

use splinter::admin::rest_api::CircuitResourceProvider;
use splinter::admin::service::AdminServiceBuilder;
use splinter::circuit::routing::RoutingTableWriter;
use splinter::error::InternalError;
use splinter::orchestrator::ServiceOrchestratorBuilder;
use splinter::peer::PeerManagerConnector;
use splinter::rest_api::actix_web_1::RestResourceProvider as _;
use splinter::service::ServiceProcessorBuilder;
use splinter::store::StoreFactory;
use splinter::transport::{inproc::InprocTransport, Transport};

use crate::node::running::admin::AdminSubsystem;

pub struct RunnableAdminSubsystem {
    pub node_id: String,
    pub admin_timeout: Duration,
    pub store_factory: Box<dyn StoreFactory>,
    pub peer_connector: PeerManagerConnector,
    pub routing_writer: Box<dyn RoutingTableWriter>,
    pub service_transport: InprocTransport,
}

impl RunnableAdminSubsystem {
    pub fn run(self) -> Result<AdminSubsystem, InternalError> {
        let node_id = self.node_id;
        let store_factory = self.store_factory;
        let admin_timeout = self.admin_timeout;
        let peer_connector = self.peer_connector;
        let mut service_transport = self.service_transport;
        let routing_writer = self.routing_writer;

        let registry = store_factory.get_registry_store();

        let orchestrator_connection = service_transport
            .connect("inproc://orchestator")
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let orchestrator = ServiceOrchestratorBuilder::new()
            .with_connection(orchestrator_connection)
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?
            .run()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        let mut admin_service_builder = AdminServiceBuilder::new();

        let signing_context = Secp256k1Context::new();
        let admin_service_verifier = signing_context.new_verifier();

        admin_service_builder = admin_service_builder
            .with_node_id(node_id.clone())
            .with_service_orchestrator(orchestrator)
            .with_peer_manager_connector(peer_connector)
            .with_admin_service_store(store_factory.get_admin_service_store())
            .with_admin_event_store(store_factory.get_admin_service_store())
            .with_signature_verifier(admin_service_verifier)
            .with_admin_key_verifier(Box::new(registry.clone_box_as_reader()))
            .with_key_permission_manager(Box::new(
                splinter::keys::insecure::AllowAllKeyPermissionManager,
            ))
            .with_coordinator_timeout(admin_timeout)
            .with_routing_table_writer(routing_writer);

        let circuit_resource_provider =
            CircuitResourceProvider::new(node_id, store_factory.get_admin_service_store());

        #[cfg(feature = "admin-service-event-store")]
        {
            admin_service_builder = admin_service_builder
                .with_admin_event_store(store_factory.get_admin_service_store());
        }

        let admin_service = admin_service_builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut actix1_resources = vec![];

        actix1_resources.append(&mut admin_service.resources());
        actix1_resources.append(&mut circuit_resource_provider.resources());

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

        Ok(AdminSubsystem {
            registry_writer: registry.clone_box_as_writer(),
            admin_service_shutdown,
            actix1_resources,
        })
    }
}
