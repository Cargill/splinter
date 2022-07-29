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

//! An identity provider backed by an OAuth server

use std::time::Duration;

use log::debug;
use splinter::biome::OAuthUserSessionStore;
use splinter::error::InternalError;
use splinter::oauth::OAuthClient;

use crate::auth::{AuthorizationHeader, BearerToken};

use super::{Identity, IdentityProvider};

/// The default amount of time since the last authentication for which the identity provider can
/// assume the session is still valid
const DEFAULT_REAUTHENTICATION_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour

/// An identity provider, backed by an OAuth server, that returns a user's Biome ID
///
/// This provider uses an [OAuthUserSessionStore] as a cache of identities. The session store tracks
/// all OAuth users' sessions with a "last authenticated" timestamp. Sessions are initially added by
/// the OAuth REST API endpoints when a user logs in.
///
/// If the session has not been authenticated within the re-authentication interval, the user will
/// be re-authenticated using the internal [OAuthClient] and the session will be updated in the
/// session store. If re-authentication fails, the session will be removed from the store and the
/// user will need to start a new session by logging in.
///
/// This identity provider will also use a session's refresh token (if it has one) to get a new
/// OAuth access token for the session as needed.
///
/// This provider only accepts `AuthorizationHeader::Bearer(BearerToken::OAuth2(token))`
/// authorizations, and the inner token must be a valid Splinter access token for an OAuth user.
#[derive(Clone)]
pub struct OAuthUserIdentityProvider {
    oauth_client: OAuthClient,
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
    reauthentication_interval: Duration,
}

impl OAuthUserIdentityProvider {
    /// Creates a new OAuth user identity provider
    ///
    /// # Arguments
    ///
    /// * `oauth_client` - The OAuth client that will be used to check if a session is still valid
    /// * `oauth_user_session_store` - The store that tracks users' sessions
    /// * `reauthentication_interval` - The amount of time since the last authentication for which
    ///   the identity provider can assume the session is still valid. If this amount of time has
    ///   elapsed since the last authentication of a session, the session will be re-authenticated
    ///   by the identity provider. If not provided, the default will be used (1 hour).
    pub fn new(
        oauth_client: OAuthClient,
        oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
        reauthentication_interval: Option<Duration>,
    ) -> Self {
        Self {
            oauth_client,
            oauth_user_session_store,
            reauthentication_interval: reauthentication_interval
                .unwrap_or(DEFAULT_REAUTHENTICATION_INTERVAL),
        }
    }
}

impl IdentityProvider for OAuthUserIdentityProvider {
    fn get_identity(
        &self,
        authorization: &AuthorizationHeader,
    ) -> Result<Option<Identity>, InternalError> {
        let token = match authorization {
            AuthorizationHeader::Bearer(BearerToken::OAuth2(token)) => token,
            _ => return Ok(None),
        };

        let session = match self
            .oauth_user_session_store
            .get_session(token)
            .map_err(|err| InternalError::from_source(err.into()))?
        {
            Some(session) => session,
            None => return Ok(None),
        };

        let user_id = session.user().user_id().to_string();

        let time_since_authenticated = session
            .last_authenticated()
            .elapsed()
            .map_err(|err| InternalError::from_source(err.into()))?;
        if time_since_authenticated >= self.reauthentication_interval {
            match self.oauth_client.get_subject(session.oauth_access_token()) {
                Ok(Some(_)) => {
                    let updated_session = session.into_update_builder().build();
                    self.oauth_user_session_store
                        .update_session(updated_session)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Ok(Some(Identity::User(user_id)))
                }
                Ok(None) => {
                    // The access token didn't work; see if there's a refresh token that can be used
                    // to get a new one.
                    match session.oauth_refresh_token() {
                        Some(refresh_token) => {
                            // Try using the session's OAuth refresh token to get a new OAuth
                            // access token
                            match self
                                .oauth_client
                                .exchange_refresh_token(refresh_token.to_string())
                            {
                                Ok(access_token) => {
                                    // Update the access token in the store
                                    let updated_session = session
                                        .into_update_builder()
                                        .with_oauth_access_token(access_token.clone())
                                        .build();
                                    self.oauth_user_session_store
                                        .update_session(updated_session)
                                        .map_err(|err| InternalError::from_source(err.into()))?;
                                    // Authenticate with the new access token; if this fails (we
                                    // get Ok(None) or Err(_)), something's wrong that can't be
                                    // handled here.
                                    match self.oauth_client.get_subject(&access_token)? {
                                        Some(_) => Ok(Some(Identity::User(user_id))),
                                        None => Err(InternalError::with_message(
                                            "failed to authenticate user with new access token"
                                                .into(),
                                        )),
                                    }
                                }
                                Err(err) => {
                                    // The refresh token didn't work; delete the session since it's
                                    // no longer valid
                                    debug!("Failed to exchange refresh token: {}", err);
                                    self.oauth_user_session_store
                                        .remove_session(token)
                                        .map_err(|err| InternalError::from_source(err.into()))?;
                                    Ok(None)
                                }
                            }
                        }
                        None => {
                            // The access token didn't work and there's no refresh token for this
                            // session; delete the session since it's no longer valid.
                            self.oauth_user_session_store
                                .remove_session(token)
                                .map_err(|err| InternalError::from_source(err.into()))?;
                            Ok(None)
                        }
                    }
                }
                Err(err) => {
                    self.oauth_user_session_store
                        .remove_session(token)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Err(err)
                }
            }
        } else {
            Ok(Some(Identity::User(user_id)))
        }
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}
