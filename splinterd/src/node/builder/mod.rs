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
pub(super) mod network;

use std::time::Duration;

use cylinder::{secp256k1::Secp256k1Context, Context, Signer};
use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::{AuthConfig, RestApiBuilder as RestApiBuilder1};
use splinter::rest_api::actix_web_3::RestApiBuilder as RestApiBuilder3;
use splinter::rest_api::auth::{
    authorization::{AuthorizationHandler, AuthorizationHandlerResult},
    identity::{Identity, IdentityProvider},
    AuthorizationHeader,
};
use splinter::rest_api::RestApiBind;
use splinter::store::StoreFactory;

use super::{RunnableNode, RunnableNodeRestApiVariant};

use self::admin::AdminSubsystemBuilder;

/// An enumeration of the REST API backend variants.
#[derive(Clone, Copy, Debug)]
pub enum RestApiVariant {
    /// Actix Web 1 as the backend implementation
    ActixWeb1,

    /// Actix Web 3 as the backend implementation
    ActixWeb3,
}

/// Constructs a `RunnableNode` instance.
pub struct NodeBuilder {
    admin_subsystem_builder: AdminSubsystemBuilder,
    admin_signer: Option<Box<dyn Signer>>,
    rest_api_port: Option<u32>,
    rest_api_variant: RestApiVariant,
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
            admin_signer: None,
            rest_api_port: None,
            rest_api_variant: RestApiVariant::ActixWeb1,
        }
    }

    /// Specifies the id for the node. Defaults to a random node id.
    pub fn with_node_id(mut self, node_id: String) -> Self {
        self.admin_subsystem_builder = self.admin_subsystem_builder.with_node_id(node_id);
        self
    }

    /// Specifies the private key that will be used for signing admin payloads against the final
    /// node.
    pub fn with_admin_signer(mut self, signer: Box<dyn Signer>) -> Self {
        self.admin_signer = Some(signer);
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
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_heartbeat_interval(heartbeat_interval);
        self
    }

    /// Configure whether or not strict reference counts will be used in the peer manager. Defaults
    /// to false.
    pub fn with_strict_ref_counts(mut self, strict_ref_counts: bool) -> Self {
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_strict_ref_counts(strict_ref_counts);
        self
    }

    /// Specifies the store factory to use with the node. Defaults to the MemoryStoreFactory.
    pub fn with_store_factory(mut self, store_factory: Box<dyn StoreFactory>) -> Self {
        self.admin_subsystem_builder = self
            .admin_subsystem_builder
            .with_store_factory(store_factory);
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

    /// Builds the `RunnableNode` and consumes the `NodeBuilder`.
    pub fn build(mut self) -> Result<RunnableNode, InternalError> {
        let url = format!("127.0.0.1:{}", self.rest_api_port.take().unwrap_or(0),);

        let runnable_admin_subsystem = self.admin_subsystem_builder.build()?;

        let admin_signer = self.admin_signer.take().unwrap_or_else(|| {
            let context = Secp256k1Context::new();
            let pk = context.new_random_private_key();
            context.new_signer(pk)
        });

        let rest_api_variant = match self.rest_api_variant {
            RestApiVariant::ActixWeb1 => {
                let auth_config = AuthConfig::Custom {
                    resources: vec![],
                    identity_provider: Box::new(MockIdentityProvider),
                };

                RunnableNodeRestApiVariant::ActixWeb1(
                    RestApiBuilder1::new()
                        .with_bind(RestApiBind::Insecure(url))
                        .with_auth_configs(vec![auth_config])
                        .with_authorization_handlers(vec![Box::new(MockAuthorizationHandler)])
                        .build()
                        .map_err(|e| InternalError::from_source(Box::new(e)))?,
                )
            }
            RestApiVariant::ActixWeb3 => RunnableNodeRestApiVariant::ActixWeb3(
                RestApiBuilder3::new()
                    .with_bind(RestApiBind::Insecure(url))
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
            ),
        };

        Ok(RunnableNode {
            admin_signer,
            runnable_admin_subsystem,
            rest_api_variant,
        })
    }
}

#[derive(Clone)]
struct MockIdentityProvider;

impl IdentityProvider for MockIdentityProvider {
    fn get_identity(
        &self,
        _authorization: &AuthorizationHeader,
    ) -> Result<Option<Identity>, InternalError> {
        Ok(Some(Identity::Custom("".into())))
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
