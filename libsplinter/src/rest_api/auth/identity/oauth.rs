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
use crate::oauth::OAuthClient;
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
            match self.oauth_client.get_subject(session.oauth_access_token()) {
                Ok(Some(_)) => {
                    let updated_session = session.into_update_builder().build();
                    self.oauth_user_session_store
                        .update_session(updated_session)
                        .map_err(|err| InternalError::from_source(err.into()))?;
                    Ok(Some(user_id))
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
                                        Some(_) => Ok(Some(user_id)),
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

    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
    use futures::Future;

    use crate::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use crate::biome::MemoryOAuthUserSessionStore;
    use crate::oauth::{
        store::MemoryInflightOAuthRequestStore, OAuthClientBuilder, SubjectProvider,
    };

    const TOKEN_ENDPOINT: &str = "/token";
    const REFRESH_TOKEN: &str = "refresh_token";
    const NEW_OAUTH_ACCESS_TOKEN: &str = "new_oauth_access_token";

    /// Verifies that the `OAuthUserIdentityProvider` returns a cached user identity when a session
    /// does not need to be re-authenticated.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, an OAuth client that
    ///    always fails to get a subject (this will verify that the cache is used and this isn't
    ///    called), and the default re-authentication interval (an hour is long enough to ensure
    ///    that the session does not expire while this test is running).
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
            OAuthUserIdentityProvider::new(always_err_client(), session_store, None);

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
    /// 1. Create a new `OAuthUserIdentityProvider` with an empty session store and an OAuth client
    ///    that always successfully gets a subject (this will verify that the subject provider isn't
    ///    called when the session doesn't even exist).
    /// 2. Call the `get_identity` method and verify that `None` is returned
    #[test]
    fn get_identity_no_session() {
        let identity_provider = OAuthUserIdentityProvider::new(
            always_some_client(),
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
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, an OAuth client
    ///    that always successfully gets a subject, and a re-authentication interval of 0 (the
    ///    session will expire immediately).
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
            always_some_client(),
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
    /// subect provider returns `Ok(None)` when re-authenticating a session without a refresh token.
    ///
    /// 1. Create a new `OAuthUserSessionStore`
    /// 2. Add a session without a refresh token to the store
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, an OAuth client
    ///    that always returns `Ok(None)` when getting a subject, and a re-authentication interval
    ///    of 0 (the session will expire immediately).
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
            always_none_client(),
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
    /// 3. Create a new `OAuthUserIdentityProvider` with the session store, an OAuth client that
    ///    always fails to get a subject, and a re-authentication interval of 0 (the session will
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
            always_err_client(),
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

    /// Verifies that the `OAuthUserIdentityProvider` correctly handles the case where
    /// re-authentication is required and a session's refresh token must be used to get a new access
    /// token.
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new `OAuthUserSessionStore`
    /// 3. Add a session with a refresh token to the store
    /// 4. Create a new OAuthClient with a subject provider that only returns an identity for the
    ///    refreshed OAuth access token.
    /// 5. Create a new `OAuthUserIdentityProvider` with the session store, the OAuth client, and a
    ///    re-authentication interval of 0 (the session will expire immediately).
    /// 6. Call the `get_identity` method with the session's access token and verify that the
    ///    identity is correct.
    /// 7. Verify that the "last authenticated" time for the session in the session store is more
    ///    recent than the session's original "last authenticated" time.
    /// 8. Verify that the session's OAuth access token has been updated to the correct value.
    /// 9. Stop the mock OAuth server
    #[test]
    fn get_identity_refresh_successful() {
        let (shutdown_handle, address) = run_mock_oauth_server("get_identity_refresh_successful");

        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .with_oauth_refresh_token(Some(REFRESH_TOKEN.into()))
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");
        let original_session = session_store
            .get_session(splinter_access_token)
            .expect("Failed to get inserted session")
            .expect("Inserted session not found");

        let client = OAuthClientBuilder::new()
            .with_client_id("client_id".into())
            .with_client_secret("client_secret".into())
            .with_auth_url("http://test.com/auth".into())
            .with_redirect_url("http://test.com/redirect".into())
            .with_token_url(format!("{}{}", address, TOKEN_ENDPOINT))
            .with_subject_provider(Box::new(RefreshedTokenSubjectProvider))
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .build()
            .expect("Failed to build OAuth client");

        let identity_provider = OAuthUserIdentityProvider::new(
            client,
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

        assert_eq!(new_session.oauth_access_token(), NEW_OAUTH_ACCESS_TOKEN);

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OAuthUserIdentityProvider` correctly handles the case where
    /// re-authentication is required and the OAuth client fails to exchange the session's refresh
    /// token for a new access token.
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new `OAuthUserSessionStore`
    /// 3. Add a session with an unknown refresh token to the store (the mock OAuth server checks
    ///    the refresh token, so an unknown token will cause the refresh to fail)
    /// 4. Create a new OAuthClient with a subject provider that only returns an identity for the
    ///    refreshed OAuth access token.
    /// 5. Create a new `OAuthUserIdentityProvider` with the session store, the OAuth client, and a
    ///    re-authentication interval of 0 (the session will expire immediately).
    /// 6. Call the `get_identity` method with the session's access token and verify that `Ok(None)`
    ///    is returned.
    /// 7. Verify that the session has been removed from the store.
    /// 8. Stop the mock OAuth server
    #[test]
    fn get_identity_refresh_failed() {
        let (shutdown_handle, address) = run_mock_oauth_server("get_identity_refresh_successful");

        let session_store = Box::new(MemoryOAuthUserSessionStore::new());

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .with_oauth_refresh_token(Some("unknown_refresh_token".into()))
            .build()
            .expect("Failed to build session");
        session_store
            .add_session(session)
            .expect("Failed to add session");

        let client = OAuthClientBuilder::new()
            .with_client_id("client_id".into())
            .with_client_secret("client_secret".into())
            .with_auth_url("http://test.com/auth".into())
            .with_redirect_url("http://test.com/redirect".into())
            .with_token_url(format!("{}{}", address, TOKEN_ENDPOINT))
            .with_subject_provider(Box::new(RefreshedTokenSubjectProvider))
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .build()
            .expect("Failed to build OAuth client");

        let identity_provider = OAuthUserIdentityProvider::new(
            client,
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

        shutdown_handle.shutdown();
    }

    /// Returns a mock OAuth client that wraps an `AlwaysSomeSubjectProvider`
    fn always_some_client() -> OAuthClient {
        OAuthClientBuilder::new()
            .with_client_id("client_id".into())
            .with_client_secret("client_secret".into())
            .with_auth_url("http://test.com/auth".into())
            .with_redirect_url("http://test.com/redirect".into())
            .with_token_url("http://test.com/token".into())
            .with_subject_provider(Box::new(AlwaysSomeSubjectProvider))
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .build()
            .expect("Failed to build OAuth client")
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

    /// Returns a mock OAuth client that wraps an `AlwaysNoneSubjectProvider`
    fn always_none_client() -> OAuthClient {
        OAuthClientBuilder::new()
            .with_client_id("client_id".into())
            .with_client_secret("client_secret".into())
            .with_auth_url("http://test.com/auth".into())
            .with_redirect_url("http://test.com/redirect".into())
            .with_token_url("http://test.com/token".into())
            .with_subject_provider(Box::new(AlwaysNoneSubjectProvider))
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .build()
            .expect("Failed to build OAuth client")
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

    /// Returns a mock OAuth client that wraps an `AlwaysErrSubjectProvider`
    fn always_err_client() -> OAuthClient {
        OAuthClientBuilder::new()
            .with_client_id("client_id".into())
            .with_client_secret("client_secret".into())
            .with_auth_url("http://test.com/auth".into())
            .with_redirect_url("http://test.com/redirect".into())
            .with_token_url("http://test.com/token".into())
            .with_subject_provider(Box::new(AlwaysErrSubjectProvider))
            .with_inflight_request_store(Box::new(MemoryInflightOAuthRequestStore::new()))
            .build()
            .expect("Failed to build OAuth client")
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

    /// Subject provider that returns a subject when the new `NEW_OAUTH_ACCESS_TOKEN` is provided;
    /// returns `Ok(None)` otherwise.
    #[derive(Clone)]
    struct RefreshedTokenSubjectProvider;

    impl SubjectProvider for RefreshedTokenSubjectProvider {
        fn get_subject(&self, access_token: &str) -> Result<Option<String>, InternalError> {
            if access_token == NEW_OAUTH_ACCESS_TOKEN {
                Ok(Some("subject".into()))
            } else {
                Ok(None)
            }
        }

        fn clone_box(&self) -> Box<dyn SubjectProvider> {
            Box::new(self.clone())
        }
    }

    /// Runs a mock OAuth server and returns its shutdown handle along with the address the server
    /// is running on.
    fn run_mock_oauth_server(test_name: &str) -> (OAuthServerShutdownHandle, String) {
        let (tx, rx) = channel();

        let instance_name = format!("OAuth-Server-{}", test_name);
        let join_handle = std::thread::Builder::new()
            .name(instance_name.clone())
            .spawn(move || {
                let sys = System::new(instance_name);
                let server = HttpServer::new(|| {
                    App::new().service(web::resource(TOKEN_ENDPOINT).to(token_endpoint))
                })
                .bind("127.0.0.1:0")
                .expect("Failed to bind OAuth server");
                let address = format!("http://127.0.0.1:{}", server.addrs()[0].port());
                let server = server.disable_signals().system_exit().start();
                tx.send((server, address)).expect("Failed to send server");
                sys.run().expect("OAuth server runtime failed");
            })
            .expect("Failed to spawn OAuth server thread");

        let (server, address) = rx.recv().expect("Failed to receive server");

        (OAuthServerShutdownHandle(server, join_handle), address)
    }

    /// The handler for the OAuth server's token endpoint. This endpoint receives the request
    /// parameters as a form, since that's how the OAuth2 crate sends the request.
    fn token_endpoint(form: web::Form<TokenRequestForm>) -> HttpResponse {
        assert_eq!(&form.grant_type, "refresh_token");
        if &form.refresh_token == REFRESH_TOKEN {
            HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "token_type": "bearer",
                    "access_token": NEW_OAUTH_ACCESS_TOKEN,
                }))
        } else {
            HttpResponse::Unauthorized().finish()
        }
    }

    #[derive(Deserialize)]
    struct TokenRequestForm {
        grant_type: String,
        refresh_token: String,
    }

    struct OAuthServerShutdownHandle(Server, JoinHandle<()>);

    impl OAuthServerShutdownHandle {
        pub fn shutdown(self) {
            self.0
                .stop(false)
                .wait()
                .expect("Failed to stop OAuth server");
            self.1.join().expect("OAuth server thread failed");
        }
    }
}
