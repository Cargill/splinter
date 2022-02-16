// Copyright 2018-2022 Cargill Incorporated
//

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

//! Contains the implementation of `RestApiBuilder`.

#[cfg(any(feature = "cylinder-jwt", feature = "store-factory"))]
use std::sync::{Arc, Mutex};

#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::AuthorizationHandler;
#[cfg(feature = "cylinder-jwt")]
use crate::rest_api::auth::identity::cylinder::CylinderKeyIdentityProvider;
use crate::rest_api::{auth::identity::IdentityProvider, BindConfig, RestApiServerError};
#[cfg(feature = "store-factory")]
use crate::store::StoreFactory;

use super::{AuthConfig, ResourceProvider, RunnableRestApi};

/// Builds a `RunnableRestApi`.
///
/// This builder's primary function is to create the runnable REST API in a valid state.
#[derive(Default)]
pub struct RestApiBuilder {
    resource_providers: Vec<Box<dyn ResourceProvider>>,
    bind: Option<BindConfig>,
    #[cfg(feature = "store-factory")]
    store_factory: Option<Box<dyn StoreFactory + Send>>,
    #[cfg(feature = "authorization")]
    authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    identity_providers: Vec<Box<dyn IdentityProvider>>,
}

impl RestApiBuilder {
    /// Constructs a new `RestApiBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bind value, which will be used when binding to one or more ports.
    pub fn with_bind(mut self, value: BindConfig) -> Self {
        self.bind = Some(value);
        self
    }

    #[cfg(feature = "store-factory")]
    pub fn with_store_factory(mut self, factory: Box<dyn StoreFactory + Send>) -> Self {
        self.store_factory = Some(factory.into());
        self
    }

    /// Appends a resource provider to the internal list kept by the builder. The resource
    /// providers' resources will be used when starting up the REST API, and thus determine the
    /// available endpoints.
    pub fn add_resource_provider(mut self, resource_provider: Box<dyn ResourceProvider>) -> Self {
        self.resource_providers.push(resource_provider);
        self
    }

    /// Appends auth configs to the existing vec of configs.
    pub fn append_auth_configs(self, auth_configs: Vec<AuthConfig>) -> Self {
        auth_configs
            .into_iter()
            .fold(self, |s, config| s.push_auth_config(config))
    }

    // Pushes a single auth config onto the auth config vector.
    pub fn push_auth_config(mut self, auth_config: AuthConfig) -> Self {
        match auth_config {
            #[cfg(feature = "biome-credentials")]
            AuthConfig::Biome {
                biome_credentials_resource_provider: _,
            } => {
                // Not adding biome support atm
                unimplemented!();
            }
            #[cfg(feature = "cylinder-jwt")]
            AuthConfig::Cylinder { verifier } => {
                self.identity_providers
                    .push(Box::new(CylinderKeyIdentityProvider::new(Arc::new(
                        Mutex::new(verifier),
                    ))));
            }
            #[cfg(feature = "oauth")]
            AuthConfig::OAuth {
                oauth_config: _,
                oauth_user_session_store: _,
                #[cfg(feature = "biome-profile")]
                    user_profile_store: _,
            } => {
                // Not adding actual oauth support atm.
                // The Resource Provider trait hasn't been implemented for it and since
                // actix-web-1 is all over that code I am leaving it till after proof of
                // concept.
                unimplemented!();
            }
            AuthConfig::Custom {
                resource_provider,
                identity_provider,
            } => {
                self.resource_providers.push(resource_provider);
                self.identity_providers.push(identity_provider);
            }
        }
        self
    }

    /// Validate the arguments and build the `RunnableRestApi` struct.
    pub fn build(self) -> Result<RunnableRestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        Ok(RunnableRestApi {
            bind,
            resource_providers: self.resource_providers,
            #[cfg(feature = "store-factory")]
            store_factory: self.store_factory,
            #[cfg(feature = "authorization")]
            identity_providers: self.identity_providers,
            #[cfg(feature = "authorization")]
            authorization_handlers: self.authorization_handlers,
        })
    }

    /// Add authorization_handlers
    #[cfg(feature = "authorization")]
    pub fn with_authorization_handlers(
        mut self,
        authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    ) -> Self {
        self.authorization_handlers = authorization_handlers;
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Verifies that the `RestApiThreadBuilder` builds succesfully when all required configuration
    /// is provided.
    #[test]
    fn rest_api_thread_builder_successful() {
        let builder = RestApiBuilder::new().with_bind(BindConfig::Http("test".into()));

        assert!(builder.build().is_ok())
    }
}
