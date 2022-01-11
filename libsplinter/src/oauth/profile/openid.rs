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

//! A profile provider that looks up OpenID profile information

use base64::encode;
use reqwest::{blocking::Client, StatusCode};
use serde::Deserialize;

use crate::error::InternalError;
use crate::oauth::Profile;

use super::ProfileProvider;

#[derive(Clone)]
pub struct OpenIdProfileProvider {
    userinfo_endpoint: String,
}

impl OpenIdProfileProvider {
    pub fn new(userinfo_endpoint: String) -> OpenIdProfileProvider {
        OpenIdProfileProvider { userinfo_endpoint }
    }
}

impl ProfileProvider for OpenIdProfileProvider {
    fn get_profile(&self, access_token: &str) -> Result<Option<Profile>, InternalError> {
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

        let mut user_profile = response
            .json::<OpenIdProfileResponse>()
            .map_err(|_| InternalError::with_message("Received unexpected response body".into()))?;

        // If azure openid is being used for authentication make a call to the
        // microsoft graph api endpoint with the access token to retrieve the
        // binary data for the authenticated user's profile photo
        if self.userinfo_endpoint.contains("graph.microsoft.com") {
            let picture_response = match Client::builder()
                .build()
                .map_err(|err| InternalError::from_source(err.into()))?
                .get("https://graph.microsoft.com/beta/me/photo/$value")
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
            {
                Ok(res) => {
                    if res.status().is_success() {
                        match res.bytes() {
                            Ok(image_data) => Some(encode(image_data.to_vec())),
                            Err(_) => {
                                warn!("Failed to get bytes from microsoft graph HTTP response");
                                Some("".into())
                            }
                        }
                    } else {
                        warn!("Microsoft graph API request failed");
                        Some("".into())
                    }
                }
                Err(_) => {
                    warn!("Failed to get user profile picture from microsoft graph API");
                    Some("".into())
                }
            };
            user_profile.picture = picture_response;
        }
        Ok(Some(Profile::from(user_profile)))
    }

    fn clone_box(&self) -> Box<dyn ProfileProvider> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Deserialize)]
pub struct OpenIdProfileResponse {
    pub sub: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
}

impl From<OpenIdProfileResponse> for Profile {
    fn from(openid_profile: OpenIdProfileResponse) -> Self {
        Profile {
            subject: openid_profile.sub,
            name: openid_profile.name,
            given_name: openid_profile.given_name,
            family_name: openid_profile.family_name,
            email: openid_profile.email,
            picture: openid_profile.picture,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc::channel;
    use std::thread::JoinHandle;

    use actix::System;
    use actix_web::{dev::Server, web, App, HttpRequest, HttpResponse, HttpServer};
    use futures::Future;

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
}
