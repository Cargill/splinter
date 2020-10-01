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

//! Local callback server for OAuth redirects

use std::collections::HashMap;

use tiny_http::{Header, Method, Request, Response, Server};

use super::error::OAuth2CallbackError;

/// Local callback server for OAuth redirects
///
/// This module provides a local server for the OAuth2 redirect callback.  This call includes a
/// code that will be used to exchange tokens with an OAuth2 provider.  It is necessary when using
/// authorization code grants.
///
/// It includes a pre-rendered page on success.
pub struct OAuth2Callback {
    port: u16,
    join_handle: std::thread::JoinHandle<Result<(String, String), OAuth2CallbackError>>,
}

impl OAuth2Callback {
    /// Constructs a new OAuth2Callback
    ///
    /// # Errors
    ///
    /// Returns an error if the local callback server cannot be started.
    pub fn new() -> Result<Self, OAuth2CallbackError> {
        let server =
            Server::http("127.0.0.1:0").map_err(|err| OAuth2CallbackError(err.to_string()))?;

        let port = server.server_addr().port();

        let join_handle = std::thread::Builder::new()
            .name("Thread-OAuth2Callback".into())
            .spawn(move || Self::run_callback_server(server))
            .map_err(|err| OAuth2CallbackError(err.to_string()))?;

        Ok(Self { join_handle, port })
    }

    /// Return the callback URL where requests will be received.
    pub fn callback_url(&self) -> String {
        format!("http://localhost:{}/auth_response", self.port)
    }

    /// Return the authorization code and the CSRF state.
    ///
    /// This function blocks until a response has been received.
    pub fn recv(self) -> Result<(String, String), OAuth2CallbackError> {
        self.join_handle.join().map_err(|_| {
            OAuth2CallbackError("Unable to join authorization callback thread".into())
        })?
    }

    fn run_callback_server(server: Server) -> Result<(String, String), OAuth2CallbackError> {
        loop {
            let request = server
                .recv()
                .map_err(|err| OAuth2CallbackError(err.to_string()))?;

            if request.method() == &Method::Get && request.url().starts_with("/auth_response") {
                let query = Self::parse_query_str(request.url());
                let code = query
                    .get("code")
                    .and_then(|vals| vals.iter().next())
                    .map(|s| s.to_string());

                if let Some(code) = code {
                    let state = query
                        .get("state")
                        .and_then(|vals| vals.iter().next())
                        .map(|s| s.to_string())
                        .ok_or_else(|| {
                            OAuth2CallbackError(
                                "Provided a code, but the CSRF state was not provided.".into(),
                            )
                        })?;

                    Self::send_reponse(request)?;
                    break Ok((code, state));
                }
            } else {
                debug!("Ignoring request {}: {}", request.method(), request.url());
            }
        }
    }

    fn send_reponse(request: Request) -> Result<(), OAuth2CallbackError> {
        let response = Response::from_string(include_str!("oauth_success.html")).with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..])
                .expect("valid header could not be turned into a header struct"),
        );

        request
            .respond(response)
            .map_err(|err| OAuth2CallbackError(err.to_string()))
    }

    fn parse_query_str<'a>(uri: &'a str) -> HashMap<&'a str, Vec<&'a str>> {
        let mut result: HashMap<&'a str, Vec<&'a str>> = HashMap::new();

        let split = uri.split('?');
        if let Some(query) = split.last() {
            let params = query.split('&');
            for param in params {
                let mut pair = param.split('=');
                if let Some(key) = pair.next() {
                    let val = pair.next().unwrap_or("");
                    result
                        .entry(key)
                        .and_modify(|vals| vals.push(val))
                        .or_insert_with(|| vec![val]);
                }
            }
        }

        result
    }
}
