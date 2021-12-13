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

//! Contains the implementation of `RunnableNode`.

pub(super) mod admin;
pub(super) mod biome;
pub(super) mod network;

use splinter::biome::credentials::rest_api::BiomeCredentialsRestResourceProvider;
use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::RestApiBuilder;
use splinter::rest_api::{
    auth::{
        authorization::{
            maintenance::MaintenanceModeAuthorizationHandler,
            rbac::rest_api::RoleBasedAuthorizationResourceProvider,
        },
        identity::{Identity, IdentityProvider},
        AuthorizationHeader,
    },
    AuthConfig, RestResourceProvider,
};

use super::builder::admin::AdminSubsystemBuilder;
use super::builder::biome::BiomeSubsystemBuilder;
use super::{BiomeResourceProvider, Node, NodeRestApiVariant};

use self::network::RunnableNetworkSubsystem;

pub(super) enum RunnableNodeRestApiVariant {
    ActixWeb1(RestApiBuilder),
}

impl RunnableNodeRestApiVariant {
    #[cfg(feature = "biome-credentials")]
    pub fn with_biome_auth(self, provider: BiomeCredentialsRestResourceProvider) -> Self {
        match self {
            RunnableNodeRestApiVariant::ActixWeb1(builder) => {
                RunnableNodeRestApiVariant::ActixWeb1(builder.push_auth_config(AuthConfig::Biome {
                    biome_credentials_resource_provider: provider,
                }))
            }
        }
    }
    pub fn with_cylinder_auth(self, verifier: Box<dyn cylinder::Verifier>) -> Self {
        match self {
            RunnableNodeRestApiVariant::ActixWeb1(builder) => {
                RunnableNodeRestApiVariant::ActixWeb1(
                    builder.push_auth_config(AuthConfig::Cylinder { verifier }),
                )
            }
        }
    }
}

/// A fully configured and runnable instance of a node.
pub struct RunnableNode {
    pub(super) admin_signer: Box<dyn cylinder::Signer>,
    pub(super) admin_subsystem_builder: AdminSubsystemBuilder,
    pub(super) biome_subsystem_builder: BiomeSubsystemBuilder,
    pub(super) rest_api_variant: RunnableNodeRestApiVariant,
    pub(super) runnable_network_subsystem: RunnableNetworkSubsystem,
    pub(super) node_id: String,
    pub(super) enable_biome: bool,
    pub(super) signers: Vec<Box<dyn cylinder::Signer>>,
}

impl RunnableNode {
    /// Starts up the Node.
    pub fn run(self) -> Result<Node, InternalError> {
        let network_subsystem = self.runnable_network_subsystem.run()?;

        let runnable_admin_subsystem = self
            .admin_subsystem_builder
            .with_peer_connector(network_subsystem.peer_connector())
            .with_routing_writer(network_subsystem.routing_table_writer())
            .with_service_transport(network_subsystem.service_transport())
            .build()?;

        let mut admin_subsystem = runnable_admin_subsystem.run()?;

        let runnable_biome_subsystem = self.biome_subsystem_builder.build()?;

        let biome_subsystem = runnable_biome_subsystem.run()?;

        let node_id = self.node_id;

        let signers = self.signers;

        let rest_api_variant = match self.rest_api_variant {
            RunnableNodeRestApiVariant::ActixWeb1(rest_api) => {
                let admin_resources = admin_subsystem.take_actix1_resources();
                let mut biome_resources = vec![];
                let mut auth_configs = vec![AuthConfig::Custom {
                    resources: vec![],
                    identity_provider: Box::new(MockIdentityProvider),
                }];

                // Create the `Biome` resources if the node has biome enabled
                if self.enable_biome {
                    // Build the `BiomeResourceProvider` to allow the node to access `Biome` endpoints
                    let mut biome_resource_provider =
                        BiomeResourceProvider::new(&*admin_subsystem.store_factory)?;
                    auth_configs.append(&mut biome_resource_provider.auth_configs);
                    biome_resources.append(&mut biome_resource_provider.take_actix1_resources());
                };

                // Create the authroization handler resources to allow the node to access the
                // authorization endpoints
                let auth_handler_resources = vec![
                    RoleBasedAuthorizationResourceProvider::new(
                        admin_subsystem
                            .store_factory
                            .get_role_based_authorization_store(),
                    )
                    .resources(),
                    MaintenanceModeAuthorizationHandler::new(Some(
                        admin_subsystem
                            .store_factory
                            .get_role_based_authorization_store(),
                    ))
                    .resources(),
                ];

                let (rest_api_shutdown_handle, rest_api_join_handle) = rest_api
                    .append_auth_configs(&mut auth_configs)
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?
                    .add_resources(admin_resources)
                    .add_resources(biome_resources)
                    .add_resources(auth_handler_resources.into_iter().flatten().collect())
                    .run()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;

                let port_numbers = rest_api_shutdown_handle.port_numbers();

                // The REST API's collection of port numbers is busted, so if we
                // see more than one, it is meaningless and we have to abort what
                // we are doing. For example, if you give localhost:0 as a bind
                // argument to the REST API, this will return two ports, one for
                // ipv4 and one for ipv6, it's not clear which is which.
                if port_numbers.len() != 1 {
                    return Err(InternalError::with_message(format!(
                        "Expected a single port number but saw multiple: {:?}",
                        port_numbers
                    )));
                }

                NodeRestApiVariant::ActixWeb1(rest_api_shutdown_handle, rest_api_join_handle)
            }
        };

        let rest_api_port = match &rest_api_variant {
            NodeRestApiVariant::ActixWeb1(shutdown_handle, _) => shutdown_handle.port_numbers()[0],
        };

        let admin_service_event_client = Box::new(
            admin_subsystem
                .admin_service_event_client(
                    format!("http://localhost:{}", rest_api_port),
                    "foo".to_string(),
                    "*".to_string(),
                    None,
                )
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
        );

        Ok(Node {
            admin_signer: self.admin_signer,
            admin_subsystem,
            biome_subsystem,
            network_subsystem,
            rest_api_variant,
            rest_api_port,
            node_id,
            admin_service_event_client,
            signers,
        })
    }
}

#[derive(Clone)]
struct MockIdentityProvider;

impl IdentityProvider for MockIdentityProvider {
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<Identity>, InternalError> {
        match authorization {
            AuthorizationHeader::Custom(_) => Ok(Some(Identity::Custom("".into()))),
            _ => Err(InternalError::with_message(
                "`Authorization` belongs to external IdentityProvider".into(),
            )),
        }
    }

    /// Clones implementation for `IdentityProvider`. The implementation of the `Clone` trait for
    /// `Box<dyn IdentityProvider>` calls this method.
    ///
    /// # Example
    ///
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn IdentityProvider> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
