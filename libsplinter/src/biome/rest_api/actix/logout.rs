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

use std::sync::Arc;

use crate::actix_web::HttpResponse;
use crate::biome::refresh_tokens::store::{RefreshTokenError, RefreshTokenStore};
use crate::biome::rest_api::{
    actix::authorize::authorize_user, config::BiomeRestConfig,
    resources::authorize::AuthorizationResult,
};
use crate::futures::IntoFuture;
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{HandlerFunction, Method, ProtocolVersionRangeGuard, Resource},
    secrets::SecretManager,
    sessions::default_validation,
    ErrorResponse,
};

/// Defines a REST endpoint to remove any refresh tokens belonging to the user.
///
pub fn make_logout_route(
    refresh_token_store: Arc<dyn RefreshTokenStore>,
    secret_manager: Arc<dyn SecretManager>,
    rest_config: Arc<BiomeRestConfig>,
) -> Resource {
    Resource::build("/biome/logout")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_LOGIN_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Patch,
            add_logout_route(refresh_token_store, secret_manager, rest_config),
        )
}

pub fn add_logout_route(
    refresh_token_store: Arc<dyn RefreshTokenStore>,
    secret_manager: Arc<dyn SecretManager>,
    rest_config: Arc<BiomeRestConfig>,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let rest_config = rest_config.clone();
        let secret_manager = secret_manager.clone();
        let refresh_token_store = refresh_token_store.clone();
        let validation = default_validation(&rest_config.issuer());
        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
            AuthorizationResult::Failed => {
                return Box::new(
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future(),
                );
            }
        };

        Box::new(match refresh_token_store.remove_token(&user_id) {
            Ok(()) => HttpResponse::Ok()
                .json(json!({
                    "message": "User successfully logged out"
                }))
                .into_future(),
            Err(err) => match err {
                RefreshTokenError::NotFoundError(_) => HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request(&format!(
                        "User not found: {}",
                        &user_id
                    )))
                    .into_future(),
                _ => {
                    error!("Failed to remove refresh token: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            },
        })
    })
}
