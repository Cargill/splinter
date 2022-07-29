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

use std::sync::Arc;

use actix_web::HttpResponse;
use futures::{Future, IntoFuture};
use splinter::biome::credentials::store::{CredentialsStore, CredentialsStoreError};
use splinter::biome::refresh_tokens::store::RefreshTokenStore;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;
use splinter_rest_api_common::sessions::{AccessTokenIssuer, ClaimsBuilder, TokenIssuer};
use splinter_rest_api_common::{response_models::ErrorResponse, SPLINTER_PROTOCOL_VERSION};

use crate::framework::{into_bytes, Method, ProtocolVersionRangeGuard, Resource};

use crate::biome::credentials::resources::credentials::UsernamePassword;
use crate::biome::credentials::BiomeCredentialsRestConfig;

const BIOME_LOGIN_PROTOCOL_MIN: u32 = 1;

/// Defines a REST endpoint for login
///
/// The payload should be in the JSON format:
///   {
///       "username": <existing username of the user>
///       "hashed_password": <hash of the user's existing password>
///   }
pub fn make_login_route(
    credentials_store: Arc<dyn CredentialsStore>,
    refresh_token_store: Arc<dyn RefreshTokenStore>,
    rest_config: Arc<BiomeCredentialsRestConfig>,
    token_issuer: Arc<AccessTokenIssuer>,
) -> Resource {
    let resource = Resource::build("/biome/login").add_request_guard(
        ProtocolVersionRangeGuard::new(BIOME_LOGIN_PROTOCOL_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(
            Method::Post,
            Permission::AllowUnauthenticated,
            move |_, payload| {
                let credentials_store = credentials_store.clone();
                let rest_config = rest_config.clone();
                let token_issuer = token_issuer.clone();
                let refresh_token_store = refresh_token_store.clone();
                Box::new(into_bytes(payload).and_then(move |bytes| {
                    let username_password = match serde_json::from_slice::<UsernamePassword>(&bytes)
                    {
                        Ok(val) => val,
                        Err(err) => {
                            debug!("Error parsing payload {}", err);
                            return HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request(&format!(
                                    "Failed to parse payload: {}",
                                    err
                                )))
                                .into_future();
                        }
                    };

                    let credentials = match credentials_store
                        .fetch_credential_by_username(&username_password.username)
                    {
                        Ok(credentials) => credentials,
                        Err(err) => {
                            debug!("Failed to fetch credentials {}", err);
                            match err {
                                CredentialsStoreError::NotFoundError(_) => {
                                    return HttpResponse::BadRequest()
                                        .json(ErrorResponse::bad_request(&format!(
                                            "Username not found: {}",
                                            username_password.username
                                        )))
                                        .into_future();
                                }
                                _ => {
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future()
                                }
                            }
                        }
                    };

                    match credentials.verify_password(&username_password.hashed_password) {
                        Ok(is_valid) => {
                            if is_valid {
                                let claim_builder = ClaimsBuilder::default();
                                let claim = match claim_builder
                                    .with_user_id(&credentials.user_id)
                                    .with_issuer(&rest_config.issuer())
                                    .with_duration(rest_config.access_token_duration())
                                    .build()
                                {
                                    Ok(claim) => claim,
                                    Err(err) => {
                                        debug!("Failed to build claim {}", err);
                                        return HttpResponse::InternalServerError()
                                            .json(ErrorResponse::internal_error())
                                            .into_future();
                                    }
                                };

                                let token = match token_issuer.issue_token_with_claims(claim) {
                                    Ok(token) => format!("Biome:{}", token),
                                    Err(err) => {
                                        debug!("Failed to issue token {}", err);
                                        return HttpResponse::InternalServerError()
                                            .json(ErrorResponse::internal_error())
                                            .into_future();
                                    }
                                };

                                let refresh_claims = match ClaimsBuilder::default()
                                    .with_user_id(&credentials.user_id)
                                    .with_issuer(&rest_config.issuer())
                                    .with_duration(rest_config.refresh_token_duration())
                                    .build()
                                {
                                    Ok(claims) => claims,
                                    Err(err) => {
                                        debug!("Failed to build refresh claim {}", err);
                                        return HttpResponse::InternalServerError()
                                            .json(ErrorResponse::internal_error())
                                            .into_future();
                                    }
                                };

                                let refresh_token = match token_issuer
                                    .issue_refresh_token_with_claims(refresh_claims)
                                {
                                    Ok(token) => token,
                                    Err(err) => {
                                        debug!("Failed to issue refresh token {}", err);
                                        return HttpResponse::InternalServerError()
                                            .json(ErrorResponse::internal_error())
                                            .into_future();
                                    }
                                };

                                if let Err(err) = refresh_token_store
                                    .add_token(&credentials.user_id, &refresh_token)
                                {
                                    debug!("Failed to store refresh token {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }

                                HttpResponse::Ok()
                                    .json(json!({
                                        "message": "Successful login",
                                        "user_id": credentials.user_id,
                                        "token": token,
                                        "refresh_token": refresh_token,
                                    }))
                                    .into_future()
                            } else {
                                HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request("Invalid password"))
                                    .into_future()
                            }
                        }
                        Err(err) => {
                            debug!("Failed to verify password {}", err);
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future()
                        }
                    }
                }))
            },
        )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Post, move |_, payload| {
            let credentials_store = credentials_store.clone();
            let rest_config = rest_config.clone();
            let token_issuer = token_issuer.clone();
            let refresh_token_store = refresh_token_store.clone();
            Box::new(into_bytes(payload).and_then(move |bytes| {
                let username_password = match serde_json::from_slice::<UsernamePassword>(&bytes) {
                    Ok(val) => val,
                    Err(err) => {
                        debug!("Error parsing payload {}", err);
                        return HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(&format!(
                                "Failed to parse payload: {}",
                                err
                            )))
                            .into_future();
                    }
                };

                let credentials = match credentials_store
                    .fetch_credential_by_username(&username_password.username)
                {
                    Ok(credentials) => credentials,
                    Err(err) => {
                        debug!("Failed to fetch credentials {}", err);
                        match err {
                            CredentialsStoreError::NotFoundError(_) => {
                                return HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(&format!(
                                        "Username not found: {}",
                                        username_password.username
                                    )))
                                    .into_future();
                            }
                            _ => {
                                return HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_future()
                            }
                        }
                    }
                };

                match credentials.verify_password(&username_password.hashed_password) {
                    Ok(is_valid) => {
                        if is_valid {
                            let claim_builder = ClaimsBuilder::default();
                            let claim = match claim_builder
                                .with_user_id(&credentials.user_id)
                                .with_issuer(&rest_config.issuer())
                                .with_duration(rest_config.access_token_duration())
                                .build()
                            {
                                Ok(claim) => claim,
                                Err(err) => {
                                    debug!("Failed to build claim {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }
                            };

                            let token = match token_issuer.issue_token_with_claims(claim) {
                                Ok(token) => format!("Biome:{}", token),
                                Err(err) => {
                                    debug!("Failed to issue token {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }
                            };

                            let refresh_claims = match ClaimsBuilder::default()
                                .with_user_id(&credentials.user_id)
                                .with_issuer(&rest_config.issuer())
                                .with_duration(rest_config.refresh_token_duration())
                                .build()
                            {
                                Ok(claims) => claims,
                                Err(err) => {
                                    debug!("Failed to build refresh claim {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }
                            };

                            let refresh_token = match token_issuer
                                .issue_refresh_token_with_claims(refresh_claims)
                            {
                                Ok(token) => token,
                                Err(err) => {
                                    debug!("Failed to issue refresh token {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }
                            };

                            if let Err(err) =
                                refresh_token_store.add_token(&credentials.user_id, &refresh_token)
                            {
                                debug!("Failed to store refresh token {}", err);
                                return HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_future();
                            }

                            HttpResponse::Ok()
                                .json(json!({
                                    "message": "Successful login",
                                    "user_id": credentials.user_id,
                                    "token": token,
                                    "refresh_token": refresh_token,
                                }))
                                .into_future()
                        } else {
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid password"))
                                .into_future()
                        }
                    }
                    Err(err) => {
                        debug!("Failed to verify password {}", err);
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future()
                    }
                }
            }))
        })
    }
}
