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

//! Support for OAuth2 authorization in Splinter

mod builder;
mod error;
#[cfg(feature = "rest-api")]
pub mod rest_api;
pub mod store;
mod subject;

use std::time::Duration;

use oauth2::{
    basic::BasicClient, reqwest::http_client, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};

use crate::error::{InternalError, InvalidArgumentError};

use store::InflightOAuthRequestStore;

#[cfg(feature = "oauth-github")]
pub use builder::GithubOAuthClientBuilder;
pub use builder::OAuthClientBuilder;
#[cfg(feature = "oauth-openid")]
pub use builder::OpenIdOAuthClientBuilder;
pub use error::OAuthClientBuildError;
#[cfg(feature = "oauth-github")]
pub use subject::GithubSubjectProvider;
#[cfg(feature = "oauth-openid")]
pub use subject::OpenIdSubjectProvider;
pub use subject::SubjectProvider;

/// An OAuth2 client for Splinter
///
/// This client currently supports OAuth2 authorization code grants
/// (<https://tools.ietf.org/html/rfc6749#section-4.1>).
#[derive(Clone)]
pub struct OAuthClient {
    /// The inner OAuth2 client
    client: BasicClient,
    /// The scopes that will be requested for each user that's authenticated
    scopes: Vec<String>,
    /// OAuth2 subject provider used to retrieve users' subject identifiers
    subject_provider: Box<dyn SubjectProvider>,

    /// Store for pending authorization requests, including the CSRF token, PKCE verifier, and
    /// client's redirect URL
    inflight_request_store: Box<dyn InflightOAuthRequestStore>,
}

impl OAuthClient {
    /// Creates a new `OAuthClient`
    ///
    /// # Arguments
    ///
    /// * `client` - the [oauth2::basic::BasicClient], used for requests to the provider
    /// * `scopes` - The scopes that will be requested for each user
    /// * `subject_provider` - The OAuth subject provider used to retrieve users' subject
    ///   identifiers
    /// * `inflight_request_store` - The store for information about in-flight request to a
    /// provider.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the auth, redirect, or token URLs are invalid
    fn new(
        client: BasicClient,
        scopes: Vec<String>,
        subject_provider: Box<dyn SubjectProvider>,
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    ) -> Result<Self, InvalidArgumentError> {
        Ok(Self {
            client,
            scopes,
            subject_provider,
            inflight_request_store,
        })
    }

    /// Generates the URL that the end user should be redirected to for authorization
    ///
    /// # Arguments
    ///
    /// * `client_redirect_url` - The endpoint that Splinter will redirect to after it has
    ///   completed authorization and the code exchange
    pub fn get_authorization_url(
        &self,
        client_redirect_url: String,
    ) -> Result<String, InternalError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = self
            .client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);
        for scope in &self.scopes {
            request = request.add_scope(Scope::new(scope.into()));
        }
        let (authorize_url, csrf_state) = request.url();

        self.inflight_request_store
            .insert_request(
                csrf_state.secret().into(),
                PendingAuthorization {
                    pkce_verifier: pkce_verifier.secret().into(),
                    client_redirect_url,
                },
            )
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(authorize_url.to_string())
    }

    /// Exchanges the given authorization code for an access token and the client redirect URL
    /// provided in the original auth request, represented by a `String`.
    ///
    /// # Arguments
    ///
    /// * `auth_code` - The authorization code that was supplied by the OAuth provider
    /// * `csrf_token` - The CSRF token that was provided in the original auth request, which is
    ///   used to prevent CSRF attacks and to correlate the auth code with the original auth
    ///   request.
    pub fn exchange_authorization_code(
        &self,
        auth_code: String,
        csrf_token: &str,
    ) -> Result<Option<(UserInfo, String)>, InternalError> {
        let pending_authorization = match self
            .inflight_request_store
            .remove_request(csrf_token)
            .map_err(|err| InternalError::from_source(Box::new(err)))?
        {
            Some(pending_authorization) => pending_authorization,
            None => return Ok(None),
        };

        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(auth_code))
            .set_pkce_verifier(PkceCodeVerifier::new(pending_authorization.pkce_verifier))
            .request(http_client)
            .map_err(|err| {
                InternalError::with_message(format!(
                    "failed to make authorization code exchange request: {}",
                    err,
                ))
            })?;

        // Fetch the users subject identifier from OAuth provider
        let subject = self
            .get_subject(token_response.access_token().secret())?
            .ok_or_else(|| InternalError::with_message("subject not found".into()))?;

        let user_info = UserInfo {
            access_token: token_response.access_token().secret().into(),
            expires_in: token_response.expires_in(),
            refresh_token: token_response
                .refresh_token()
                .map(|token| token.secret().into()),
            subject,
        };

        Ok(Some((user_info, pending_authorization.client_redirect_url)))
    }

    /// Attempts to get the subject that the given access token is for from the OAuth server. This
    /// method will return `Ok(None)` if the access token could not be resolved to a subject.
    pub fn get_subject(&self, access_token: &str) -> Result<Option<String>, InternalError> {
        self.subject_provider.get_subject(access_token)
    }
}

fn new_basic_client(
    client_id: String,
    client_secret: String,
    auth_url: String,
    redirect_url: String,
    token_url: String,
) -> Result<BasicClient, InvalidArgumentError> {
    Ok(BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new(auth_url)
            .map_err(|err| InvalidArgumentError::new("auth_url".into(), err.to_string()))?,
        Some(
            TokenUrl::new(token_url)
                .map_err(|err| InvalidArgumentError::new("token_url".into(), err.to_string()))?,
        ),
    )
    .set_redirect_url(
        RedirectUrl::new(redirect_url)
            .map_err(|err| InvalidArgumentError::new("redirect_url".into(), err.to_string()))?,
    ))
}

/// Information pertaining to pending authorization requests, including the PKCE verifier, and
/// client's redirect URL
#[derive(Debug, PartialEq)]
pub struct PendingAuthorization {
    pkce_verifier: String,
    client_redirect_url: String,
}

/// User information returned by the OAuth2 client
pub struct UserInfo {
    /// The access token to be used for authentication in future requests
    access_token: String,
    /// The amount of time (if the provider gives it) until the access token expires and the refresh
    /// token will need to be used
    expires_in: Option<Duration>,
    /// The refresh token (if the provider gives one) for refreshing the access token
    refresh_token: Option<String>,
    /// The user's subject identifier
    subject: String,
}

impl UserInfo {
    /// Gets the user's access token
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// Gets the amount of time that the user's access token is valid for. Not all providers expire
    /// access tokens, so this may be `None` for some providers.
    pub fn expires_in(&self) -> Option<Duration> {
        self.expires_in
    }

    /// Gets the user's refresh token. Not all providers use refresh tokens, so this may be `None`
    /// for some providers.
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_deref()
    }

    /// Gets the user's subject identifier.
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl std::fmt::Debug for UserInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("UserInfo")
            .field("access_token", &"<Redacted>".to_string())
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_deref().map(|_| "<Redacted>"),
            )
            .field("subject", &self.subject)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::store::InflightOAuthRequestStoreError;

    /// Verifies that the `OAuthClient::new` is successful when valid URLs are provided but returns
    /// appropriate errors when invalid URLs are provided.
    #[test]
    fn client_construction() {
        let subject_box: Box<dyn SubjectProvider> = Box::new(TestSubjectProvider);
        let inflight_request_store = Box::new(TestInflightOAuthRequestStore);
        OAuthClient::new(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "https://provider.com/auth".into(),
                "https://localhost/oauth/callback".into(),
                "https://provider.com/token".into(),
            )
            .expect("Failed to create basic client"),
            vec![],
            subject_box.clone_box(),
            inflight_request_store.clone_box(),
        )
        .expect("Failed to create client from valid inputs");

        assert!(matches!(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "invalid_auth_url".into(),
                "https://localhost/oauth/callback".into(),
                "https://provider.com/token".into(),
            ),
            Err(err) if &err.argument() == "auth_url"
        ));

        assert!(matches!(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "https://provider.com/auth".into(),
                "invalid_redirect_url".into(),
                "https://provider.com/token".into(),
            ),
            Err(err) if &err.argument() == "redirect_url"
        ));

        assert!(matches!(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "https://provider.com/auth".into(),
                "https://localhost/oauth/callback".into(),
                "invalid_token_url".into(),
            ),
            Err(err) if &err.argument() == "token_url"
        ));
    }

    #[derive(Clone)]
    pub struct TestSubjectProvider;

    impl SubjectProvider for TestSubjectProvider {
        fn get_subject(&self, _: &str) -> Result<Option<String>, InternalError> {
            Ok(Some("".to_string()))
        }

        fn clone_box(&self) -> Box<dyn SubjectProvider> {
            Box::new(self.clone())
        }
    }

    #[derive(Clone)]
    pub struct TestInflightOAuthRequestStore;

    impl InflightOAuthRequestStore for TestInflightOAuthRequestStore {
        fn insert_request(
            &self,
            _request_id: String,
            _authorization: PendingAuthorization,
        ) -> Result<(), InflightOAuthRequestStoreError> {
            Ok(())
        }

        fn remove_request(
            &self,
            _request_id: &str,
        ) -> Result<Option<PendingAuthorization>, InflightOAuthRequestStoreError> {
            Ok(None)
        }

        fn clone_box(&self) -> Box<dyn InflightOAuthRequestStore> {
            Box::new(self.clone())
        }
    }
}
