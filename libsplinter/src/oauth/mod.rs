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
    CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope,
    TokenResponse, TokenUrl,
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
    /// Extra parameters that will be added to an authorization request
    extra_auth_params: Vec<(String, String)>,
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
    /// * `extra_auth_params` - Extra parameters that will be added to an authorization request
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
        extra_auth_params: Vec<(String, String)>,
        scopes: Vec<String>,
        subject_provider: Box<dyn SubjectProvider>,
        inflight_request_store: Box<dyn InflightOAuthRequestStore>,
    ) -> Result<Self, InvalidArgumentError> {
        Ok(Self {
            client,
            extra_auth_params,
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
        for (key, value) in self.extra_auth_params.iter() {
            request = request.add_extra_param(key, value);
        }
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

    /// Exchanges the given refresh token for an access token.
    pub fn exchange_refresh_token(&self, refresh_token: String) -> Result<String, InternalError> {
        self.client
            .exchange_refresh_token(&RefreshToken::new(refresh_token))
            .request(http_client)
            .map(|response| response.access_token().secret().into())
            .map_err(|err| {
                InternalError::with_message(format!(
                    "failed to make refresh token exchange request: {}",
                    err,
                ))
            })
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

    use std::collections::HashMap;

    use url::Url;

    use super::store::{InflightOAuthRequestStoreError, MemoryInflightOAuthRequestStore};

    const CLIENT_ID: &str = "client_id";
    const CLIENT_SECRET: &str = "client_secret";
    const AUTH_URL: &str = "http://oauth/auth";
    const REDIRECT_URL: &str = "http://oauth/callback";
    const TOKEN_ENDPOINT: &str = "/token";
    const EXTRA_AUTH_PARAM_KEY: &str = "key";
    const EXTRA_AUTH_PARAM_VAL: &str = "val";
    const SCOPE1: &str = "scope1";
    const SCOPE2: &str = "scope2";
    const CLIENT_REDIRECT_URL: &str = "http://client/redirect";
    const SUBJECT: &str = "subject";

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

    /// Verifies that the OAuth client generates a correct authorization URL based on its
    /// configuration and inputs.
    ///
    /// 1. Create a new OAuthClient
    /// 2. Get a new authorization URL from the client
    /// 3. Verify that the base URL (origin) is correct
    /// 4. Verify that all the expected query parameters are set to the correct values
    /// 5. Verify that the correct CSRF state, PKCE verifier, and client redirect URL were saved in
    ///    the in-flight request store.
    #[test]
    fn get_authorization_url() {
        let auth_url = Url::parse(AUTH_URL).expect("Failed to parse auth url");
        let request_store = Box::new(MemoryInflightOAuthRequestStore::new());
        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                auth_url.as_str().into(),
                REDIRECT_URL.into(),
                format!("http://oauth{}", TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![(EXTRA_AUTH_PARAM_KEY.into(), EXTRA_AUTH_PARAM_VAL.into())],
            vec![SCOPE1.into(), SCOPE2.into()],
            Box::new(TestSubjectProvider),
            request_store.clone(),
        )
        .expect("Failed to create client");

        let generated_auth_url = Url::parse(
            &client
                .get_authorization_url(CLIENT_REDIRECT_URL.into())
                .expect("Failed to generate auth URL"),
        )
        .expect("Failed to parse generated auth URL");

        assert_eq!(auth_url.origin(), generated_auth_url.origin());

        let query_map: HashMap<String, String> =
            generated_auth_url.query_pairs().into_owned().collect();
        assert_eq!(
            query_map.get("client_id").expect("Missing client_id"),
            CLIENT_ID,
        );
        assert_eq!(
            query_map.get("redirect_uri").expect("Missing redirect_uri"),
            REDIRECT_URL,
        );
        assert_eq!(
            query_map
                .get(EXTRA_AUTH_PARAM_KEY)
                .expect("Missing extra auth param"),
            EXTRA_AUTH_PARAM_VAL,
        );
        assert_eq!(
            query_map.get("scope").expect("Missing scope"),
            &format!("{} {}", SCOPE1, SCOPE2),
        );
        assert_eq!(
            query_map
                .get("response_type")
                .expect("Missing response_type"),
            "code",
        );
        assert_eq!(
            query_map
                .get("code_challenge_method")
                .expect("Missing code_challenge_method"),
            "S256",
        );
        let code_challenge = query_map
            .get("code_challenge")
            .expect("Missing code_challenge");
        let state = query_map.get("state").expect("Missing state");

        let pending_authorization = request_store
            .remove_request(state)
            .expect("Failed to get pending authorization")
            .expect("Pending authorization not saved");
        assert_eq!(
            &pending_authorization.client_redirect_url,
            CLIENT_REDIRECT_URL
        );
        assert_eq!(
            PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(
                pending_authorization.pkce_verifier
            ))
            .as_str(),
            code_challenge.as_str(),
        );
    }

    #[derive(Clone)]
    pub struct TestSubjectProvider;

    impl SubjectProvider for TestSubjectProvider {
        fn get_subject(&self, _: &str) -> Result<Option<String>, InternalError> {
            Ok(Some(SUBJECT.to_string()))
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

/// These tests require actix to be enabled
#[cfg(test)]
#[cfg(all(feature = "actix", feature = "actix-web", feature = "futures"))]
mod actix_tests {
    use super::*;

    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
    use futures::Future;

    use crate::oauth::store::MemoryInflightOAuthRequestStore;

    use super::tests::TestSubjectProvider;

    const CLIENT_ID: &str = "client_id";
    const CLIENT_SECRET: &str = "client_secret";
    const AUTH_URL: &str = "http://oauth/auth";
    const REDIRECT_URL: &str = "http://oauth/callback";
    const TOKEN_ENDPOINT: &str = "/token";
    const CLIENT_REDIRECT_URL: &str = "http://client/redirect";
    const AUTH_CODE: &str = "auth_code";
    const MOCK_PKCE_VERIFIER: &str = "F9ZfayKQHV5exVsgM3WyzRt15UQvYxVZBm41iO-h20A";
    const ACCESS_TOKEN: &str = "access_token";
    const REFRESH_TOKEN: &str = "refresh_token";
    const EXPIRES_IN: Duration = Duration::from_secs(3600);
    const SUBJECT: &str = "subject";

    /// Verifies that the OAuth client correctly handles exchanging an authorization code for the
    /// user's tokens and returns the correct user values in the `exchange_authorization_code`
    /// method.
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new InflightOAuthRequestStore and add a pending authorization
    /// 3. Create a new OAuthClient with the pre-populated in-flight request store
    /// 4. Call `exchange_authorization_code` with the CSRF token of the pending authorization; the
    ///    mock server will verify that the correct data was sent.
    /// 5. Verify that the returned user info and client redirect URL are correct
    /// 6. Verify that the pending authorization has been removed from the store and calling
    ///    `exchange_authorization_code` again returns `Ok(None)`
    /// 7. Stop the mock OAuth server
    #[test]
    fn exchange_authorization_code() {
        let (shutdown_handle, address) = run_mock_oauth_server("exchange_authorization_code");

        let request_store = Box::new(MemoryInflightOAuthRequestStore::new());
        let csrf_token = "csrf_token";
        request_store
            .insert_request(
                csrf_token.into(),
                PendingAuthorization {
                    pkce_verifier: MOCK_PKCE_VERIFIER.into(),
                    client_redirect_url: CLIENT_REDIRECT_URL.into(),
                },
            )
            .expect("Failed to insert in-flight request");

        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                AUTH_URL.into(),
                REDIRECT_URL.into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            request_store.clone(),
        )
        .expect("Failed to create client");

        let (user_info, client_redirect_url) = client
            .exchange_authorization_code(AUTH_CODE.into(), csrf_token)
            .expect("Failed to exchange authorization code")
            .expect("Pending request not found");

        assert_eq!(&user_info.access_token, ACCESS_TOKEN);
        assert_eq!(
            user_info.expires_in.expect("expires_in missing"),
            EXPIRES_IN
        );
        assert_eq!(
            &user_info.refresh_token.expect("refresh_token missing"),
            REFRESH_TOKEN
        );
        assert_eq!(&user_info.subject, SUBJECT);
        assert_eq!(&client_redirect_url, CLIENT_REDIRECT_URL);

        assert!(request_store
            .remove_request(csrf_token)
            .expect("Failed to check in-flight request store")
            .is_none());
        assert!(client
            .exchange_authorization_code(AUTH_CODE.into(), csrf_token)
            .expect("Failed to exchange authorization code")
            .is_none());

        shutdown_handle.shutdown();
    }

    /// Verifies that the OAuth client correctly handles exchanging a refresh token for a new access
    /// access token with the `exchange_refresh_token` method.
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new OAuthClient
    /// 3. Call `exchange_refresh_token`; the mock server will verify that the correct data was sent.
    /// 4. Verify that the returned access token is correct
    /// 5. Stop the mock OAuth server
    #[test]
    fn exchange_refresh_token() {
        let (shutdown_handle, address) = run_mock_oauth_server("exchange_refresh_token");

        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                AUTH_URL.into(),
                REDIRECT_URL.into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            Box::new(MemoryInflightOAuthRequestStore::new()),
        )
        .expect("Failed to create client");

        let access_token = client
            .exchange_refresh_token(REFRESH_TOKEN.into())
            .expect("Failed to exchange refresh token");

        assert_eq!(&access_token, ACCESS_TOKEN);

        shutdown_handle.shutdown();
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
        match form.grant_type.as_str() {
            "authorization_code" => {
                assert_eq!(form.code.as_deref(), Some(AUTH_CODE));
                assert_eq!(form.code_verifier.as_deref(), Some(MOCK_PKCE_VERIFIER));
                assert_eq!(form.redirect_uri.as_deref(), Some(REDIRECT_URL));

                HttpResponse::Ok()
                    .content_type("application/json")
                    .json(json!({
                        "token_type": "bearer",
                        "access_token": ACCESS_TOKEN,
                        "refresh_token": REFRESH_TOKEN,
                        "expires_in": EXPIRES_IN.as_secs(),
                    }))
            }
            "refresh_token" => {
                assert_eq!(form.refresh_token.as_deref(), Some(REFRESH_TOKEN));

                HttpResponse::Ok()
                    .content_type("application/json")
                    .json(json!({
                        "token_type": "bearer",
                        "access_token": ACCESS_TOKEN,
                    }))
            }
            _ => panic!("Invalid grant_type"),
        }
    }

    #[derive(Deserialize)]
    struct TokenRequestForm {
        grant_type: String,
        // Authorization code requests
        code: Option<String>,
        code_verifier: Option<String>,
        redirect_uri: Option<String>,
        // Refresh token requests
        refresh_token: Option<String>,
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
