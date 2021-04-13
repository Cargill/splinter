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

//! Provides an builder for OAuthConfig from string values.

use splinter::error::InvalidStateError;
use splinter::rest_api::OAuthConfig;
use splinter::store::StoreFactory;

enum RunnableOAuthProvider {
    Azure {
        oauth_openid_url: String,
    },
    Github,
    Google,
    OpenId {
        oauth_openid_url: String,
        auth_params: Option<Vec<(String, String)>>,
        scopes: Option<Vec<String>>,
    },
}

/// A configured, but not fully runnable OAuth configuration.
pub struct RunnableOAuthConfig {
    client_id: String,
    client_secret: String,
    redirect_url: String,
    provider: RunnableOAuthProvider,
}

impl RunnableOAuthConfig {
    /// Convert this runnable config into an OAuthConfig using the provided store factory.
    pub fn into_oauth_config(self, store_factory: &dyn StoreFactory) -> OAuthConfig {
        match self.provider {
            RunnableOAuthProvider::Azure { oauth_openid_url } => OAuthConfig::Azure {
                client_id: self.client_id,
                client_secret: self.client_secret,
                redirect_url: self.redirect_url,
                oauth_openid_url,
                inflight_request_store: store_factory.get_oauth_inflight_request_store(),
            },
            RunnableOAuthProvider::Github => OAuthConfig::GitHub {
                client_id: self.client_id,
                client_secret: self.client_secret,
                redirect_url: self.redirect_url,
                inflight_request_store: store_factory.get_oauth_inflight_request_store(),
            },
            RunnableOAuthProvider::Google => OAuthConfig::Google {
                client_id: self.client_id,
                client_secret: self.client_secret,
                redirect_url: self.redirect_url,
                inflight_request_store: store_factory.get_oauth_inflight_request_store(),
            },
            RunnableOAuthProvider::OpenId {
                oauth_openid_url,
                auth_params,
                scopes,
            } => OAuthConfig::OpenId {
                client_id: self.client_id,
                client_secret: self.client_secret,
                redirect_url: self.redirect_url,
                oauth_openid_url,
                auth_params,
                scopes,
                inflight_request_store: store_factory.get_oauth_inflight_request_store(),
            },
        }
    }
}

/// Builds OAuthConfig values.
///
/// This builder takes string values and produces valid OAuthConfig structs, if the string values
/// match.  It also may return None, based on paramters, if no OAuth configuration was specified.
#[derive(Default)]
pub struct OAuthConfigBuilder {
    oauth_provider: Option<String>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    oauth_redirect_url: Option<String>,
    oauth_openid_url: Option<String>,
    oauth_openid_auth_params: Option<Vec<(String, String)>>,
    oauth_openid_scopes: Option<Vec<String>>,
}

impl OAuthConfigBuilder {
    /// Sets the OAuth provider type.
    ///
    /// Required to produce `Some(OAuthConfig)` as a build result.
    pub fn with_oauth_provider(mut self, value: Option<String>) -> Self {
        self.oauth_provider = value;
        self
    }

    /// Sets the OAuth client ID.
    ///
    /// Required to produce `Some(OAuthConfig)` as a build result.
    pub fn with_oauth_client_id(mut self, value: Option<String>) -> Self {
        self.oauth_client_id = value;
        self
    }

    /// Sets the OAuth client secret.
    ///
    /// Required to produce `Some(OAuthConfig)` as a build result.
    pub fn with_oauth_client_secret(mut self, value: Option<String>) -> Self {
        self.oauth_client_secret = value;
        self
    }

    /// Sets the OAuth redirect URL.
    ///
    /// Required to produce `Some(OAuthConfig)` as a build result.
    pub fn with_oauth_redirect_url(mut self, value: Option<String>) -> Self {
        self.oauth_redirect_url = value;
        self
    }

    /// Sets the OpenID discovery document URL.
    ///
    /// Required if the provider type is either `"azure"` or `"openid"`.
    pub fn with_oauth_openid_url(mut self, value: Option<String>) -> Self {
        self.oauth_openid_url = value;
        self
    }

    /// Sets the OpenID authorization parameters.
    ///
    /// Optional for the provider type `"openid"`.
    pub fn with_oauth_openid_auth_params(mut self, value: Option<Vec<(String, String)>>) -> Self {
        self.oauth_openid_auth_params = value;
        self
    }

    /// Sets the OpenID authorized scopes.
    ///
    /// Optional for the provider type `"openid"`.
    pub fn with_oauth_openid_scopes(mut self, value: Option<Vec<String>>) -> Self {
        self.oauth_openid_scopes = value;
        self
    }

    /// Builds the RunnableOAuthConfig.
    ///
    /// # Returns
    ///
    /// Returns `Some(RunnableOAuthConfig)` if the oauth provider, client ID, client secret and
    /// redirect URL fields are all provided with a `Some` value. Otherwise, returns `None`.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidStateError` if there are missing fields required for a particular
    /// provider or any one (but not all) of the oauth provider, client ID, client secret and
    /// redirect URL fields are not provided.
    pub fn build(self) -> Result<Option<RunnableOAuthConfig>, InvalidStateError> {
        let any_oauth_args_provided = self.oauth_provider.is_some()
            || self.oauth_client_id.is_some()
            || self.oauth_client_secret.is_some()
            || self.oauth_redirect_url.is_some();
        if any_oauth_args_provided {
            let oauth_provider = self.oauth_provider.as_deref().ok_or_else(|| {
                InvalidStateError::with_message("missing OAuth provider configuration".into())
            })?;
            let client_id = self.oauth_client_id.clone().ok_or_else(|| {
                InvalidStateError::with_message("missing OAuth client ID configuration".into())
            })?;
            let client_secret = self.oauth_client_secret.clone().ok_or_else(|| {
                InvalidStateError::with_message("missing OAuth client secret configuration".into())
            })?;
            let redirect_url = self.oauth_redirect_url.clone().ok_or_else(|| {
                InvalidStateError::with_message("missing OAuth redirect URL configuration".into())
            })?;
            let provider = match oauth_provider {
                "azure" => RunnableOAuthProvider::Azure {
                    oauth_openid_url: self.oauth_openid_url.clone().ok_or_else(|| {
                        InvalidStateError::with_message(
                            "missing OAuth OpenID discovery document URL configuration".into(),
                        )
                    })?,
                },
                "github" => RunnableOAuthProvider::Github,
                "google" => RunnableOAuthProvider::Google,
                "openid" => RunnableOAuthProvider::OpenId {
                    oauth_openid_url: self.oauth_openid_url.clone().ok_or_else(|| {
                        InvalidStateError::with_message(
                            "missing OAuth OpenID discovery document URL configuration".into(),
                        )
                    })?,
                    auth_params: self.oauth_openid_auth_params,
                    scopes: self.oauth_openid_scopes,
                },
                other_provider => {
                    return Err(InvalidStateError::with_message(format!(
                        "invalid OAuth provider: {}",
                        other_provider
                    )))
                }
            };

            Ok(Some(RunnableOAuthConfig {
                client_id,
                client_secret,
                redirect_url,
                provider,
            }))
        } else {
            Ok(None)
        }
    }
}
