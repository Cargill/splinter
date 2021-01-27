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

use reqwest::{blocking::Client, StatusCode};

use crate::error::InternalError;

use super::SubjectProvider;

#[derive(Clone)]
pub struct OpenIdSubjectProvider {
    userinfo_endpoint: String,
}

impl OpenIdSubjectProvider {
    pub fn new(userinfo_endpoint: String) -> OpenIdSubjectProvider {
        OpenIdSubjectProvider { userinfo_endpoint }
    }
}

impl SubjectProvider for OpenIdSubjectProvider {
    fn get_subject(&self, access_token: &str) -> Result<Option<String>, InternalError> {
        let response = Client::builder()
            .build()
            .map_err(|err| InternalError::from_source(err.into()))?
            .get(&self.userinfo_endpoint)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .map_err(|err| InternalError::from_source(err.into()))?;

        if !response.status().is_success() {
            match response.status() {
                StatusCode::UNAUTHORIZED => return Ok(None),
                status_code => {
                    return Err(InternalError::with_message(format!(
                        "Received unexpected response code: {}",
                        status_code
                    )))
                }
            }
        }

        let subject = response
            .json::<UserResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?
            .sub;

        Ok(Some(subject))
    }

    fn clone_box(&self) -> Box<dyn SubjectProvider> {
        Box::new(self.clone())
    }
}

/// Deserializes response
#[derive(Debug, Deserialize)]
struct UserResponse {
    sub: String,
}

#[cfg(test)]
#[cfg(all(feature = "actix", feature = "actix-web", feature = "futures"))]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpRequest, HttpResponse, HttpServer};
    use futures::Future;

    const ACCESS_TOKEN: &str = "access_token";
    const SUBJECT_IDENTIFIER: &str = "AAAAAAAAAAAAAAAAAAAQEh-c1Zkltuwhd-12345";
    const USER_INFO_ENDPOINT: &str = "/userinfo";

    /// Verifies that the `OpenIdSubjectProvider` `get_subject` method successfully returns the `sub`
    /// value from the Openid OAuth provider's `user_info` endpoint when passed a valid access token.
    ///
    /// 1. Start the mock Openid server
    /// 2. Create a new OpenIdSubjectProvider with the address of the `user_info` endpoint
    /// 3. Call `get_subject`; the mock server will verify that the correct data was sent.
    /// 4. Verify that the returned access `sub` value is correct
    /// 5. Stop the mock Openid server
    #[test]
    fn get_subject_success() {
        let (shutdown_handle, address) = run_mock_openid_server("get_subject", user_info_endpoint);
        let subject_provider =
            OpenIdSubjectProvider::new(format!("{}{}", address, USER_INFO_ENDPOINT));

        let subject = subject_provider
            .get_subject(ACCESS_TOKEN)
            .expect("Failed to retrieve subject");

        assert_eq!(subject, Some(SUBJECT_IDENTIFIER.to_string()));

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OpenIdSubjectProvider` `get_subject` method returns None if passed
    /// an invalid access token
    ///
    /// 1. Start the mock Openid server
    /// 2. Create a new OpenIdSubjectProvider with the address of the `user_info` endpoint
    /// 3. Call `get_subject` with an invalid access token
    /// 4. Verify that None is returned
    /// 5. Stop the mock Openid server
    #[test]
    fn get_subject_invalid_token() {
        let (shutdown_handle, address) =
            run_mock_openid_server("get_subject_bad_token", user_info_endpoint);
        let subject_provider =
            OpenIdSubjectProvider::new(format!("{}{}", address, USER_INFO_ENDPOINT));

        assert!(subject_provider
            .get_subject("invalid_token")
            .unwrap()
            .is_none());

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OpenIdSubjectProvider` `get_subject` method returns an error if
    /// the `user_info` endpoint returns a json object that does not have a `sub` field.
    ///
    /// 1. Start the mock Openid server
    /// 2. Create a new OpenIdSubjectProvider with the address of the `user_info` endpoint
    ///    that returns a json object with no `sub` field
    /// 3. Call `get_subject` with an valid access token
    /// 4. Verify that an error is returned
    /// 5. Stop the mock Openid server
    #[test]
    fn get_subject_bad_response_body() {
        let (shutdown_handle, address) =
            run_mock_openid_server("get_subject", bad_response_body_user_info_endpoint);
        let subject_provider =
            OpenIdSubjectProvider::new(format!("{}{}", address, USER_INFO_ENDPOINT));

        assert!(subject_provider.get_subject(ACCESS_TOKEN).is_err());

        shutdown_handle.shutdown();
    }

    /// Verifies that the `OpenIdSubjectProvider` `get_subject` method returns an error if
    /// the `user_info` endpoint returns an unexpected response code.
    ///
    /// 1. Start the mock Openid server
    /// 2. Create a new OpenIdSubjectProvider with the address of the `user_info` endpoint
    ///    that returns an unexpected response code
    /// 3. Call `get_subject` with an valid access token
    /// 4. Verify that an error is returned
    /// 5. Stop the mock Openid server
    #[test]
    fn get_subject_bad_response_status() {
        let (shutdown_handle, address) =
            run_mock_openid_server("get_subject", bad_response_status_user_info_endpoint);
        let subject_provider =
            OpenIdSubjectProvider::new(format!("{}{}", address, USER_INFO_ENDPOINT));

        assert!(subject_provider.get_subject(ACCESS_TOKEN).is_err());

        shutdown_handle.shutdown();
    }

    /// Runs a mock Openid server to mimick an Openid OAuth provider. Recieves a test name
    /// and a function for handling requests to the user_info endpoint. Returns its
    /// shutdown handle along with the address the server is running on.
    fn run_mock_openid_server(
        test_name: &str,
        endpoint: fn(HttpRequest) -> HttpResponse,
    ) -> (OpenidServerShutdownHandle, String) {
        let (tx, rx) = channel();

        let instance_name = format!("Openid-Server-{}", test_name);
        let join_handle = std::thread::Builder::new()
            .name(instance_name.clone())
            .spawn(move || {
                let sys = System::new(instance_name);
                let server = HttpServer::new(move || {
                    App::new().service(web::resource(USER_INFO_ENDPOINT).to(endpoint))
                })
                .bind("127.0.0.1:0")
                .expect("Failed to bind Openid server");
                let address = format!("http://127.0.0.1:{}", server.addrs()[0].port());
                let server = server.disable_signals().system_exit().start();
                tx.send((server, address)).expect("Failed to send server");
                sys.run().expect("Openid server runtime failed");
            })
            .expect("Failed to spawn Openid server thread");

        let (server, address) = rx.recv().expect("Failed to receive server");

        (OpenidServerShutdownHandle(server, join_handle), address)
    }

    /// A handler for the Openid server's user_info endpoint. If the request received by this endpoint
    /// has an authorization header containing `ACCESS_TOKEN` a json object with user info including a `sub`
    /// field is returned in the http response. If the access token in the authorization header is invalid
    /// an "unauthorized" http response is returned.
    fn user_info_endpoint(request: HttpRequest) -> HttpResponse {
        match request.headers().get("Authorization") {
            Some(auth_header) => {
                let access_token = auth_header
                    .to_str()
                    .expect("Unable to get authorization header value");
                if access_token == format!("Bearer {}", ACCESS_TOKEN) {
                    HttpResponse::Ok()
                        .content_type("application/json")
                        .json(json!({
                                "sub": SUBJECT_IDENTIFIER,
                                "name": "Bob",
                                "given_name": "Bob",
                                "picture" : "https://graph.microsoft.com/v1.0/me/photo/$value",
                        }))
                } else {
                    HttpResponse::Unauthorized().finish()
                }
            }
            None => panic!("Invalid request, missing authorization header"),
        }
    }

    /// A handler for the Openid server's user_info endpoint. This handler simulates a successful http
    /// response with an unexpected response body from an Openid OAuth provider's user_info endpoint.
    /// This handler returns a json object with user info but no `sub` field if the recieved request
    /// has an authorization header with a valid access token.
    fn bad_response_body_user_info_endpoint(request: HttpRequest) -> HttpResponse {
        match request.headers().get("Authorization") {
            Some(auth_header) => {
                let access_token = auth_header
                    .to_str()
                    .expect("Unable to get authorization header value");
                if access_token == format!("Bearer {}", ACCESS_TOKEN) {
                    HttpResponse::Ok()
                        .content_type("application/json")
                        .json(json!({
                                "name": "Bob",
                                "given_name": "Bob",
                                "picture" : "https://graph.microsoft.com/v1.0/me/photo/$value",
                        }))
                } else {
                    HttpResponse::Unauthorized().finish()
                }
            }
            None => panic!("Invalid request, missing authorization header"),
        }
    }

    /// A handler for the Openid server's user_info endpoint. This handler simulates an http response
    /// with an unexpected response status from the Openid OAuth provider's user_info endpoint.
    fn bad_response_status_user_info_endpoint(_request: HttpRequest) -> HttpResponse {
        HttpResponse::NotAcceptable().finish()
    }

    struct OpenidServerShutdownHandle(Server, JoinHandle<()>);

    impl OpenidServerShutdownHandle {
        pub fn shutdown(self) {
            self.0
                .stop(false)
                .wait()
                .expect("Failed to stop Openid server");
            self.1.join().expect("Openid server thread failed");
        }
    }
}
