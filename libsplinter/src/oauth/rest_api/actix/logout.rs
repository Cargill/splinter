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

//! The `GET /oauth/logout` endpoint for removing a user's tokens.

use actix_web::{HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};

use crate::biome::oauth::store::{OAuthUserSessionStore, OAuthUserSessionStoreError};
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    auth::{AuthorizationHeader, BearerToken},
    ErrorResponse,
};

pub fn make_logout_route(oauth_user_session_store: Box<dyn OAuthUserSessionStore>) -> Resource {
    Resource::build("/oauth/logout")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::OAUTH_LOGOUT_MIN,
            protocol::OAUTH_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |req, _| {
            let access_token = match get_access_token(req) {
                Ok(access_token) => access_token,
                Err(err_response) => return err_response,
            };

            Box::new(
                match oauth_user_session_store.remove_session(&access_token) {
                    // `InvalidState` means there's no session for this token; we return `200 Ok`
                    // here because session removal is idempotent.
                    Ok(()) | Err(OAuthUserSessionStoreError::InvalidState(_)) => HttpResponse::Ok()
                        .json(json!({
                            "message": "User successfully logged out"
                        }))
                        .into_future(),
                    Err(err) => {
                        error!("Unable to remove user session: {}", err);
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future()
                    }
                },
            )
        })
}

fn get_access_token(
    req: HttpRequest,
) -> Result<String, Box<dyn Future<Item = HttpResponse, Error = actix_web::Error>>> {
    let auth_header = match req
        .headers()
        .get("Authorization")
        .map(|auth| auth.to_str())
        .transpose()
    {
        Ok(Some(header_str)) => header_str,
        Ok(None) => {
            return Err(Box::new(
                HttpResponse::Unauthorized()
                    .json(ErrorResponse::unauthorized())
                    .into_future(),
            ))
        }
        Err(_) => {
            return Err(Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request(
                        "Authorization header must contain only visible ASCII characters",
                    ))
                    .into_future(),
            ))
        }
    };

    match auth_header.parse() {
        Ok(AuthorizationHeader::Bearer(BearerToken::OAuth2(access_token))) => Ok(access_token),
        Ok(_) | Err(_) => Err(Box::new(
            HttpResponse::Unauthorized()
                .json(ErrorResponse::unauthorized())
                .into_future(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};

    use crate::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use crate::biome::MemoryOAuthUserSessionStore;
    use crate::rest_api::actix_web_1::{RestApiBuilder, RestApiShutdownHandle};

    const SPLINTER_ACCESS_TOKEN: &str = "splinter_access_token";

    /// Verifies the correct functionality of the `GET /oauth/logout` endpoint when the provided
    /// token matches an existing session
    ///
    /// 1. Create a new OAuthUserSessionStore and pre-populate it with a session
    /// 2. Run the Splinter REST API on an open port with the `GET /oauth/logout` endpoint backed by
    ///    the session store
    /// 3. Make the `GET /oauth/logout` request with the access token for the pre-populated session
    /// 4. Verify the response has status `200 Ok`
    /// 5. Verify the session is no longer in the session store
    /// 6. Shutdown the REST API
    #[test]
    fn get_logout_existing_session() {
        let session_store = MemoryOAuthUserSessionStore::new();
        session_store
            .add_session(
                InsertableOAuthUserSessionBuilder::new()
                    .with_splinter_access_token(SPLINTER_ACCESS_TOKEN.into())
                    .with_subject("subject".into())
                    .with_oauth_access_token("oauth_access_token".into())
                    .build()
                    .expect("Failed to build session"),
            )
            .expect("Failed to add session");

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_logout_route(session_store.clone_box())]);

        let url =
            Url::parse(&format!("http://{}/oauth/logout", bind_url)).expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .header(
                "Authorization",
                format!("Bearer OAuth2:{}", SPLINTER_ACCESS_TOKEN),
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        assert!(session_store
            .get_session(SPLINTER_ACCESS_TOKEN)
            .expect("Failed to check session")
            .is_none());

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verifies the correct functionality of the `GET /oauth/logout` endpoint when there is no
    /// session for the provided token
    ///
    /// 1. Create a new, empty OAuthUserSessionStore
    /// 2. Run the Splinter REST API on an open port with the `GET /oauth/logout` endpoint backed by
    ///    the empty session store
    /// 3. Make the `GET /oauth/logout` request with an access token
    /// 4. Verify the response has status `200 Ok`
    /// 5. Shutdown the REST API
    #[test]
    fn get_logout_non_existent_session() {
        let session_store = MemoryOAuthUserSessionStore::new();

        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_logout_route(session_store.clone_box())]);

        let url =
            Url::parse(&format!("http://{}/oauth/logout", bind_url)).expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::OAUTH_PROTOCOL_VERSION)
            .header(
                "Authorization",
                format!("Bearer OAuth2:{}", SPLINTER_ACCESS_TOKEN),
            )
            .send()
            .expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
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
