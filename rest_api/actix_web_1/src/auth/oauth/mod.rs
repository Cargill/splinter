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

pub(super) mod callback;
pub(super) mod list_users;
pub(super) mod login;
pub(super) mod logout;
pub(super) mod resource_provider;

#[cfg(test)]
mod tests {

    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;
    use std::time::Duration;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpRequest, HttpResponse, HttpServer};
    use futures::Future;
    use splinter::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use splinter::biome::MemoryOAuthUserSessionStore;
    use splinter::biome::OAuthUserSessionStore;
    use splinter::error::InternalError;
    use splinter::oauth::OAuthClient;
    use splinter::oauth::{
        store::MemoryInflightOAuthRequestStore, OAuthClientBuilder, SubjectProvider,
    };
    use splinter::oauth::{OpenIdProfileProvider, Profile, ProfileProvider};
    use splinter_rest_api_common::auth::{
        AuthorizationHeader, BearerToken, Identity, IdentityProvider, OAuthUserIdentityProvider,
    };

    const USERINFO_ENDPOINT: &str = "/userinfo";
    const ALL_DETAILS_TOKEN: &str = "all_details";
    const ONLY_SUB_TOKEN: &str = "only_sub";
    const UNEXPECTED_RESPONSE_CODE_TOKEN: &str = "unexpected_response_code";
    const INVALID_RESPONSE_TOKEN: &str = "invalid_response";
    const SUB: &str = "sub";
    const NAME: &str = "name";
    const GIVEN_NAME: &str = "given_name";
    const FAMILY_NAME: &str = "family_name";
    const EMAIL: &str = "email";
    const PICTURE: &str = "picture";
    const TOKEN_ENDPOINT: &str = "/token";
    const REFRESH_TOKEN: &str = "refresh_token";
    const NEW_OAUTH_ACCESS_TOKEN: &str = "new_oauth_access_token";

    /// Verifies that the OpenID profile provider correctly returns all relevant profile information
    /// when it's provided.
    ///
    /// 1. Start the mock OpenID server
    /// 2. Get the profile for a user with all details filled out
    /// 3. Verify that all profile details are correct
    /// 4. Shutdown the OpenID server
    #[test]
    fn all_details() {
        let (shutdown_handle, address) = run_mock_openid_server("all_details");

        let profile = OpenIdProfileProvider::new(format!("{}{}", address, USERINFO_ENDPOINT))
            .get_profile(ALL_DETAILS_TOKEN)
            .expect("Failed to get profile")
            .expect("Profile not found");

        assert_eq!(&profile.subject, SUB);
        assert_eq!(profile.name.as_deref(), Some(NAME));
        assert_eq!(profile.given_name.as_deref(), Some(GIVEN_NAME));
        assert_eq!(profile.family_name.as_deref(), Some(FAMILY_NAME));
        assert_eq!(profile.email.as_deref(), Some(EMAIL));
        assert_eq!(profile.picture.as_deref(), Some(PICTURE));

        shutdown_handle.shutdown();
    }

    /// Verifies that the OpenID profile provider correctly returns the profile when only the
    /// subject is provided
    ///
    /// 1. Start the mock OpenID server
    /// 2. Get the profile for a user with only the subject filled out
    /// 3. Verify that the `subject` field is correct and all other fields are empty
    /// 4. Shutdown the OpenID server
    #[test]
    fn only_sub() {
        let (shutdown_handle, address) = run_mock_openid_server("only_sub");

        let profile = OpenIdProfileProvider::new(format!("{}{}", address, USERINFO_ENDPOINT))
            .get_profile(ONLY_SUB_TOKEN)
            .expect("Failed to get profile")
            .expect("Profile not found");

        assert_eq!(&profile.subject, SUB);
        assert!(profile.name.is_none());
        assert!(profile.given_name.is_none());
        assert!(profile.family_name.is_none());
        assert!(profile.email.is_none());
        assert!(profile.picture.is_none());

        shutdown_handle.shutdown();
    }

    /// Verifies that the OpenID profile provider correctly returns `Ok(None)` when receiving a
    /// `401 Unauthorized` response from the OpenID server (which means the token is unknown).
    ///
    /// 1. Start the mock OpenID server
    /// 2. Attempt to get the profile for an unknown token
    /// 3. Verify that the profile provider returns the correct value
    /// 4. Shutdown the OpenID server
    #[test]
    fn unauthorized_token() {
        let (shutdown_handle, address) = run_mock_openid_server("unauthorized_token");

        let profile_opt = OpenIdProfileProvider::new(format!("{}{}", address, USERINFO_ENDPOINT))
            .get_profile("unknown_token")
            .expect("Failed to get profile");

        assert!(profile_opt.is_none());

        shutdown_handle.shutdown();
    }

    /// Verifies that the OpenID profile provider correctly returns an error when receiving an
    /// unexpected response code from the OpenID server.
    ///
    /// 1. Start the mock OpenID server
    /// 2. Attempt to get the profile for a token that the server will return a non-200 and non-401
    ///    response for
    /// 3. Verify that the profile provider returns an error
    /// 4. Shutdown the OpenID server
    #[test]
    fn unexpected_response_code() {
        let (shutdown_handle, address) = run_mock_openid_server("unauthorized_token");

        let profile_res = OpenIdProfileProvider::new(format!("{}{}", address, USERINFO_ENDPOINT))
            .get_profile(UNEXPECTED_RESPONSE_CODE_TOKEN);

        assert!(profile_res.is_err());

        shutdown_handle.shutdown();
    }

    /// Verifies that the OpenID profile provider correctly returns an error when receiving a
    /// response that doesn't contain the `sub` field.
    ///
    /// 1. Start the mock OpenID server
    /// 2. Attempt to get the profile for a token that the server will return an invalid response
    ///    for
    /// 3. Verify that the profile provider returns an error
    /// 4. Shutdown the OpenID server
    #[test]
    fn invalid_response() {
        let (shutdown_handle, address) = run_mock_openid_server("unauthorized_token");

        let profile_res = OpenIdProfileProvider::new(format!("{}{}", address, USERINFO_ENDPOINT))
            .get_profile(INVALID_RESPONSE_TOKEN);

        assert!(profile_res.is_err());

        shutdown_handle.shutdown();
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
                    App::new().service(web::resource(USERINFO_ENDPOINT).to(userinfo_endpoint))
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

    /// The handler for the OpenID server's user info endpoint.
    fn userinfo_endpoint(req: HttpRequest) -> HttpResponse {
        match req
            .headers()
            .get("Authorization")
            .and_then(|auth| auth.to_str().ok())
            .and_then(|auth_str| auth_str.strip_prefix("Bearer "))
        {
            Some(token) if token == ALL_DETAILS_TOKEN => HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "sub": SUB,
                    "name": NAME,
                    "given_name": GIVEN_NAME,
                    "family_name": FAMILY_NAME,
                    "email": EMAIL,
                    "picture": PICTURE,
                })),
            Some(token) if token == ONLY_SUB_TOKEN => HttpResponse::Ok()
                .content_type("application/json")
                .json(json!({
                    "sub": SUB,
                })),
            Some(token) if token == UNEXPECTED_RESPONSE_CODE_TOKEN => {
                HttpResponse::BadRequest().finish()
            }
            Some(token) if token == INVALID_RESPONSE_TOKEN => HttpResponse::Ok().finish(),
            Some(_) => HttpResponse::Unauthorized().finish(),
            None => HttpResponse::BadRequest().finish(),
        }
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
        assert_eq!(identity, Identity::User(user_id));
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
        assert_eq!(
            identity,
            Identity::User(original_session.user().user_id().into())
        );

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
            .with_profile_provider(Box::new(RefreshedTokenProfileProvider))
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
        assert_eq!(
            identity,
            Identity::User(original_session.user().user_id().into())
        );

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
            .with_profile_provider(Box::new(RefreshedTokenProfileProvider))
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
            .with_profile_provider(Box::new(AlwaysSomeProfileProvider))
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

    /// Profile provider that always returns a profile
    #[derive(Clone)]
    struct AlwaysSomeProfileProvider;

    impl ProfileProvider for AlwaysSomeProfileProvider {
        fn get_profile(&self, _access_token: &str) -> Result<Option<Profile>, InternalError> {
            let profile = Profile {
                subject: "subject".to_string(),
                name: None,
                given_name: None,
                family_name: None,
                email: None,
                picture: None,
            };
            Ok(Some(profile))
        }

        fn clone_box(&self) -> Box<dyn ProfileProvider> {
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
            .with_profile_provider(Box::new(AlwaysNoneProfileProvider))
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

    /// Profile provider that always returns `Ok(None)`
    #[derive(Clone)]
    struct AlwaysNoneProfileProvider;

    impl ProfileProvider for AlwaysNoneProfileProvider {
        fn get_profile(&self, _access_token: &str) -> Result<Option<Profile>, InternalError> {
            Ok(None)
        }

        fn clone_box(&self) -> Box<dyn ProfileProvider> {
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
            .with_profile_provider(Box::new(AlwaysErrProfileProvider))
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

    /// Profile provider that always returns `Err`
    #[derive(Clone)]
    struct AlwaysErrProfileProvider;

    impl ProfileProvider for AlwaysErrProfileProvider {
        fn get_profile(&self, _access_token: &str) -> Result<Option<Profile>, InternalError> {
            Err(InternalError::with_message("error".into()))
        }

        fn clone_box(&self) -> Box<dyn ProfileProvider> {
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

    /// Profile provider that returns a profile when the new `NEW_OAUTH_ACCESS_TOKEN` is provided;
    /// returns `Ok(None)` otherwise.
    #[derive(Clone)]
    struct RefreshedTokenProfileProvider;

    impl ProfileProvider for RefreshedTokenProfileProvider {
        fn get_profile(&self, access_token: &str) -> Result<Option<Profile>, InternalError> {
            if access_token == NEW_OAUTH_ACCESS_TOKEN {
                let profile = Profile {
                    subject: "subject".to_string(),
                    name: None,
                    given_name: None,
                    family_name: None,
                    email: None,
                    picture: None,
                };
                Ok(Some(profile))
            } else {
                Ok(None)
            }
        }

        fn clone_box(&self) -> Box<dyn ProfileProvider> {
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
