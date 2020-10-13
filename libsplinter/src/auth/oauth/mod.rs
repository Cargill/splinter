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

mod error;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use oauth2::{
    basic::{BasicClient, BasicTokenResponse},
    reqwest::http_client,
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, Scope, TokenResponse, TokenUrl,
};

pub use error::{OAuthClientConfigurationError, OAuthClientError};

/// An OAuth2 client for Splinter
#[derive(Clone)]
pub struct OAuthClient {
    client: BasicClient,
    /// List of (CSRF token, PKCE verifier) pairs for pending authorization requests
    pending_authorizations: Arc<Mutex<HashMap<String, String>>>,
    scopes: Vec<String>,
}

impl OAuthClient {
    pub fn new(
        client_id: String,
        client_secret: String,
        auth_url: String,
        token_url: String,
        scopes: Vec<String>,
    ) -> Result<Self, OAuthClientConfigurationError> {
        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(auth_url)
                .map_err(|err| OAuthClientConfigurationError::InvalidAuthUrl(err.to_string()))?,
            Some(
                TokenUrl::new(token_url).map_err(|err| {
                    OAuthClientConfigurationError::InvalidTokenUrl(err.to_string())
                })?,
            ),
        );
        Ok(Self {
            client,
            pending_authorizations: Default::default(),
            scopes,
        })
    }

    /// Generates the URL that the end user should be redirected to for authorization
    pub fn get_authorization_url(&self) -> Result<String, OAuthClientError> {
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut request = self
            .client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);
        for scope in &self.scopes {
            request = request.add_scope(Scope::new(scope.into()));
        }
        let (authorize_url, csrf_state) = request.url();

        self.pending_authorizations
            .lock()
            .map_err(|_| OAuthClientError::new("pending authorizations lock was poisoned"))?
            .insert(csrf_state.secret().into(), pkce_verifier.secret().into());

        Ok(authorize_url.to_string())
    }

    /// Exchanges the given authorization code for an access token
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
    ) -> Result<Option<UserTokens>, OAuthClientError> {
        let pkce_verifier = match self
            .pending_authorizations
            .lock()
            .map_err(|_| OAuthClientError::new("pending authorizations lock was poisoned"))?
            .remove(csrf_token)
        {
            Some(pkce_verifier) => PkceCodeVerifier::new(pkce_verifier),
            None => return Ok(None),
        };

        let token_response = self
            .client
            .exchange_code(AuthorizationCode::new(auth_code))
            .set_pkce_verifier(pkce_verifier)
            .request(http_client)
            .map_err(|err| {
                OAuthClientError::new(&format!(
                    "failed to make authorization code exchange request: {}",
                    err,
                ))
            })?;

        Ok(Some(UserTokens::from(token_response)))
    }
}

/// User information returned by the OAuth2 client
pub struct UserTokens {
    /// The access token to be used for authentication in future requests
    access_token: String,
    /// The amount of time (if the provider gives it) until the access token expires and the refresh
    /// token will need to be used
    expires_in: Option<Duration>,
    /// The refresh token (if the provider gives one) for refreshing the access token
    refresh_token: Option<String>,
}

impl UserTokens {
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
}

impl std::fmt::Debug for UserTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("UserTokens")
            .field("access_token", &"<Redacted>".to_string())
            .field("expires_in", &self.expires_in)
            .field(
                "refresh_token",
                &self.refresh_token.as_deref().map(|_| "<Redacted>"),
            )
            .finish()
    }
}

impl From<BasicTokenResponse> for UserTokens {
    fn from(token_response: BasicTokenResponse) -> Self {
        Self {
            access_token: token_response.access_token().secret().into(),
            expires_in: token_response.expires_in(),
            refresh_token: token_response
                .refresh_token()
                .map(|token| token.secret().into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that the `OAuthClient::new` is successful when valid URLs are provided but returns
    /// appropriate errors when invalid URLs are provided.
    #[test]
    fn client_construction() {
        OAuthClient::new(
            "client_id".into(),
            "client_secret".into(),
            "https://provider.com/auth".into(),
            "https://provider.com/token".into(),
            vec![],
        )
        .expect("Failed to create client from valid inputs");

        assert!(matches!(
            OAuthClient::new(
                "client_id".into(),
                "client_secret".into(),
                "invalid_auth_url".into(),
                "https://provider.com/token".into(),
                vec![],
            ),
            Err(OAuthClientConfigurationError::InvalidAuthUrl(_))
        ));

        assert!(matches!(
            OAuthClient::new(
                "client_id".into(),
                "client_secret".into(),
                "https://provider.com/auth".into(),
                "invalid_token_url".into(),
                vec![],
            ),
            Err(OAuthClientConfigurationError::InvalidTokenUrl(_))
        ));
    }
}
