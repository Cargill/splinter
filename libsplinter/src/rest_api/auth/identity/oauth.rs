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

//! A general OAuth identity provider implementation wraps another, OAuth server-backed identity
//! provider

use crate::biome::OAuthUserSessionStore;
use crate::error::InternalError;
use crate::rest_api::auth::{AuthorizationHeader, BearerToken};

use super::IdentityProvider;

/// A general OAuth identity provider implementation wraps another, OAuth server-backed identity
/// provider
///
/// This provider uses an [OAuthUserSessionStore] as a cache of identities. The session store tracks
/// all OAuth users' sessions with a "last authenticated" timestamp. If the user has been
/// authenticated within the last hour, the identity from the session store will be returned; if the
/// user has not been authenticated within the last hour, the user will be re-authenticated using
/// the internal identity provider and the session will be updated in the session store.
///
/// This identity provider will also use a session's refresh token (if it has one) to get a new
/// OAuth access token for the session as needed.
#[derive(Clone)]
pub struct OAuthUserIdentityProvider {
    _internal_identity_provider: Box<dyn IdentityProvider>,
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
}

impl OAuthUserIdentityProvider {
    pub fn new(
        internal_identity_provider: Box<dyn IdentityProvider>,
        oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
    ) -> Self {
        Self {
            _internal_identity_provider: internal_identity_provider,
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
            .map(|session| session.user().subject().to_string()))
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
