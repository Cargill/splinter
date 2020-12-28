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
