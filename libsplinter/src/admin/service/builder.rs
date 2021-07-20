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

//! Builder for the AdminService

#[cfg(feature = "service-arg-validation")]
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cylinder::Verifier as SignatureVerifier;

use crate::admin::store::AdminServiceStore;
use crate::circuit::routing::RoutingTableWriter;
use crate::error::InvalidStateError;
use crate::keys::KeyPermissionManager;
use crate::orchestrator::ServiceOrchestrator;
use crate::peer::PeerManagerConnector;
#[cfg(feature = "challenge-authorization")]
use crate::public_key::PublicKey;
#[cfg(feature = "service-arg-validation")]
use crate::service::validation::ServiceArgValidator;

use super::shared::AdminServiceShared;
use super::{admin_service_id, AdminKeyVerifier, AdminService};

const DEFAULT_COORDINATOR_TIMEOUT: u64 = 30; // 30 seconds

/// AdminService builder.
///
/// This builder constructs an AdminService.  The Admin service created is prepared for use in a
/// ServiceProcessor.  It is not started once built, but must be started via the Service::start
/// method.
#[derive(Default)]
pub struct AdminServiceBuilder {
    node_id: Option<String>,
    orchestrator: Option<ServiceOrchestrator>,
    #[cfg(feature = "service-arg-validation")]
    service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
    peer_connector: Option<PeerManagerConnector>,
    admin_store: Option<Box<dyn AdminServiceStore>>,
    signature_verifier: Option<Box<dyn SignatureVerifier>>,
    key_verifier: Option<Box<dyn AdminKeyVerifier>>,
    key_permission_manager: Option<Box<dyn KeyPermissionManager>>,
    coordinator_timeout: Option<Duration>,
    routing_table_writer: Option<Box<dyn RoutingTableWriter>>,
    event_store: Option<Box<dyn AdminServiceStore>>,
    #[cfg(feature = "challenge-authorization")]
    public_keys: Option<Vec<PublicKey>>,
}

impl AdminServiceBuilder {
    /// Constructs a new AdminServiceBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the node for the service.
    pub fn with_node_id(mut self, node_id: String) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Sets the service orchestrator.
    pub fn with_service_orchestrator(mut self, orchestrator: ServiceOrchestrator) -> Self {
        self.orchestrator = Some(orchestrator);
        self
    }

    /// Sets the service argument validators.
    ///
    /// The service argument validators are mapped by service type.
    #[cfg(feature = "service-arg-validation")]
    pub fn with_service_arg_validators(
        mut self,
        service_arg_validators: HashMap<String, Box<dyn ServiceArgValidator + Send>>,
    ) -> Self {
        self.service_arg_validators = service_arg_validators;
        self
    }

    /// Sets the peer manager connector.
    pub fn with_peer_manager_connector(mut self, peer_connector: PeerManagerConnector) -> Self {
        self.peer_connector = Some(peer_connector);
        self
    }

    /// Sets the admin service store instance.
    pub fn with_admin_service_store(
        mut self,
        admin_service_store: Box<dyn AdminServiceStore>,
    ) -> Self {
        self.admin_store = Some(admin_service_store);
        self
    }

    /// Sets the signature verifier instance.
    pub fn with_signature_verifier(
        mut self,
        signature_verifier: Box<dyn SignatureVerifier>,
    ) -> Self {
        self.signature_verifier = Some(signature_verifier);
        self
    }

    /// Sets the admin key verifier instance.
    pub fn with_admin_key_verifier(
        mut self,
        admin_key_verifier: Box<dyn AdminKeyVerifier>,
    ) -> Self {
        self.key_verifier = Some(admin_key_verifier);
        self
    }

    /// Sets the key permission manager instance.
    pub fn with_key_permission_manager(
        mut self,
        key_permission_manager: Box<dyn KeyPermissionManager>,
    ) -> Self {
        self.key_permission_manager = Some(key_permission_manager);
        self
    }

    /// Sets the coordinator timeout for the two-phase commit consensus engine.
    pub fn with_coordinator_timeout(mut self, coordinator_timeout: Duration) -> Self {
        self.coordinator_timeout = Some(coordinator_timeout);
        self
    }

    /// Sets the routing table writer instance.
    pub fn with_routing_table_writer(
        mut self,
        routing_table_writer: Box<dyn RoutingTableWriter>,
    ) -> Self {
        self.routing_table_writer = Some(routing_table_writer);
        self
    }

    /// Sets the admin event store instance.
    pub fn with_admin_event_store(mut self, event_store: Box<dyn AdminServiceStore>) -> Self {
        self.event_store = Some(event_store);

        self
    }

    /// Sets the public keys
    #[cfg(feature = "challenge-authorization")]
    pub fn with_public_keys(mut self, public_keys: Vec<PublicKey>) -> Self {
        self.public_keys = Some(public_keys);

        self
    }

    /// Constructs the AdminServce.
    ///
    /// # Errors
    ///
    /// Returns an [InvalidStateError] if any required properties are missing.
    pub fn build(self) -> Result<super::AdminService, InvalidStateError> {
        let coordinator_timeout = self
            .coordinator_timeout
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_COORDINATOR_TIMEOUT));

        let orchestrator = self.orchestrator.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires a service_orchestrator".into(),
            )
        })?;
        let orchestrator = Arc::new(Mutex::new(orchestrator));

        let node_id = self.node_id.ok_or_else(|| {
            InvalidStateError::with_message("An admin service requires a node_id".into())
        })?;

        #[cfg(feature = "service-arg-validation")]
        let service_arg_validators = self.service_arg_validators;

        let admin_store = self.admin_store.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires an admin_service_store".into(),
            )
        })?;

        let peer_connector = self.peer_connector.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires a peer_manager_connector".into(),
            )
        })?;

        let signature_verifier = self.signature_verifier.ok_or_else(|| {
            InvalidStateError::with_message("An admin service requires a signature_verifier".into())
        })?;

        let key_verifier = self.key_verifier.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires an admin_key_verifier".into(),
            )
        })?;
        let key_permission_manager = self.key_permission_manager.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires a key_permission_manager".into(),
            )
        })?;

        let routing_table_writer = self.routing_table_writer.ok_or_else(|| {
            InvalidStateError::with_message(
                "An admin service requires an routing_table_writer".into(),
            )
        })?;

        let admin_event_store = self.event_store.ok_or_else(|| {
            InvalidStateError::with_message("An admin service requires an admin_event_store".into())
        })?;

        let service_id = admin_service_id(&node_id);

        #[cfg(feature = "challenge-authorization")]
        let public_keys = self.public_keys.unwrap_or_default();

        let admin_service_shared = Arc::new(Mutex::new(AdminServiceShared::new(
            node_id.clone(),
            orchestrator.clone(),
            #[cfg(feature = "service-arg-validation")]
            service_arg_validators,
            peer_connector.clone(),
            admin_store,
            signature_verifier,
            key_verifier,
            key_permission_manager,
            routing_table_writer,
            admin_event_store,
            #[cfg(feature = "challenge-authorization")]
            public_keys,
        )));

        Ok(AdminService {
            service_id,
            node_id,
            admin_service_shared,
            orchestrator,
            coordinator_timeout,
            consensus: None,
            peer_connector,
            peer_notification_run_state: None,
        })
    }
}
