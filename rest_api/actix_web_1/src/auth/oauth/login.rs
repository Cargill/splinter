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

//! The `GET /oauth/login` endpoint for getting the authorization request URL for the provider.

use actix_web::{http::header::LOCATION, web, HttpResponse};
use futures::future::IntoFuture;
use std::collections::HashMap;

use splinter::oauth::OAuthClient;
use splinter_rest_api_common::{response_models::ErrorResponse, SPLINTER_PROTOCOL_VERSION};

use crate::framework::{Method, ProtocolVersionRangeGuard, Resource};
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

const OAUTH_LOGIN_MIN: u32 = 1;

pub fn make_login_route(client: OAuthClient) -> Resource {
    let resource = Resource::build("/oauth/login").add_request_guard(
        ProtocolVersionRangeGuard::new(OAUTH_LOGIN_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(
            Method::Get,
            Permission::AllowUnauthenticated,
            move |req, _| {
                let query: web::Query<HashMap<String, String>> =
                    if let Ok(q) = web::Query::from_query(req.query_string()) {
                        q
                    } else {
                        return Box::new(
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid query"))
                                .into_future(),
                        );
                    };
                let client_redirect_url = if let Some(header_value) = query.get("redirect_url") {
                    header_value
                } else {
                    match req.headers().get("referer") {
                        Some(url) => match url.to_str() {
                            Ok(url) => url,
                            Err(_) => {
                                return Box::new(
                                    HttpResponse::BadRequest()
                                        .json(ErrorResponse::bad_request(
                                            "Referer header is set, but is not a valid URL",
                                        ))
                                        .into_future(),
                                )
                            }
                        },
                        None => {
                            return Box::new(
                                HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(
                                        "No valid redirect URL supplied",
                                    ))
                                    .into_future(),
                            )
                        }
                    }
                };

                Box::new(
                    match client.get_authorization_url(client_redirect_url.to_string()) {
                        Ok(auth_url) => HttpResponse::Found().header(LOCATION, auth_url).finish(),
                        Err(err) => {
                            error!("{}", err);
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                        }
                    }
                    .into_future(),
                )
            },
        )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |req, _| {
            let query: web::Query<HashMap<String, String>> =
                if let Ok(q) = web::Query::from_query(req.query_string()) {
                    q
                } else {
                    return Box::new(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request("Invalid query"))
                            .into_future(),
                    );
                };
            let client_redirect_url = if let Some(header_value) = query.get("redirect_url") {
                header_value
            } else {
                match req.headers().get("referer") {
                    Some(url) => match url.to_str() {
                        Ok(url) => url,
                        Err(_) => {
                            return Box::new(
                                HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(
                                        "No valid redirect URL supplied",
                                    ))
                                    .into_future(),
                            )
                        }
                    },
                    None => {
                        return Box::new(
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("No valid redirect URL supplied"))
                                .into_future(),
                        )
                    }
                }
            };

            Box::new(
                match client.get_authorization_url(client_redirect_url.to_string()) {
                    Ok(auth_url) => HttpResponse::Found().header(LOCATION, auth_url).finish(),
                    Err(err) => {
                        error!("{}", err);
                        HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
                    }
                }
                .into_future(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use oauth2::basic::BasicClient;
    use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
    use reqwest::{blocking::Client, redirect, StatusCode, Url};
    use splinter::error::InternalError;
    use splinter::error::InvalidArgumentError;
    use splinter::oauth::{
        store::{
            InflightOAuthRequestStore, InflightOAuthRequestStoreError,
            MemoryInflightOAuthRequestStore,
        },
        PendingAuthorization,
    };
    use splinter::oauth::{Profile, ProfileProvider, SubjectProvider};

    use crate::framework::{RestApiBuilder, RestApiShutdownHandle};

    const CLIENT_ID: &str = "client_id";
    const CLIENT_SECRET: &str = "client_secret";
    const AUTH_URL: &str = "http://oauth/auth";
    const REDIRECT_URL: &str = "http://oauth/callback";
    const TOKEN_ENDPOINT: &str = "/token";
    const CLIENT_REDIRECT_URL: &str = "http://client/redirect";
    const SUBJECT: &str = "subject";

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
    pub struct TestProfileProvider;

    impl ProfileProvider for TestProfileProvider {
        fn get_profile(&self, _: &str) -> Result<Option<Profile>, InternalError> {
            Ok(Some(Profile {
                subject: "".to_string(),
                name: None,
                given_name: None,
                family_name: None,
                email: None,
                picture: None,
            }))
        }

        fn clone_box(&self) -> Box<dyn ProfileProvider> {
            Box::new(self.clone())
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
                .map_err(|err| InvalidArgumentError::new("auth_url", err.to_string()))?,
            Some(
                TokenUrl::new(token_url)
                    .map_err(|err| InvalidArgumentError::new("token_url", err.to_string()))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(redirect_url)
                .map_err(|err| InvalidArgumentError::new("redirect_url", err.to_string()))?,
        ))
    }

    /// Verifies the correct functionality of the `GET /oauth/login` endpoint when the client
    /// redirect is specified in the request's query
    ///
    /// 1. Create a new OAuth client using the in-flight request store that verifies the client
    ///    redirect URL
    /// 2. Run the Splinter REST API on an open port with the `GET /oauth/login` endpoint backed by
    ///    the OAuth client
    /// 3. Make the `GET /oauth/login` request with the `redirect_url` query parameter set (the
    ///    in-flight request store implementation will verify the redirect is correctly parsed by
    ///    the endpoint handler)
    /// 4. Verify the response has status `302 Found` and the `Location` header is set to the
    ///    correct authorization URL
    /// 5. Shutdown the REST API

    #[test]
    fn get_login_with_redirect_url() {
        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                AUTH_URL.into(),
                REDIRECT_URL.into(),
                format!("http://oauth{}", TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            Box::new(TestInflightOAuthRequestStore),
            Box::new(TestProfileProvider),
        );

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_login_route(client)]);

        let url = Url::parse_with_params(
            &format!("http://{}/oauth/login", bind_url),
            &[("redirect_url", CLIENT_REDIRECT_URL)],
        )
        .expect("Failed to parse URL");
        let resp = Client::builder()
            // Disable redirects so the client doesn't actually go to the mock auth URL
            .redirect(redirect::Policy::none())
            .build()
            .expect("Failed to build client")
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::FOUND);
        assert!(resp
            .headers()
            .get("Location")
            .expect("Location header not set")
            .to_str()
            .expect("Location header should only contain visible ASCII characters")
            .starts_with(AUTH_URL));

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verifies the correct functionality of the `GET /oauth/login` endpoint when the client
    /// redirect is specified by the client using the "Referer" header.
    ///
    /// 1. Create a new OAuth client using the in-flight request store that verifies the client
    ///    redirect URL
    /// 2. Run the Splinter REST API on an open port with the `GET /oauth/login` endpoint backed by
    ///    the OAuth client
    /// 3. Make the `GET /oauth/login` request with the "Referer" header set (the in-flight request
    ///    store implementation will verify the redirect is correctly parsed by the endpoint
    ///    handler)
    /// 4. Verify the response has status `302 Found` and the `Location` header is set to the
    ///    correct authorization URL
    /// 5. Shutdown the REST API
    #[test]
    fn get_login_with_referer_header() {
        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                AUTH_URL.into(),
                REDIRECT_URL.into(),
                format!("http://oauth{}", TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            Box::new(TestInflightOAuthRequestStore),
            Box::new(TestProfileProvider),
        );

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_login_route(client)]);

        let url =
            Url::parse(&format!("http://{}/oauth/login", bind_url)).expect("Failed to parse URL");
        let resp = Client::builder()
            // Disable redirects so the client doesn't actually go to the mock auth URL
            .redirect(redirect::Policy::none())
            .build()
            .expect("Failed to build client")
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .header("Referer", CLIENT_REDIRECT_URL)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::FOUND);
        assert!(resp
            .headers()
            .get("Location")
            .expect("Location header not set")
            .to_str()
            .expect("Location header should only contain visible ASCII characters")
            .starts_with(AUTH_URL));

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verifies that the `GET /oauth/login` endpoint responds with `400 Bad Request` when the
    /// client does not provide a client redirect URL with the query parameter or header.
    ///
    /// 1. Create a new OAuth client
    /// 2. Run the Splinter REST API on an open port with the `GET /oauth/login` endpoint backed by
    ///    the OAuth client
    /// 3. Make the `GET /oauth/login` request without a client redirect URL set
    /// 4. Verify the response has status `400 Bad Request`
    /// 5. Shutdown the REST API
    #[test]
    fn get_login_missing_client_redirect() {
        let client = OAuthClient::new(
            new_basic_client(
                CLIENT_ID.into(),
                CLIENT_SECRET.into(),
                AUTH_URL.into(),
                REDIRECT_URL.into(),
                format!("http://oauth{}", TOKEN_ENDPOINT),
            )
            .expect("Failed to create basic client"),
            vec![],
            vec![],
            Box::new(TestSubjectProvider),
            Box::new(MemoryInflightOAuthRequestStore::new()),
            Box::new(TestProfileProvider),
        );

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_login_route(client)]);

        let url =
            Url::parse(&format!("http://{}/oauth/login", bind_url)).expect("Failed to parse URL");
        let resp = Client::builder()
            // Disable redirects so the client doesn't actually go to the mock auth URL
            .redirect(redirect::Policy::none())
            .build()
            .expect("Failed to build client")
            .get(url)
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION)
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Checks that the inserted authorization always has the expected client redirect URL
    #[derive(Clone)]
    pub struct TestInflightOAuthRequestStore;

    impl InflightOAuthRequestStore for TestInflightOAuthRequestStore {
        fn insert_request(
            &self,
            _request_id: String,
            authorization: PendingAuthorization,
        ) -> Result<(), InflightOAuthRequestStoreError> {
            assert_eq!(
                &authorization.get_client_redirect_url(),
                &CLIENT_REDIRECT_URL
            );
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

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources)
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
