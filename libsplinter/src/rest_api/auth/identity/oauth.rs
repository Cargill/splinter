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

//! An identity provider backed by an OAuth server

use crate::biome::OAuthUserSessionStore;
use crate::error::InternalError;
use crate::oauth::SubjectProvider;
use crate::rest_api::auth::{AuthorizationHeader, BearerToken};

use super::IdentityProvider;

/// An identity provider, backed by an OAuth server, that returns a user's Biome ID
///
/// This provider uses an [OAuthUserSessionStore] as a cache of identities. The session store tracks
/// all OAuth users' sessions with a "last authenticated" timestamp. Sessions are initially added by
/// the OAuth REST API endpoints when a user logs in.
///
/// If the session has not been authenticated within the re-authentication interval, the user will
/// be re-authenticated using the internal OAuth [SubjectProvider] and the session will be updated
/// in the session store. If re-authentication fails, the session will be removed from the store
/// and the user will need to start a new session by logging in.
///
/// This identity provider will also use a session's refresh token (if it has one) to get a new
/// OAuth access token for the session as needed.
///
/// This provider only accepts `AuthorizationHeader::Bearer(BearerToken::OAuth2(token))`
/// authorizations, and the inner token must be a valid Splinter access token for an OAuth user.
#[derive(Clone)]
pub struct OAuthUserIdentityProvider {
    _subject_provider: Box<dyn SubjectProvider>,
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
}

impl OAuthUserIdentityProvider {
    /// Creates a new OAuth user identity provider
    ///
    /// # Arguments
    ///
    /// * `subject_provider` - The OAuth subject provider that calls the OAuth server to check if a
    ///   session is still valid
    /// * `oauth_user_session_store` - The store that tracks users' sessions
    pub fn new(
        _subject_provider: Box<dyn SubjectProvider>,
        oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
    ) -> Self {
        Self {
            _subject_provider: subject_provider,
            oauth_user_session_store,
        }
    }
}

impl IdentityProvider for OAuthUserIdentityProvider {
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<String>, InternalError> {
        let token = match authorization {
            AuthorizationHeader::Bearer(BearerToken::OAuth2(token)) => token,
            _ => return Ok(None),
        };

        Ok(self
            .oauth_user_session_store
            .get_session(token)
            .map_err(|err| InternalError::from_source(err.into()))?
            .map(|session| session.user().user_id().to_string()))
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
