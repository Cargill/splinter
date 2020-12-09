// Copyright 2018-2020 Cargill Incorporated
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

use reqwest::blocking::Client;

use crate::error::{InternalError, InvalidStateError};
use crate::oauth::{builder::OAuthClientBuilder, error::OAuthClientBuildError, OAuthClient};
use crate::rest_api::auth::identity::{openid::OpenIdUserIdentityProvider, IdentityProvider};

/// Builds a new `OAuthClient` using an OpenID discovery document.
pub struct OpenIdOAuthClientBuilder {
    openid_discovery_url: Option<String>,
    inner: OAuthClientBuilder,
}

impl OpenIdOAuthClientBuilder {
    /// Constructs a new [`OpenIdOAuthClientBuilder`].
    pub fn new() -> Self {
        Self {
            openid_discovery_url: None,
            inner: OAuthClientBuilder::default(),
        }
    }

    /// Sets the client ID for the OAuth2 provider.
    pub fn with_client_id(self, client_id: String) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self.inner.with_client_id(client_id),
        }
    }

    /// Sets the client secret for the OAuth2 provider.
    pub fn with_client_secret(self, client_secret: String) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self.inner.with_client_secret(client_secret),
        }
    }

    /// Sets the redirect URL for the OAuth2 provider.
    pub fn with_redirect_url(self, redirect_url: String) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self.inner.with_redirect_url(redirect_url),
        }
    }

    /// Sets the discovery document URL for the OpenID Connect provider.
    pub fn with_discovery_url(mut self, discovery_url: String) -> Self {
        self.openid_discovery_url = Some(discovery_url);

        self
    }

    /// Builds an OAuthClient and an [`IdentityProvider`] based on the OpenID provider's discovery
    /// document.
    ///
    /// # Errors
    ///
    /// Returns an [`OAuthClientBuildError`] if there are required fields missing, if any URL's
    /// provided are invalid or it is unable to load the discovery document.
    pub fn build(self) -> Result<(OAuthClient, Box<dyn IdentityProvider>), OAuthClientBuildError> {
        let discovery_url = self.openid_discovery_url.ok_or_else(|| {
            InvalidStateError::with_message(
                "An OpenID discovery URL  is required to successfully build an OAuthClient".into(),
            )
        })?;

        // make a call to the discovery document
        let response = Client::new().get(&discovery_url).send().map_err(|err| {
            InternalError::from_source_with_message(
                Box::new(err),
                "Unable to retrieve OpenID discovery document".into(),
            )
        })?;
        // deserialize response
        let discovery_document_response =
            response
                .json::<DiscoveryDocumentResponse>()
                .map_err(|err| {
                    InternalError::from_source_with_message(
                        Box::new(err),
                        "Unable to deserialize OpenID discovery document".into(),
                    )
                })?;

        let userinfo_endpoint = discovery_document_response.userinfo_endpoint;

        self.inner
            .with_auth_url(discovery_document_response.authorization_endpoint)
            .with_token_url(discovery_document_response.token_endpoint)
            .with_scopes(discovery_document_response.scopes_supported)
            .with_identity_provider(Box::new(OpenIdUserIdentityProvider::new(userinfo_endpoint)))
            .build()
    }
}

impl Default for OpenIdOAuthClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Deserializes the OpenId discovery document response
#[derive(Debug, Deserialize)]
struct DiscoveryDocumentResponse {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    scopes_supported: Vec<String>,
}
