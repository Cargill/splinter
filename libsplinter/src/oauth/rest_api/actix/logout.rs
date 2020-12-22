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

use crate::biome::oauth::store::OAuthUserSessionStore;
use crate::protocol;
use crate::rest_api::auth::{AuthorizationHeader, BearerToken};
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

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
                    Ok(()) => HttpResponse::Ok()
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
