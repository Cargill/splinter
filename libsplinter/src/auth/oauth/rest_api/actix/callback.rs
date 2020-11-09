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

use crate::auth::oauth::{
    rest_api::{
        resources::callback::{CallbackQuery, CallbackResponse},
        SaveTokensOperation,
    },
    OAuthClient,
};
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

pub fn make_callback_route(
    client: OAuthClient,
    save_token_op: Box<dyn SaveTokensOperation>,
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
                            Ok(Some((user_tokens, redirect_url))) => {
                                if let Err(err) = save_token_op.save_tokens(&user_tokens) {
                                    error!("Unable to store user tokens: {}", err);
                                    HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                } else {
                                    // Adding the user tokens to the redirect URL, so the client may
                                    // access these values after a redirect
                                    let callback_response = CallbackResponse::from(&user_tokens);
                                    let mut redirect_url = format!(
                                        "{}?access_token={}",
                                        redirect_url, callback_response.access_token
                                    );
                                    if let Some(expiry) = callback_response.expires_in {
                                        redirect_url.push_str(&format!("&expires_in={}", expiry))
                                    };
                                    if let Some(refresh) = callback_response.refresh_token {
                                        redirect_url
                                            .push_str(&format!("&refresh_token={}", refresh))
                                    };
                                    HttpResponse::Found()
                                        .header(LOCATION, redirect_url)
                                        .finish()
                                }
                            }
                            Ok(None) => {
                                error!(
                                "Received OAuth callback request that does not correlate to an \
                                 open authorization request"
                                );
                                match req.headers().get("referer") {
                                    Some(referer) => HttpResponse::Found()
                                        .header(LOCATION, referer.clone())
                                        .finish(),
                                    None => {
                                        HttpResponse::BadRequest().json(ErrorResponse::bad_request(
                                            "No `redirect_url` supplied, no `referer` found",
                                        ))
                                    }
                                }
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
