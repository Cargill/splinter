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

use splinter::admin::rest_api::actix_web_3::AdminResourceProvider;
use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::{AuthConfig, RestApiBuilder as RestApiBuilder1};
use splinter::rest_api::actix_web_3::RestApiBuilder as RestApiBuilder3;
use splinter::rest_api::auth::{
    identity::{Identity, IdentityProvider},
    AuthorizationHeader,
};
use splinter::rest_api::RestApiBind;

use super::{RunnableNode, RunnableNodeRestApiVariant};

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
            rest_api_port: None,
            rest_api_variant: RestApiVariant::ActixWeb1,
        }
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
    pub fn build(self) -> Result<RunnableNode, InternalError> {
        let url = format!("127.0.0.1:{}", self.rest_api_port.unwrap_or(0),);

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
                        .build()
                        .map_err(|e| InternalError::from_source(Box::new(e)))?,
                )
            }
            RestApiVariant::ActixWeb3 => RunnableNodeRestApiVariant::ActixWeb3(
                RestApiBuilder3::new()
                    .with_bind(RestApiBind::Insecure(url))
                    .add_resource_provider(Box::new(AdminResourceProvider::new()))
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
            ),
        };

        Ok(RunnableNode { rest_api_variant })
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
