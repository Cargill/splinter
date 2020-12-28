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

use std::time::Duration;

use crate::biome::OAuthUserSessionStore;
use crate::error::InternalError;
use crate::oauth::SubjectProvider;
use crate::rest_api::auth::{AuthorizationHeader, BearerToken};

use super::IdentityProvider;

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
    subject_provider: Box<dyn SubjectProvider>,
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
    reauthentication_interval: Duration,
}

impl OAuthUserIdentityProvider {
    /// Creates a new OAuth user identity provider
    ///
    /// # Arguments
    ///
    /// * `subject_provider` - The OAuth subject provider that calls the OAuth server to check if a
    ///   session is still valid
    /// * `oauth_user_session_store` - The store that tracks users' sessions
    /// * `reauthentication_interval` - The amount of time since the last authentication for which
    ///   the identity provider can assume the session is still valid. If this amount of time has
    ///   elapsed since the last authentication of a session, the session will be re-authenticated
    ///   by the identity provider. If not provided, the default will be used (1 hour).
    pub fn new(
        subject_provider: Box<dyn SubjectProvider>,
        oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
        reauthentication_interval: Option<Duration>,
    ) -> Self {
        Self {
            subject_provider,
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
    ) -> Result<Option<String>, InternalError> {
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
            match self
                .subject_provider
                .get_subject(session.oauth_access_token())
            {
                Ok(Some(_)) => {
                    let updated_session = session.into_update_builder().build();
                    self.oauth_user_session_store
                        .update_session(updated_session)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Ok(Some(user_id))
                }
                Ok(None) => {
                    self.oauth_user_session_store
                        .remove_session(token)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Ok(None)
                }
                Err(err) => {
                    self.oauth_user_session_store
                        .remove_session(token)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Err(err)
                }
            }
        } else {
            Ok(Some(user_id))
        }
    }

    fn clone_box(&self) -> Box<dyn IdentityProvider> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use crate::biome::MemoryOAuthUserSessionStore;

    /// Verifies that the `OAuthUserIdentityProvider` returns a cached user identity when a session
    /// does not need to be re-authenticated.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, a subject provider
    ///    that always fails (this will verify that the cache is used and this isn't called), and
    ///    the default re-authentication interval (an hour is long enough to ensure that the session
    ///    does not expire while this test is running).
    /// 4. Call the `get_identity` method with the session's access token and verify that the
    ///    correct identity (the user's Biome ID) is returned.
    #[test]
    fn get_identity_cached() {
        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");
        let user_id = session_store
            .get_session(splinter_access_token)
            .expect("Failed to get inserted session")
            .expect("Inserted session not found")
            .user()
            .user_id()
            .to_string();

        let identity_provider =
            OAuthUserIdentityProvider::new(Box::new(AlwaysErrSubjectProvider), session_store, None);

        let authorization_header =
            AuthorizationHeader::Bearer(BearerToken::OAuth2(splinter_access_token.into()));
        let identity = identity_provider
            .get_identity(&authorization_header)
            .expect("Failed to get identity")
            .expect("Identity not found");
        assert_eq!(identity, user_id);
    }

    /// Verifies that the `OAuthUserIdentityProvider` returns `None` when the sessions store does
    /// not have a session for the given token.
    ///
    /// 1. Create a new `OAuthUserIdentityProvider` with an empty session store and a subject
    ///    provider that always succeeds (this will verify that the subject provider isn't called
    ///    when the session doesn't even exist).
    /// 2. Call the `get_identity` method and verify that `None` is returned
    #[test]
    fn get_identity_no_session() {
        let identity_provider = OAuthUserIdentityProvider::new(
            Box::new(AlwaysSomeSubjectProvider),
            Box::new(MemoryOAuthUserSessionStore::new()),
            None,
        );

        let authorization_header =
            AuthorizationHeader::Bearer(BearerToken::OAuth2("splinter_access_token".into()));
        assert!(identity_provider
            .get_identity(&authorization_header)
            .expect("Failed to get identity")
            .is_none());
    }

    /// Verifies that the `OAuthUserIdentityProvider` re-authenticates a session when the
    /// re-authentication interval has expired for a session.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, a subject provider
    ///    that always succeeds, and a re-authentication interval of 0 (the session will expire
    ///    immediately).
    /// 4. Call the `get_identity` method with the session's access token and verify that the
    ///    identity is correct.
    /// 5. Verify that the "last authenticated" time for the session in the session store is more
    ///    recent than the session's original "last authenticated" time.
    #[test]
    fn get_identity_reauthentication_successful() {
        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");
        let original_session = session_store
            .get_session(splinter_access_token)
            .expect("Failed to get inserted session")
            .expect("Inserted session not found");

        let identity_provider = OAuthUserIdentityProvider::new(
            Box::new(AlwaysSomeSubjectProvider),
            session_store.clone(),
            Some(Duration::from_secs(0)),
        );

        let authorization_header =
            AuthorizationHeader::Bearer(BearerToken::OAuth2(splinter_access_token.into()));
        let identity = identity_provider
            .get_identity(&authorization_header)
            .expect("Failed to get identity")
            .expect("Identity not found");
        assert_eq!(&identity, original_session.user().user_id());

        let new_session = session_store
            .get_session(splinter_access_token)
            .expect("Failed to get updated session")
            .expect("Updated session not found");
        assert!(new_session.last_authenticated() > original_session.last_authenticated());
    }

    /// Verifies that the `OAuthUserIdentityProvider` correctly handles the case where the internal
    /// subect provider returns `Ok(None)` when re-authenticating a session.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, a subject provider
    ///    that always returns `Ok(None)`, and a re-authentication interval of 0 (the session will
    ///    expire immediately).
    /// 4. Call the `get_identity` method with the session's access token and verify that `Ok(None)`
    ///    is returned.
    /// 5. Verify that the session has been removed from the store.
    #[test]
    fn get_identity_reauthentication_unauthorized() {
        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");

        let identity_provider = OAuthUserIdentityProvider::new(
            Box::new(AlwaysNoneSubjectProvider),
            session_store.clone(),
            Some(Duration::from_secs(0)),
        );

        let authorization_header =
            AuthorizationHeader::Bearer(BearerToken::OAuth2(splinter_access_token.into()));
        assert!(identity_provider
            .get_identity(&authorization_header)
            .expect("Failed to get identity")
            .is_none());

        assert!(session_store
            .get_session(splinter_access_token)
            .expect("Failed to get session")
            .is_none());
    }

    /// Verifies that the `OAuthUserIdentityProvider` correctly handles the case where the internal
    /// subect provider returns an error when re-authenticating a session.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, a subject provider
    ///    that always returns an error, and a re-authentication interval of 0 (the session will
    ///    expire immediately).
    /// 4. Call the `get_identity` method with the session's access token and verify that an error
    ///    is returned.
    /// 5. Verify that the session has been removed from the store.
    #[test]
    fn get_identity_reauthentication_failed() {
        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");

        let identity_provider = OAuthUserIdentityProvider::new(
            Box::new(AlwaysErrSubjectProvider),
            session_store.clone(),
            Some(Duration::from_secs(0)),
        );

        let authorization_header =
            AuthorizationHeader::Bearer(BearerToken::OAuth2(splinter_access_token.into()));
        assert!(identity_provider
            .get_identity(&authorization_header)
            .is_err());

        assert!(session_store
            .get_session(splinter_access_token)
            .expect("Failed to get session")
            .is_none());
    }

    /// Subject provider that always returns a subject
    #[derive(Clone)]
    struct AlwaysSomeSubjectProvider;

    impl SubjectProvider for AlwaysSomeSubjectProvider {
        fn get_subject(&self, _access_token: &str) -> Result<Option<String>, InternalError> {
            Ok(Some("subject".into()))
        }

        fn clone_box(&self) -> Box<dyn SubjectProvider> {
            Box::new(self.clone())
        }
    }

    /// Subject provider that always returns `Ok(None)`
    #[derive(Clone)]
    struct AlwaysNoneSubjectProvider;

    impl SubjectProvider for AlwaysNoneSubjectProvider {
        fn get_subject(&self, _access_token: &str) -> Result<Option<String>, InternalError> {
            Ok(None)
        }

        fn clone_box(&self) -> Box<dyn SubjectProvider> {
            Box::new(self.clone())
        }
    }

    /// Subject provider that always returns `Err`
    #[derive(Clone)]
    struct AlwaysErrSubjectProvider;

    impl SubjectProvider for AlwaysErrSubjectProvider {
        fn get_subject(&self, _access_token: &str) -> Result<Option<String>, InternalError> {
            Err(InternalError::with_message("error".into()))
        }

        fn clone_box(&self) -> Box<dyn SubjectProvider> {
            Box::new(self.clone())
        }
    }
}
