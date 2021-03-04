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

#[cfg(feature = "cylinder-jwt")]
use std::sync::Arc;
#[cfg(feature = "cylinder-jwt")]
use std::sync::Mutex;

use crate::error::InvalidStateError;
#[cfg(feature = "oauth")]
use crate::oauth::{GithubOAuthClientBuilder, OpenIdOAuthClientBuilder};
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::AuthorizationHandler;
#[cfg(feature = "cylinder-jwt")]
use crate::rest_api::auth::identity::cylinder::CylinderKeyIdentityProvider;
#[cfg(feature = "oauth")]
use crate::rest_api::{
    auth::identity::oauth::OAuthUserIdentityProvider, OAuthConfig, OAuthResourceProvider,
};
use crate::rest_api::{auth::identity::IdentityProvider, RestApiBind, RestApiServerError};

use super::AuthConfig;
#[cfg(any(feature = "biome-credentials", feature = "oauth"))]
use super::RestResourceProvider;
use super::{Resource, RestApi};

/// Builder `struct` for `RestApi`.
pub struct RestApiBuilder {
    resources: Vec<Resource>,
    bind: Option<RestApiBind>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    auth_configs: Vec<AuthConfig>,
    #[cfg(feature = "authorization")]
    authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
}

impl Default for RestApiBuilder {
    fn default() -> Self {
        Self {
            resources: Vec::new(),
            bind: None,
            #[cfg(feature = "rest-api-cors")]
            whitelist: None,
            auth_configs: Vec::new(),
            #[cfg(feature = "authorization")]
            authorization_handlers: Vec::new(),
        }
    }
}

impl RestApiBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(feature = "https-bind"))]
    pub fn with_bind(mut self, value: &str) -> Self {
        self.bind = Some(RestApiBind::Insecure(value.to_string()));
        self
    }

    #[cfg(feature = "https-bind")]
    pub fn with_bind(mut self, value: RestApiBind) -> Self {
        self.bind = Some(value);
        self
    }

    pub fn add_resource(mut self, value: Resource) -> Self {
        self.resources.push(value);
        self
    }

    pub fn add_resources(mut self, mut values: Vec<Resource>) -> Self {
        self.resources.append(&mut values);
        self
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn with_whitelist(mut self, values: Vec<String>) -> Self {
        self.whitelist = Some(values);
        self
    }

    pub fn with_auth_configs(mut self, auth_configs: Vec<AuthConfig>) -> Self {
        self.auth_configs = auth_configs;
        self
    }

    #[cfg(feature = "authorization")]
    pub fn with_authorization_handlers(
        mut self,
        authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
    ) -> Self {
        self.authorization_handlers = authorization_handlers;
        self
    }

    // Allowing unused_mut because self must be mutable if feature `auth` is enabled
    #[allow(unused_mut)]
    pub fn build(mut self) -> Result<RestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        let identity_providers = {
            if self.auth_configs.is_empty() {
                return Err(RestApiServerError::InvalidStateError(
                    InvalidStateError::with_message(
                        "REST API auth is enabled, but no auth has been configured".to_string(),
                    ),
                ));
            }

            let mut identity_providers = Vec::<Box<dyn IdentityProvider>>::new();
            #[cfg(feature = "oauth")]
            let mut oauth_configured = false;

            for auth_config in self.auth_configs.into_iter() {
                match auth_config {
                    #[cfg(feature = "biome-credentials")]
                    AuthConfig::Biome {
                        biome_credentials_resource_provider,
                    } => {
                        identity_providers.push(Box::new(
                            biome_credentials_resource_provider.get_identity_provider(),
                        ));
                        self.resources
                            .append(&mut biome_credentials_resource_provider.resources());
                    }
                    #[cfg(feature = "cylinder-jwt")]
                    AuthConfig::Cylinder { verifier } => {
                        identity_providers.push(Box::new(CylinderKeyIdentityProvider::new(
                            Arc::new(Mutex::new(verifier)),
                        )));
                    }
                    #[cfg(feature = "oauth")]
                    AuthConfig::OAuth {
                        oauth_config,
                        oauth_user_session_store,
                        #[cfg(feature = "biome-profile")]
                        user_profile_store,
                    } => {
                        if oauth_configured {
                            return Err(RestApiServerError::InvalidStateError(
                                InvalidStateError::with_message(
                                    "Only one OAuth provider can be configured".to_string(),
                                ),
                            ));
                        }

                        let oauth_client = match oauth_config {
                            OAuthConfig::Azure {
                                client_id,
                                client_secret,
                                redirect_url,
                                oauth_openid_url,
                                inflight_request_store,
                            } => OpenIdOAuthClientBuilder::new_azure()
                                .with_discovery_url(oauth_openid_url)
                                .with_client_id(client_id)
                                .with_client_secret(client_secret)
                                .with_redirect_url(redirect_url)
                                .with_inflight_request_store(inflight_request_store)
                                .build()?,
                            OAuthConfig::GitHub {
                                client_id,
                                client_secret,
                                redirect_url,
                                inflight_request_store,
                            } => GithubOAuthClientBuilder::new()
                                .with_client_id(client_id)
                                .with_client_secret(client_secret)
                                .with_redirect_url(redirect_url)
                                .with_inflight_request_store(inflight_request_store)
                                .build()?,
                            OAuthConfig::Google {
                                client_id,
                                client_secret,
                                redirect_url,
                                inflight_request_store,
                            } => OpenIdOAuthClientBuilder::new_google()
                                .with_client_id(client_id)
                                .with_client_secret(client_secret)
                                .with_redirect_url(redirect_url)
                                .with_inflight_request_store(inflight_request_store)
                                .build()?,
                            OAuthConfig::OpenId {
                                client_id,
                                client_secret,
                                redirect_url,
                                oauth_openid_url,
                                auth_params,
                                scopes,
                                inflight_request_store,
                            } => {
                                let mut builder = OpenIdOAuthClientBuilder::new()
                                    .with_discovery_url(oauth_openid_url)
                                    .with_client_id(client_id)
                                    .with_client_secret(client_secret)
                                    .with_redirect_url(redirect_url)
                                    .with_inflight_request_store(inflight_request_store);
                                if let Some(auth_params) = auth_params {
                                    builder = builder.with_extra_auth_params(auth_params);
                                }
                                if let Some(scopes) = scopes {
                                    builder = builder.with_scopes(scopes);
                                }
                                builder.build()?
                            }
                        };

                        identity_providers.push(Box::new(OAuthUserIdentityProvider::new(
                            oauth_client.clone(),
                            oauth_user_session_store.clone(),
                            None,
                        )));
                        self.resources.append(
                            &mut OAuthResourceProvider::new(
                                oauth_client,
                                oauth_user_session_store,
                                #[cfg(feature = "biome-profile")]
                                user_profile_store,
                            )
                            .resources(),
                        );
                        oauth_configured = true;
                    }
                    AuthConfig::Custom {
                        mut resources,
                        identity_provider,
                    } => {
                        self.resources.append(&mut resources);
                        identity_providers.push(identity_provider);
                    }
                }
            }

            identity_providers
        };

        Ok(RestApi {
            bind,
            resources: self.resources,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
            identity_providers,
            #[cfg(feature = "authorization")]
            authorization_handlers: self.authorization_handlers,
        })
    }

    /// Builds the `RestApi` without requiring any security configuration
    #[cfg(test)]
    pub fn build_insecure(self) -> Result<RestApi, RestApiServerError> {
        let bind = self
            .bind
            .ok_or_else(|| RestApiServerError::MissingField("bind".to_string()))?;

        let bind = match bind {
            #[cfg(feature = "https-bind")]
            RestApiBind::Secure { bind, .. } => RestApiBind::Insecure(bind),
            insecure @ RestApiBind::Insecure(_) => insecure,
        };

        Ok(RestApi {
            bind,
            resources: self.resources,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
            identity_providers: vec![],
            #[cfg(feature = "authorization")]
            authorization_handlers: vec![],
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::error::InternalError;
    use crate::rest_api::auth::{identity::Identity, AuthorizationHeader};

    /// Verifies that the `RestApiBuilder` builds succesfully when all required configuration is
    /// provided.
    #[test]
    fn rest_api_builder_successful() {
        let mut builder = RestApiBuilder::new();

        #[cfg(not(feature = "https-bind"))]
        {
            builder = builder.with_bind("test");
        }
        #[cfg(feature = "https-bind")]
        {
            builder = builder.with_bind(RestApiBind::Insecure("test".into()));
        }

        let auth_config = AuthConfig::Custom {
            resources: vec![],
            identity_provider: Box::new(MockIdentityProvider),
        };
        builder = builder.with_auth_configs(vec![auth_config]);

        assert!(builder.build().is_ok())
    }

    /// Verifies that the `RestApiBuilder` fails to build when auth is enabled but no auth is
    /// configured.
    #[test]
    fn rest_api_builder_no_auth() {
        #[cfg(feature = "https-bind")]
        let result = RestApiBuilder::new()
            .with_bind(RestApiBind::Insecure("test".into()))
            .build();
        #[cfg(not(feature = "https-bind"))]
        let result = RestApiBuilder::new().with_bind("test").build();

        assert!(matches!(
            result,
            Err(RestApiServerError::InvalidStateError(_))
        ));
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
        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }
}
