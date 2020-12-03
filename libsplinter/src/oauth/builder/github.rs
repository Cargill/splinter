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

use crate::oauth::{builder::OAuthClientBuilder, error::OAuthClientBuildError, OAuthClient};
use crate::rest_api::auth::identity::github::GithubUserIdentityProvider;

/// Builds a new `OAuthClient` with GitHub's authorization and token URLs.
pub struct GithubOAuthClientBuilder {
    inner: OAuthClientBuilder,
}

impl GithubOAuthClientBuilder {
    /// Constructs a Github OAuthClient builder.
    pub fn new() -> Self {
        Self {
            inner: OAuthClientBuilder::new()
                .with_auth_url("https://github.com/login/oauth/authorize".into())
                .with_token_url("https://github.com/login/oauth/access_token".into())
                .with_identity_provider(Box::new(GithubUserIdentityProvider)),
        }
    }

    /// Sets the client ID for the OAuth2 provider.
    pub fn with_client_id(self, client_id: String) -> Self {
        Self {
            inner: self.inner.with_client_id(client_id),
        }
    }

    /// Sets the client secret for the OAuth2 provider.
    pub fn with_client_secret(self, client_secret: String) -> Self {
        Self {
            inner: self.inner.with_client_secret(client_secret),
        }
    }

    /// Sets the redirect URL for the OAuth2 provider.
    pub fn with_redirect_url(self, redirect_url: String) -> Self {
        Self {
            inner: self.inner.with_redirect_url(redirect_url),
        }
    }

    /// Builds an OAuthClient.
    ///
    /// # Errors
    ///
    /// Returns an [`OAuthClientBuildError`] if there are required fields missing, or any URL's
    /// provided are invalid.
    pub fn build(self) -> Result<OAuthClient, OAuthClientBuildError> {
        self.inner.build()
    }
}

impl Default for GithubOAuthClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
