// Copyright 2018-2022 Cargill Incorporated
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
use crate::oauth::OpenIdProfileProvider;
use crate::oauth::{
    builder::OAuthClientBuilder, error::OAuthClientBuildError, store::InflightOAuthRequestStore,
    OAuthClient, OpenIdSubjectProvider,
};

/// The scope required to get a refresh token from an Azure provider.
const AZURE_SCOPE: &str = "offline_access";
/// The scopes required to get OpenID user information.
const DEFAULT_SCOPES: &[&str] = &["openid", "profile", "email"];
/// The authorization request parameters required to get a refresh token from a Google provider.
const GOOGLE_AUTH_PARAMS: &[(&str, &str)] = &[("access_type", "offline"), ("prompt", "consent")];
/// The URL fo the Google OpenID discovery document
const GOOGLE_DISCOVERY_URL: &str = "https://accounts.google.com/.well-known/openid-configuration";

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

    /// Constructs a new [`OpenIdOAuthClientBuilder`] that's pre-configured with the scope for
    /// getting refresh tokens.
    pub fn new_azure() -> Self {
        Self {
            openid_discovery_url: None,
            inner: OAuthClientBuilder::default().with_scopes(vec![AZURE_SCOPE.into()]),
        }
    }

    /// Constructs a new [`OpenIdOAuthClientBuilder`] that's pre-configured with Google's discovery
    /// URL and the extra authorization code request parameter for getting refresh tokens.
    pub fn new_google() -> Self {
        Self {
            openid_discovery_url: Some(GOOGLE_DISCOVERY_URL.into()),
            inner: OAuthClientBuilder::default().with_extra_auth_params(
                GOOGLE_AUTH_PARAMS
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            ),
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

    /// Sets extra parameters that will be added to an authorization request.
    pub fn with_extra_auth_params(self, extra_auth_params: Vec<(String, String)>) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self.inner.with_extra_auth_params(extra_auth_params),
        }
    }

    /// Sets the scopes to request from the OAuth2 provider.
    pub fn with_scopes(self, scopes: Vec<String>) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self.inner.with_scopes(scopes),
        }
    }

    /// Sets the in-flight request store in order to store values between requests to and from the
    /// OAuth2 provider.
    pub fn with_inflight_request_store(
        self,
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    ) -> Self {
        Self {
            openid_discovery_url: self.openid_discovery_url,
            inner: self
                .inner
                .with_inflight_request_store(inflight_request_store),
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

    /// Builds an OAuthClient based on the OpenID provider's discovery document.
    ///
    /// # Errors
    ///
    /// Returns an [`OAuthClientBuildError`] if there are required fields missing, if any URL's
    /// provided are invalid or it is unable to load the discovery document.
    pub fn build(self) -> Result<OAuthClient, OAuthClientBuildError> {
        let discovery_url = self.openid_discovery_url.ok_or_else(|| {
            InvalidStateError::with_message(
                "An OpenID discovery URL is required to successfully build an OAuthClient".into(),
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

        let inner = self
            .inner
            .with_auth_url(discovery_document_response.authorization_endpoint)
            .with_token_url(discovery_document_response.token_endpoint)
            .with_scopes(DEFAULT_SCOPES.iter().map(ToString::to_string).collect())
            .with_subject_provider(Box::new(OpenIdSubjectProvider::new(
                userinfo_endpoint.clone(),
            )))
            .with_profile_provider(Box::new(OpenIdProfileProvider::new(userinfo_endpoint)));

        inner.build()
    }
}

impl Default for OpenIdOAuthClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Deserializes the OpenID discovery document response
#[derive(Debug, Deserialize)]
struct DiscoveryDocumentResponse {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
}

/// These tests require actix to be enabled
#[cfg(test)]
#[cfg(all(feature = "actix", feature = "actix-web", feature = "futures"))]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
    use futures::Future;
    use url::Url;

    use crate::oauth::store::MemoryInflightOAuthRequestStore;

    const CLIENT_ID: &str = "client_id";
    const CLIENT_SECRET: &str = "client_secret";
    const EXTRA_AUTH_PARAM_KEY: &str = "key";
    const EXTRA_AUTH_PARAM_VAL: &str = "val";
    const EXTRA_SCOPE: &str = "scope";
    const REDIRECT_URL: &str = "http://oauth/callback";
    const DISCOVERY_DOCUMENT_ENDPOINT: &str = "/.well-known/openid-configuration";
    const AUTHORIZATION_ENDPOINT: &str = "http://oauth/auth";
    const TOKEN_ENDPOINT: &str = "http://oauth/token";
    const USERINFO_ENDPOINT: &str = "http://oauth/userinfo";

    /// Verifies that the `OpenIdOAuthClientBuilder` builds an OAuth client with the correct values.
    ///
    /// 1. Start the mock OpenID server
    /// 2. Create a builder and set all of its values
    /// 3. Verify the values are correctly set for the builder
    /// 4. Build the client
    /// 5. Verify that the resulting client has the correct values (for the ones we can check)
    /// 6. Shutdown the OpenID server
    #[test]
    fn basic_client() {
        let (shutdown_handle, address) = run_mock_openid_server("basic_client");

        let extra_auth_params = vec![(
            EXTRA_AUTH_PARAM_KEY.to_string(),
            EXTRA_AUTH_PARAM_VAL.to_string(),
        )];
        let extra_scopes = vec![EXTRA_SCOPE.to_string()];
        let discovery_url = format!("{}{}", address, DISCOVERY_DOCUMENT_ENDPOINT);

        let builder = OpenIdOAuthClientBuilder::new()
            .with_client_id(CLIENT_ID.into())
            .with_client_secret(CLIENT_SECRET.into())
            .with_extra_auth_params(extra_auth_params.clone())
            .with_scopes(vec![EXTRA_SCOPE.into()])
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .with_redirect_url(REDIRECT_URL.into())
            .with_discovery_url(discovery_url.clone());

        assert_eq!(builder.inner.client_id, Some(CLIENT_ID.into()));
        assert_eq!(builder.inner.client_secret, Some(CLIENT_SECRET.into()));
        assert_eq!(builder.inner.extra_auth_params, extra_auth_params);
        assert_eq!(builder.inner.scopes, extra_scopes);
        assert!(builder.inner.inflight_request_store.is_some());
        assert_eq!(builder.inner.redirect_url, Some(REDIRECT_URL.into()));
        assert_eq!(builder.openid_discovery_url, Some(discovery_url));
        assert!(builder.inner.auth_url.is_none());
        assert!(builder.inner.token_url.is_none());
        assert!(builder.inner.subject_provider.is_none());
        assert!(builder.inner.profile_provider.is_none());

        let client = builder
            .build()
            .expect("Failed to build OpenID OAuth client");

        assert_eq!(client.extra_auth_params, extra_auth_params);
        assert_eq!(
            client
                .scopes
                .iter()
                .map(|scope| scope.as_str())
                .collect::<HashSet<_>>(),
            DEFAULT_SCOPES
                .iter()
                .cloned()
                .chain(std::iter::once(EXTRA_SCOPE.into()))
                .collect::<HashSet<_>>(),
        );

        let expected_auth_url =
            Url::parse(AUTHORIZATION_ENDPOINT).expect("Failed to parse expected auth URL");
        let generated_auth_url = Url::parse(
            &client
                .get_authorization_url("client_redirect_url".into())
                .expect("Failed to generate auth URL"),
        )
        .expect("Failed to parse generated auth URL");
        assert_eq!(expected_auth_url.origin(), generated_auth_url.origin());

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OpenIdOAuthClientBuilder` builds an Azure client with the correct scopes.
    ///
    /// 1. Start the mock OpenID server
    /// 2. Use the builder to create an Azure client
    /// 3. Verify the scopes are correct for the resulting client
    /// 4. Shutdown the OpenID server
    #[test]
    fn azure_client() {
        let (shutdown_handle, address) = run_mock_openid_server("azure_client");

        let discovery_url = format!("{}{}", address, DISCOVERY_DOCUMENT_ENDPOINT);

        let client = OpenIdOAuthClientBuilder::new_azure()
            .with_client_id(CLIENT_ID.into())
            .with_client_secret(CLIENT_SECRET.into())
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .with_redirect_url(REDIRECT_URL.into())
            .with_discovery_url(discovery_url.clone())
            .build()
            .expect("Failed to build Azure client");

        assert_eq!(
            client
                .scopes
                .iter()
                .map(|scope| scope.as_str())
                .collect::<HashSet<_>>(),
            DEFAULT_SCOPES
                .iter()
                .cloned()
                .chain(std::iter::once(AZURE_SCOPE.into()))
                .collect::<HashSet<_>>(),
        );

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OpenIdOAuthClientBuilder` builds a Google client with the correct
    /// discovery URL and auth params.
    ///
    /// 1. Create a Google OpenID client builder
    /// 2. Verify that the discovery document URL is correct
    /// 3. Create the Google client
    /// 4. Verify the auth parameters are correct for the resulting client
    #[test]
    fn google_client() {
        let builder = OpenIdOAuthClientBuilder::new_google()
            .with_client_id(CLIENT_ID.into())
            .with_client_secret(CLIENT_SECRET.into())
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .with_redirect_url(REDIRECT_URL.into());

        assert_eq!(
            builder.openid_discovery_url.as_deref(),
            Some(GOOGLE_DISCOVERY_URL)
        );

        let client = builder.build().expect("Failed to build Google client");

        assert_eq!(
            client
                .extra_auth_params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect::<HashSet<_>>(),
            GOOGLE_AUTH_PARAMS.iter().cloned().collect::<HashSet<_>>(),
        );
    }

    /// Runs a mock OAuth OpenID server and returns its shutdown handle along with the address the
    /// server is running on.
    fn run_mock_openid_server(test_name: &str) -> (OpenIDServerShutdownHandle, String) {
        let (tx, rx) = channel();

        let instance_name = format!("OpenID-Server-{}", test_name);
        let join_handle = std::thread::Builder::new()
            .name(instance_name.clone())
            .spawn(move || {
                let sys = System::new(instance_name);
                let server = HttpServer::new(|| {
                    App::new().service(
                        web::resource(DISCOVERY_DOCUMENT_ENDPOINT).to(discovery_document_endpoint),
                    )
                })
                .bind("127.0.0.1:0")
                .expect("Failed to bind OpenID server");
                let address = format!("http://127.0.0.1:{}", server.addrs()[0].port());
                let server = server.disable_signals().system_exit().start();
                tx.send((server, address)).expect("Failed to send server");
                sys.run().expect("OpenID server runtime failed");
            })
            .expect("Failed to spawn OpenID server thread");

        let (server, address) = rx.recv().expect("Failed to receive server");

        (OpenIDServerShutdownHandle(server, join_handle), address)
    }

    /// The handler for the OpenID server's discovery document endpoint. The discovery document only
    /// contains the feilds that are used by the `OpenIdOAuthClientBuilder`.
    fn discovery_document_endpoint() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "authorization_endpoint": AUTHORIZATION_ENDPOINT,
                "token_endpoint": TOKEN_ENDPOINT,
                "userinfo_endpoint": USERINFO_ENDPOINT,
            }))
    }

    struct OpenIDServerShutdownHandle(Server, JoinHandle<()>);

    impl OpenIDServerShutdownHandle {
        pub fn shutdown(self) {
            self.0
                .stop(false)
                .wait()
                .expect("Failed to stop OpenID server");
            self.1.join().expect("OpenID server thread failed");
        }
    }
}
