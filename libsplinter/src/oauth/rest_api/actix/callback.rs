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

//! The `GET /oauth/callback` endpoint for receiving the authorization code from the provider and
//! exchanging it for an access token.

use actix_web::{http::header::LOCATION, web::Query, HttpResponse};
use futures::future::IntoFuture;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::biome::oauth::store::{InsertableOAuthUserSessionBuilder, OAuthUserSessionStore};
use crate::oauth::{
    rest_api::resources::callback::{generate_redirect_query, CallbackQuery},
    OAuthClient,
};
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

pub fn make_callback_route(
    client: OAuthClient,
    oauth_user_session_store: Box<dyn OAuthUserSessionStore>,
) -> Resource {
    Resource::build("/oauth/callback")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::OAUTH_CALLBACK_MIN,
            protocol::OAUTH_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |req, _| {
            Box::new(
                match Query::<CallbackQuery>::from_query(req.query_string()) {
                    Ok(query) => {
                        match client.exchange_authorization_code(query.code.clone(), &query.state) {
                            Ok(Some((user_info, redirect_url))) => {
                                // Generate a Splinter access token for the new session
                                let splinter_access_token = new_splinter_access_token();

                                // Adding the token and subject to the redirect URL so the client
                                // may access these values after a redirect
                                let redirect_url = format!(
                                    "{}?{}",
                                    redirect_url,
                                    generate_redirect_query(
                                        &splinter_access_token,
                                        user_info.subject()
                                    )
                                );

                                // Save the new session
                                match InsertableOAuthUserSessionBuilder::new()
                                    .with_splinter_access_token(splinter_access_token)
                                    .with_subject(user_info.subject().to_string())
                                    .with_oauth_access_token(user_info.access_token().to_string())
                                    .with_oauth_refresh_token(
                                        user_info.refresh_token().map(ToOwned::to_owned),
                                    )
                                    .build()
                                {
                                    Ok(session) => {
                                        match oauth_user_session_store.add_session(session) {
                                            Ok(_) => HttpResponse::Found()
                                                .header(LOCATION, redirect_url)
                                                .finish(),
                                            Err(err) => {
                                                error!("Unable to store user session: {}", err);
                                                HttpResponse::InternalServerError()
                                                    .json(ErrorResponse::internal_error())
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        error!("Unable to build user session: {}", err);
                                        HttpResponse::InternalServerError()
                                            .json(ErrorResponse::internal_error())
                                    }
                                }
                            }
                            Ok(None) => {
                                error!(
                                    "Received OAuth callback request that does not correlate to an \
                                     open authorization request"
                                );
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                            }
                            Err(err) => {
                                error!("{}", err);
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                            }
                        }
                    }
                    Err(err) => {
                        error!(
                            "Failed to parse query string in OAuth callback request: {}",
                            err
                        );
                        HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                    }
                }
                .into_future(),
            )
        })
}

/// Generates a new Splinter access token, which is a string of 32 random alphanumeric characters
fn new_splinter_access_token() -> String {
    thread_rng().sample_iter(&Alphanumeric).take(32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpResponse, HttpServer};
    use futures::Future;
    use reqwest::{blocking::Client, redirect, StatusCode, Url as ReqwestUrl};
    use url::Url;

    use crate::biome::MemoryOAuthUserSessionStore;
    use crate::rest_api::{RestApiBuilder, RestApiShutdownHandle};

    use crate::oauth::{
        new_basic_client,
        store::{InflightOAuthRequestStore, MemoryInflightOAuthRequestStore},
        tests::TestSubjectProvider,
        PendingAuthorization,
    };

    const TOKEN_ENDPOINT: &str = "/token";
    const AUTH_CODE: &str = "auth_code";
    const SUBJECT: &str = "subject";
    const OAUTH_ACCESS_TOKEN: &str = "oauth_access_token";
    const OAUTH_REFRESH_TOKEN: &str = "oauth_refresh_token";

    /// Verifies the correct functionality of the `GET /oauth/callback` endpoint when the request
    /// is correct
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new InflightOAuthRequestStore and add a pending authorization
    /// 3. Create a new OAuthClient with the pre-populated in-flight request store
    /// 4. Create a new OAuthUserSessionStore
    /// 5. Run the Splinter REST API on an open port with the `GET /oauth/callback` endpoint backed
    ///    by the OAuth client and session store
    /// 6. Make the `GET /oauth/callback` request with an authorization code and the state (CSRF
    ///    token of pending authorization)
    /// 7. Verify the response has status `302 Found` and the `Location` header is set to the
    ///    correct client redirect URL with the correct query parameters
    /// 8. Verify the session was added to the session store
    /// 9. Shutdown the Splinter REST API
    /// 10. Stop the mock OAuth server
    #[test]
    fn get_callback_ok() {
        let (oauth_shutdown_handle, address) = run_mock_oauth_server("get_callback_ok");

        let request_store = Box::new(MemoryInflightOAuthRequestStore::new());
        let csrf_token = "csrf_token";
        let client_redirect_url =
            Url::parse("http://client/redirect").expect("Failed to parse client redirect URL");
        request_store
            .insert_request(
                csrf_token.into(),
                PendingAuthorization {
                    pkce_verifier: "F9ZfayKQHV5exVsgM3WyzRt15UQvYxVZBm41iO-h20A".into(),
                    client_redirect_url: client_redirect_url.as_str().into(),
                },
            )
            .expect("Failed to insert in-flight request");

        let client = OAuthClient::new(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "http://oauth/auth".into(),
                "http://oauth/callback".into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            request_store.clone(),
        )
        .expect("Failed to create client");

        let session_store = MemoryOAuthUserSessionStore::new();

        let (splinter_shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_callback_route(client, session_store.clone_box())]);

        let url = ReqwestUrl::parse_with_params(
            &format!("http://{}/oauth/callback", bind_url),
            &[("code", AUTH_CODE), ("state", csrf_token)],
        )
        .expect("Failed to parse URL");
        let resp = Client::builder()
            // Disable redirects so the client doesn't actually go to the client redirect URL
            .redirect(redirect::Policy::none())
            .build()
            .expect("Failed to build client")
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::FOUND);

        let location = Url::parse(
            resp.headers()
                .get("Location")
                .expect("Location header not set")
                .to_str()
                .expect("Location header should only contain visible ASCII characters"),
        )
        .expect("Failed to parse location");
        assert_eq!(location.origin(), client_redirect_url.origin());

        let query_map: HashMap<String, String> = location.query_pairs().into_owned().collect();
        let access_token = query_map
            .get("access_token")
            .expect("Missing access_token")
            .strip_prefix("OAuth2:")
            .expect("Access token invalid");
        assert_eq!(
            query_map.get("display_name").expect("Missing display_name"),
            SUBJECT
        );

        let session = session_store
            .get_session(access_token)
            .expect("Failed to get session")
            .expect("Session missing");
        assert_eq!(session.splinter_access_token(), access_token);
        assert_eq!(session.user().subject(), SUBJECT);
        assert_eq!(session.oauth_access_token(), OAUTH_ACCESS_TOKEN);
        assert_eq!(
            session
                .oauth_refresh_token()
                .expect("oauth_refresh_token missing"),
            OAUTH_REFRESH_TOKEN
        );

        splinter_shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");

        oauth_shutdown_handle.shutdown();
    }

    /// Verifies the correct functionality of the `GET /oauth/callback` endpoint when the request
    /// has an unknown state parameter (CSRF token)
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new OAuthClient with an empty in-flight request store
    /// 3. Create a new OAuthUserSessionStore
    /// 4. Run the Splinter REST API on an open port with the `GET /oauth/callback` endpoint backed
    ///    by the OAuth client and session store
    /// 5. Make the `GET /oauth/callback` request with an authorization code and an unknown state
    ///    (CSRF token)
    /// 6. Verify the response has status `500 Internal Server Error` (this is an internal error
    ///    from the authenticating client's perspective becuase this request is made by the OAuth
    ///    server)
    /// 7. Shutdown the Splinter REST API
    /// 8. Stop the mock OAuth server
    #[test]
    fn get_callback_unknown_state() {
        let (oauth_shutdown_handle, address) = run_mock_oauth_server("get_callback_unknown_state");

        let client = OAuthClient::new(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "http://oauth/auth".into(),
                "http://oauth/callback".into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            Box::new(MemoryInflightOAuthRequestStore::new()),
        )
        .expect("Failed to create client");

        let session_store = MemoryOAuthUserSessionStore::new();

        let (splinter_shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_callback_route(client, session_store.clone_box())]);

        let url = ReqwestUrl::parse_with_params(
            &format!("http://{}/oauth/callback", bind_url),
            &[("code", AUTH_CODE), ("state", "csrf_token")],
        )
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        splinter_shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");

        oauth_shutdown_handle.shutdown();
    }

    /// Verifies the correct functionality of the `GET /oauth/callback` endpoint when the request
    /// does not provide a state parameter (CSRF token)
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new InflightOAuthRequestStore and add a pending authorization
    /// 3. Create a new OAuthClient with the pre-populated in-flight request store
    /// 4. Create a new OAuthUserSessionStore
    /// 5. Run the Splinter REST API on an open port with the `GET /oauth/callback` endpoint backed
    ///    by the OAuth client and session store
    /// 6. Make the `GET /oauth/callback` request with an authorization code but no state
    /// 7. Verify the response has status `500 Internal Server Error` (this is an internal error
    ///    from the authenticating client's perspective becuase this request is made by the OAuth
    ///    server)
    /// 8. Shutdown the Splinter REST API
    /// 9. Stop the mock OAuth server
    #[test]
    fn get_callback_no_state() {
        let (oauth_shutdown_handle, address) = run_mock_oauth_server("get_callback_no_state");

        let request_store = Box::new(MemoryInflightOAuthRequestStore::new());
        request_store
            .insert_request(
                "csrf_token".into(),
                PendingAuthorization {
                    pkce_verifier: "F9ZfayKQHV5exVsgM3WyzRt15UQvYxVZBm41iO-h20A".into(),
                    client_redirect_url: "http://client/redirect".into(),
                },
            )
            .expect("Failed to insert in-flight request");

        let client = OAuthClient::new(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "http://oauth/auth".into(),
                "http://oauth/callback".into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            request_store.clone(),
        )
        .expect("Failed to create client");

        let session_store = MemoryOAuthUserSessionStore::new();

        let (splinter_shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_callback_route(client, session_store.clone_box())]);

        let url = ReqwestUrl::parse_with_params(
            &format!("http://{}/oauth/callback", bind_url),
            &[("code", AUTH_CODE)],
        )
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        splinter_shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");

        oauth_shutdown_handle.shutdown();
    }

    /// Verifies the correct functionality of the `GET /oauth/callback` endpoint when the request
    /// does not provide an authorization code parameter
    ///
    /// 1. Start the mock OAuth server
    /// 2. Create a new InflightOAuthRequestStore and add a pending authorization
    /// 3. Create a new OAuthClient with the pre-populated in-flight request store
    /// 4. Create a new OAuthUserSessionStore
    /// 5. Run the Splinter REST API on an open port with the `GET /oauth/callback` endpoint backed
    ///    by the OAuth client and session store
    /// 6. Make the `GET /oauth/callback` request with a state but no authorization code
    /// 7. Verify the response has status `500 Internal Server Error` (this is an internal error
    ///    from the authenticating client's perspective becuase this request is made by the OAuth
    ///    server)
    /// 8. Shutdown the Splinter REST API
    /// 9. Stop the mock OAuth server
    #[test]
    fn get_callback_no_authorization_code() {
        let (oauth_shutdown_handle, address) =
            run_mock_oauth_server("get_callback_no_authorization_code");

        let request_store = Box::new(MemoryInflightOAuthRequestStore::new());
        let csrf_token = "csrf_token";
        request_store
            .insert_request(
                csrf_token.into(),
                PendingAuthorization {
                    pkce_verifier: "F9ZfayKQHV5exVsgM3WyzRt15UQvYxVZBm41iO-h20A".into(),
                    client_redirect_url: "http://client/redirect".into(),
                },
            )
            .expect("Failed to insert in-flight request");

        let client = OAuthClient::new(
            new_basic_client(
                "client_id".into(),
                "client_secret".into(),
                "http://oauth/auth".into(),
                "http://oauth/callback".into(),
                format!("{}{}", address, TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            request_store.clone(),
        )
        .expect("Failed to create client");

        let session_store = MemoryOAuthUserSessionStore::new();

        let (splinter_shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_callback_route(client, session_store.clone_box())]);

        let url = ReqwestUrl::parse_with_params(
            &format!("http://{}/oauth/callback", bind_url),
            &[("state", csrf_token)],
        )
        .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        splinter_shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");

        oauth_shutdown_handle.shutdown();
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
        assert_eq!(&form.grant_type, "authorization_code");
        assert_eq!(&form.code, AUTH_CODE);

        HttpResponse::Ok()
            .content_type("application/json")
            .json(json!({
                "token_type": "bearer",
                "access_token": OAUTH_ACCESS_TOKEN,
                "refresh_token": OAUTH_REFRESH_TOKEN,
                "expires_in": 3600,
            }))
    }

    #[derive(Deserialize)]
    struct TokenRequestForm {
        code: String,
        grant_type: String,
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

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::RestApiBind::Insecure("127.0.0.1:0".into());

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .build_insecure()
            .expect("Failed to build REST API")
            .run_insecure();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }
}
