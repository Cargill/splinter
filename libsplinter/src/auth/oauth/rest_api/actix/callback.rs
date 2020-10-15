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

use actix_web::{web::Query, HttpResponse};
use futures::future::IntoFuture;

use crate::auth::oauth::{
    rest_api::resources::callback::{CallbackQuery, CallbackResponse},
    OAuthClient,
};
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

pub fn make_callback_route(client: OAuthClient) -> Resource {
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
                            Ok(Some(user_tokens)) => {
                                HttpResponse::Ok().json(CallbackResponse::from(&user_tokens))
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
