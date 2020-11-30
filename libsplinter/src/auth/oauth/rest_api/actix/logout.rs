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

use actix_web::HttpResponse;
use futures::future::IntoFuture;

use crate::auth::oauth::rest_api::OAuthUserInfoStore;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

use crate::biome::rest_api::auth::OAuthUserIdentityRef;

pub fn make_logout_route(user_info_store: Box<dyn OAuthUserInfoStore>) -> Resource {
    Resource::build("/oauth/logout")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::OAUTH_LOGOUT_MIN,
            protocol::OAUTH_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |req, _| {
            Box::new(match req.extensions().get::<OAuthUserIdentityRef>() {
                Some(OAuthUserIdentityRef(identity)) => {
                    match user_info_store.remove_user_tokens(&identity) {
                        Ok(()) => HttpResponse::Ok()
                            .json(json!({
                                "message": "User successfully logged out"
                            }))
                            .into_future(),
                        Err(err) => {
                            error!("Unable to remove user tokens: {}", err);
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future()
                        }
                    }
                }
                None => HttpResponse::Unauthorized()
                    .json(ErrorResponse::unauthorized())
                    .into_future(),
            })
        })
}
