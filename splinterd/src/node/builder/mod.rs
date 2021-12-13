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

pub(super) mod admin;
pub(super) mod biome;
pub(super) mod network;
pub(super) mod scabbard;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cylinder::{secp256k1::Secp256k1Context, Context, Signer, Verifier, VerifierFactory};
use rand::{thread_rng, Rng};
use splinter::biome::credentials::rest_api::{
    BiomeCredentialsRestResourceProvider, BiomeCredentialsRestResourceProviderBuilder,
};
use splinter::error::InternalError;
use splinter::public_key::PublicKey;
use splinter::rest_api::actix_web_1::RestApiBuilder as RestApiBuilder1;
use splinter::rest_api::auth::authorization::rbac::RoleBasedAuthorizationHandler;
use splinter::rest_api::auth::{
    authorization::{
        rbac::store::{AssignmentBuilder, Identity as AssignmentIdentity, RoleBuilder},
        AuthorizationHandler, AuthorizationHandlerResult,
    },
    identity::Identity,
};
use splinter::rest_api::BindConfig;
use splinter::store::{memory::MemoryStoreFactory, StoreFactory};

use super::{RunnableNode, RunnableNodeRestApiVariant, ScabbardConfig};

use self::admin::{AdminServiceEventClientVariant, AdminSubsystemBuilder};
use self::biome::BiomeSubsystemBuilder;
use self::network::NetworkSubsystemBuilder;

/// An enumeration of the REST API backend variants.
#[derive(Clone, Copy, Debug)]
pub enum RestApiVariant {
    /// Actix Web 1 as the backend implementation
    ActixWeb1,
}

/// Constructs a `RunnableNode` instance.
pub struct NodeBuilder {
    admin_subsystem_builder: AdminSubsystemBuilder,
    biome_subsystem_builder: BiomeSubsystemBuilder,
    admin_signer: Option<Box<dyn Signer>>,
    rest_api_port: Option<u32>,
    rest_api_variant: RestApiVariant,
    network_subsystem_builder: NetworkSubsystemBuilder,
    node_id: Option<String>,
    enable_biome: bool,
    signers: Option<Vec<Box<dyn Signer>>>,
    biome_auth: Option<BiomeCredentialsRestResourceProvider>,
    cylinder_auth: Option<Box<dyn Verifier>>,
    permission_config: Option<Vec<PermissionConfig>>,
    store_factory: Option<Box<dyn StoreFactory>>,
}

impl Default for NodeBuilder {
    fn default() -> Self {
        NodeBuilder::new()
    }
}

impl NodeBuilder {
    /// Constructs new `NodeBuilder`.
    pub fn new() -> Self {
        NodeBuilder {
            admin_subsystem_builder: AdminSubsystemBuilder::new(),
            biome_subsystem_builder: BiomeSubsystemBuilder::new(),
            admin_signer: None,
            rest_api_port: None,
            rest_api_variant: RestApiVariant::ActixWeb1,
            network_subsystem_builder: NetworkSubsystemBuilder::new(),
            node_id: None,
            enable_biome: false,
            signers: None,
            biome_auth: None,
            cylinder_auth: None,
            permission_config: None,
            store_factory: None,
        }
    }

    /// Specifies the id for the node. Defaults to a random node id.
    pub fn with_node_id(mut self, node_id: String) -> Self {
        self.node_id = Some(node_id);
        self
    }

    /// Specifies the private key that will be used for signing admin payloads against the final
    /// node.
    pub fn with_admin_signer(mut self, signer: Box<dyn Signer>) -> Self {
        self.admin_signer = Some(signer);
        self
    }

    /// Specifies the private key that will be used for challenge authorization
    pub fn with_signers(mut self, signers: Vec<Box<dyn Signer>>) -> Self {
        self.signers = Some(signers);
        self
    }

    /// Specifies the timeout for admin requests. Defaults to 30 seconds.
    pub fn with_admin_timeout(mut self, admin_timeout: Duration) -> Self {
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_admin_timeout(admin_timeout);
        self
    }

    /// Specifies the heartbeat interval between peer connections. Defaults to 30 seconds.
    pub fn with_heartbeat_interval(mut self, heartbeat_interval: Duration) -> Self {
        self.network_subsystem_builder = self
            .network_subsystem_builder
            .with_heartbeat_interval(heartbeat_interval);
        self
    }

    /// Configure whether or not strict reference counts will be used in the peer manager. Defaults
    /// to false.
    pub fn with_strict_ref_counts(mut self, strict_ref_counts: bool) -> Self {
        self.network_subsystem_builder = self
            .network_subsystem_builder
            .with_strict_ref_counts(strict_ref_counts);
        self
    }

    /// Specifies the store factory to use with the node. Defaults to the MemoryStoreFactory.
    pub fn with_store_factory(mut self, store_factory: Box<dyn StoreFactory>) -> Self {
        self.store_factory = Some(store_factory);
        self
    }

    pub fn with_admin_service_event_client_variant(
        mut self,
        admin_service_event_client_variant: AdminServiceEventClientVariant,
    ) -> Self {
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_admin_service_event_client_variant(admin_service_event_client_variant);

        self
    }

    /// Specifies the REST API port which should be used when binding the REST API.
    pub fn with_rest_api_port(mut self, port: u32) -> Self {
        self.rest_api_port = Some(port);
        self
    }

    /// Specifies the REST API variant to use as an implementation of the REST API.
    pub fn with_rest_api_variant(mut self, variant: RestApiVariant) -> Self {
        self.rest_api_variant = variant;
        self
    }

    /// Specifies the network endpoints for the node
    pub fn with_network_endpoints(mut self, network_endpoints: Vec<String>) -> Self {
        self.network_subsystem_builder = self
            .network_subsystem_builder
            .with_network_endpoints(network_endpoints);
        self
    }

    /// Make scabbard services available for circuits.
    pub fn with_scabbard(mut self, scabbard_config: ScabbardConfig) -> Self {
        self.admin_subsystem_builder = self.admin_subsystem_builder.with_scabbard(scabbard_config);
        self
    }

    /// Specifies any external registry files to be used in the unified registry.
    pub fn with_external_registries(mut self, registries: Option<Vec<String>>) -> Self {
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_external_registries(registries);
        self
    }

    /// Make Biome resources available on the network
    pub fn with_biome_enabled(mut self) -> Self {
        self.enable_biome = true;
        self
    }

    pub fn with_permission_config(
        mut self,
        permission_config: Option<Vec<PermissionConfig>>,
    ) -> Self {
        self.permission_config = permission_config;
        self
    }

    /// Enable Biome Auth
    #[cfg(feature = "biome-credentials")]
    pub fn with_biome_auth(
        self,
        key_store: Box<dyn splinter::biome::KeyStore>,
        refresh_token_store: Box<dyn splinter::biome::RefreshTokenStore>,
        credential_store: Box<dyn splinter::biome::CredentialsStore>,
    ) -> Self {
        let biome_auth = BiomeCredentialsRestResourceProviderBuilder::default()
            .with_key_store(key_store)
            .with_refresh_token_store(refresh_token_store)
            .with_credentials_store(credential_store)
            .build()
            .ok();
        Self { biome_auth, ..self }
    }

    /// Enable Cylinder Auth
    pub fn with_cylinder_auth(self, context: Box<dyn Context>) -> Self {
        let verifier = context.new_verifier();
        let cylinder_auth = Some(verifier);
        Self {
            cylinder_auth,
            ..self
        }
    }

    /// Builds the `RunnableNode` and consumes the `NodeBuilder`.
    pub fn build(mut self) -> Result<RunnableNode, InternalError> {
        let url = format!("127.0.0.1:{}", self.rest_api_port.take().unwrap_or(0),);

        let node_id = self
            .node_id
            .take()
            .unwrap_or_else(|| format!("n{}", thread_rng().gen::<u16>().to_string()));

        let context = Secp256k1Context::new();

        let admin_signer = self.admin_signer.take().unwrap_or_else(|| {
            let pk = context.new_random_private_key();
            context.new_signer(pk)
        });

        let signers = self
            .signers
            .take()
            .unwrap_or_else(|| vec![admin_signer.clone()]);

        let signing_context: Arc<Mutex<Box<dyn VerifierFactory>>> =
            Arc::new(Mutex::new(Box::new(context)));

        let runnable_network_subsystem = self
            .network_subsystem_builder
            .with_node_id(node_id.clone())
            .with_signing_context(signing_context.clone())
            .with_signers(signers.clone())
            .build()?;

        let store_factory = match self.store_factory {
            Some(store_factory) => store_factory,
            None => Box::new(MemoryStoreFactory::new()?),
        };

        let rbac_store = store_factory.get_role_based_authorization_store();

        let profile_store = store_factory.get_biome_user_profile_store();

        // Sets permissions if any were given
        if let Some(ref permission_config) = self.permission_config {
            for (i, perm) in permission_config.iter().enumerate() {
                let permissions = &perm.permissions();
                let pub_key = perm
                    .signer()
                    .public_key()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let role = RoleBuilder::new()
                    .with_id(format!("{}", i))
                    .with_display_name(format!("{}", i))
                    .with_permissions(permissions.to_vec())
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                let assignment = AssignmentBuilder::new()
                    .with_identity(AssignmentIdentity::Key(pub_key.as_hex()))
                    .with_roles(vec![format!("{}", i)])
                    .build()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;

                rbac_store
                    .add_role(role)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                rbac_store
                    .add_assignment(assignment)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
            }
        };

        let admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_node_id(node_id.clone())
            .with_signing_context(signing_context.clone())
            .with_public_keys(
                signers
                    .iter()
                    .map(|signer| {
                        signer
                            .public_key()
                            .map(|public_key| public_key.into())
                            .map_err(|err| InternalError::from_source(Box::new(err)))
                    })
                    .collect::<Result<Vec<PublicKey>, InternalError>>()?,
            )
            .with_store_factory(store_factory);

        let biome_subsystem_builder = self
            .biome_subsystem_builder
            .with_profile_store(profile_store);

        let authorization_handlers: Vec<Box<dyn AuthorizationHandler>> =
            match self.permission_config {
                Some(_) => {
                    vec![Box::new(RoleBasedAuthorizationHandler::new(
                        rbac_store.clone(),
                    ))]
                }
                None => vec![Box::new(MockAuthorizationHandler)],
            };

        let rest_api_variant = match self.rest_api_variant {
            RestApiVariant::ActixWeb1 => RunnableNodeRestApiVariant::ActixWeb1(
                RestApiBuilder1::new()
                    .with_bind(BindConfig::Http(url))
                    .with_authorization_handlers(authorization_handlers),
            ),
        };

        #[cfg(feature = "biome-credentials")]
        let rest_api_variant = if let Some(biome) = self.biome_auth {
            rest_api_variant.with_biome_auth(biome)
        } else {
            rest_api_variant
        };

        let rest_api_variant = if let Some(cylinder) = self.cylinder_auth {
            rest_api_variant.with_cylinder_auth(cylinder)
        } else {
            rest_api_variant
        };

        let enable_biome = self.enable_biome;

        Ok(RunnableNode {
            admin_signer,
            admin_subsystem_builder,
            biome_subsystem_builder,
            rest_api_variant,
            runnable_network_subsystem,
            node_id,
            enable_biome,
            signers,
        })
    }
}

#[derive(Clone)]
pub struct PermissionConfig {
    permissions: Vec<String>,
    signer: Box<dyn Signer>,
}

impl PermissionConfig {
    pub fn new(permissions: Vec<String>, signer: Box<dyn Signer>) -> Self {
        Self {
            permissions,
            signer,
        }
    }

    pub fn permissions(&self) -> Vec<String> {
        self.permissions.clone()
    }
    pub fn signer(&self) -> Box<dyn Signer> {
        self.signer.clone()
    }
}

struct MockAuthorizationHandler;

impl AuthorizationHandler for MockAuthorizationHandler {
    fn has_permission(
        &self,
        _identity: &Identity,
        _permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError> {
        Ok(AuthorizationHandlerResult::Allow)
    }

    /// Clone implementation for `AuthorizationHandler`. The implementation of the `Clone` trait for
    /// `Box<dyn AuthorizationHandler>` calls this method.
    fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
        Box::new(MockAuthorizationHandler)
    }
}
