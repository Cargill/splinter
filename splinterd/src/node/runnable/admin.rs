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
use splinter::admin::service::{admin_service_id, AdminServiceBuilder};
use splinter::circuit::routing::{memory::RoutingTable, RoutingTableWriter};
use splinter::error::InternalError;
use splinter::mesh::Mesh;
use splinter::network::connection_manager::{
    authorizers::Authorizers, authorizers::InprocAuthorizer, ConnectionManager, Connector,
};
use splinter::orchestrator::ServiceOrchestratorBuilder;
use splinter::peer::PeerManager;
use splinter::rest_api::actix_web_1::RestResourceProvider as _;
use splinter::service::ServiceProcessorBuilder;
use splinter::store::StoreFactory;
use splinter::transport::{inproc::InprocTransport, multi::MultiTransport, Listener, Transport};

use crate::node::running::admin::AdminSubsystem;

pub struct RunnableAdminSubsystem {
    pub node_id: String,
    pub transport: MultiTransport,
    pub admin_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub store_factory: Box<dyn StoreFactory>,
    pub strict_ref_counts: bool,
}

impl RunnableAdminSubsystem {
    pub fn run(self) -> Result<AdminSubsystem, InternalError> {
        let node_id = self.node_id;
        let store_factory = self.store_factory;
        let admin_timeout = self.admin_timeout;
        let heartbeat_interval = self.heartbeat_interval;
        let mut transport = self.transport;

        let mut service_transport = InprocTransport::default();
        transport.add_transport(Box::new(service_transport.clone()));

        let _internal_service_listeners = Self::build_internal_service_listeners(&mut transport)?;

        let mesh = Mesh::new(512, 128);

        // Configure connection manager
        let connection_manager = Self::build_connection_manager(
            &node_id,
            Box::new(transport),
            &mesh,
            heartbeat_interval,
        )?;
        let connection_connector = connection_manager.connector();

        let peer_manager =
            Self::build_peer_manager(&node_id, connection_connector, self.strict_ref_counts)?;

        let peer_connector = peer_manager.connector();

        let registry = store_factory.get_registry_store();

        let table = RoutingTable::default();
        let routing_writer: Box<dyn RoutingTableWriter> = Box::new(table);

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
            CircuitResourceProvider::new(node_id.clone(), store_factory.get_admin_service_store());

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

        Ok(AdminSubsystem {
            node_id,
            _admin_service_processor: admin_service_processor,
            actix1_resources,
            peer_manager,
            connection_manager,
        })
    }

    fn build_internal_service_listeners(
        transport: &mut dyn Transport,
    ) -> Result<Vec<Box<dyn Listener>>, InternalError> {
        Ok(vec![
            transport
                .listen("inproc://admin-service")
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
            transport
                .listen("inproc://orchestator")
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
        ])
    }

    fn build_connection_manager(
        node_id: &str,
        transport: Box<dyn Transport + Send>,
        mesh: &Mesh,
        heartbeat_interval: Duration,
    ) -> Result<ConnectionManager, InternalError> {
        let inproc_ids = vec![
            (
                "inproc://orchestator".to_string(),
                format!("orchestator::{}", node_id),
            ),
            (
                "inproc://admin-service".to_string(),
                admin_service_id(node_id),
            ),
        ];

        let inproc_authorizer = InprocAuthorizer::new(inproc_ids);

        let mut authorizers = Authorizers::new();
        authorizers.add_authorizer("inproc", inproc_authorizer);

        ConnectionManager::builder()
            .with_authorizer(Box::new(authorizers))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .with_heartbeat_interval(heartbeat_interval.as_secs())
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn build_peer_manager(
        node_id: &str,
        connection_connector: Connector,
        strict_ref_counts: bool,
    ) -> Result<PeerManager, InternalError> {
        PeerManager::builder()
            .with_connector(connection_connector)
            .with_identity(node_id.to_string())
            .with_strict_ref_counts(strict_ref_counts)
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}
