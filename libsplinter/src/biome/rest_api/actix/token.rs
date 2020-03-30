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

use crate::actix_web::HttpResponse;
use crate::biome::{
    refresh_tokens::store::{RefreshTokenError, RefreshTokenStore},
    rest_api::{
        actix::authorize::{authorize_user, validate_claims},
        config::BiomeRestConfig,
        resources::{authorize::AuthorizationResult, token::RefreshToken},
    },
};
use crate::futures::{Future, IntoFuture};
use crate::protocol;
use crate::rest_api::secrets::SecretManager;
use crate::rest_api::{
    into_bytes,
    sessions::{
        default_validation, ignore_exp_validation, AccessTokenIssuer, ClaimsBuilder, TokenIssuer,
    },
    ErrorResponse, Method, ProtocolVersionRangeGuard, Resource,
};

/// Defines a REST endpoint for request a new authorization token
///
/// The payload should be in the JSON format:
///   {
///       "refresh_token": <refresh token for requesting a auth token>
///   }
///
/// Endpoint returns a payload containing a new auth token
///   {
///     "token": <new auth token>
///   }
pub fn make_token_route<
    R: RefreshTokenStore + Clone + 'static,
    SM: SecretManager + Clone + 'static,
>(
    refresh_token_store: R,
    secret_manager: SM,
    refresh_token_secret_manager: SM,
    token_issuer: AccessTokenIssuer<SM>,
    rest_config: BiomeRestConfig,
) -> Resource {
    Resource::build("/biome/token")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_LOGIN_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Post, move |req, payload| {
            let validation = ignore_exp_validation(&rest_config.issuer());
            let refresh_token_validation = default_validation(&rest_config.issuer());
            let secret_manager = secret_manager.clone();
            let refresh_token_secret_manager = refresh_token_secret_manager.clone();
            let mut refresh_token_store = refresh_token_store.clone();
            let token_issuer = token_issuer.clone();
            let rest_config = rest_config.clone();
            Box::new(into_bytes(payload).and_then(move |bytes| {
                let claims = match authorize_user(&req, &secret_manager, &validation) {
                    AuthorizationResult::Authorized(claims) => claims,
                    AuthorizationResult::Unauthorized(msg) => {
                        return HttpResponse::Unauthorized()
                            .json(ErrorResponse::unauthorized(&msg))
                            .into_future();
                    }
                    AuthorizationResult::Failed => {
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                };

                let refresh_token = match serde_json::from_slice::<RefreshToken>(&bytes) {
                    Ok(refresh_token) => refresh_token.token,
                    Err(err) => {
                        error!("Malformed payload {}", err);
                        return HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(&format!(
                                "Malformed payload {}",
                                err
                            )))
                            .into_future();
                    }
                };

                let refresh_token_from_db = match refresh_token_store.fetch_token(&claims.user_id())
                {
                    Ok(token) => token,
                    Err(RefreshTokenError::NotFoundError(msg)) => {
                        return HttpResponse::Forbidden()
                            .json(ErrorResponse::forbidden(&msg))
                            .into_future();
                    }
                    Err(err) => {
                        error!("Failed to retrieve user refresh token {}", err);
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                };

                if refresh_token != refresh_token_from_db {
                    return HttpResponse::Forbidden()
                        .json(ErrorResponse::forbidden("Invalid Refresh Token"))
                        .into_future();
                }

                match validate_claims(
                    &refresh_token,
                    &refresh_token_secret_manager,
                    &refresh_token_validation,
                ) {
                    AuthorizationResult::Authorized(_) => (),
                    AuthorizationResult::Unauthorized(msg) => {
                        if let Err(err) = refresh_token_store.remove_token(&claims.user_id()) {
                            error!("Failed to delete refresh token {}", err);
                            return HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future();
                        } else {
                            return HttpResponse::Unauthorized()
                                .json(ErrorResponse::unauthorized(&msg))
                                .into_future();
                        }
                    }
                    AuthorizationResult::Failed => {
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                }
                let claim_builder = ClaimsBuilder::default();
                let claim = match claim_builder
                    .with_user_id(&claims.user_id())
                    .with_issuer(&rest_config.issuer())
                    .with_duration(rest_config.access_token_duration())
                    .build()
                {
                    Ok(claim) => claim,
                    Err(err) => {
                        error!("Failed to build claim {}", err);
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                };

                let token = match token_issuer.issue_token_with_claims(claim) {
                    Ok(token) => token,
                    Err(err) => {
                        error!("Failed to issue token {}", err);
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                };

                HttpResponse::Ok()
                    .json(json!({ "token": token }))
                    .into_future()
            }))
        })
}
