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

use futures::executor::block_on;
use std::sync::Arc;

use crate::actix_web::{web::Payload, Error, HttpResponse};
use crate::protocol;
use crate::rest_api::{into_bytes, ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

use crate::biome::credentials::store::{
    diesel::SplinterCredentialsStore, CredentialsStore, CredentialsStoreError,
};
use crate::biome::rest_api::BiomeRestConfig;
use crate::biome::sessions::{AccessTokenIssuer, ClaimsBuilder, TokenIssuer};

use super::super::resources::credentials::UsernamePassword;

/// Defines a REST endpoint for login
pub fn make_login_route(
    credentials_store: Arc<SplinterCredentialsStore>,
    rest_config: Arc<BiomeRestConfig>,
    token_issuer: Arc<AccessTokenIssuer>,
) -> Resource {
    Resource::build("/biome/login")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_LOGIN_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Post, move |_, payload| {
            let credentials_store = credentials_store.clone();
            let rest_config = rest_config.clone();
            let token_issuer = token_issuer.clone();
            add_login_method(credentials_store, rest_config, token_issuer, payload)
        })
}

fn add_login_method(
    credentials_store: Arc<SplinterCredentialsStore>,
    rest_config: Arc<BiomeRestConfig>,
    token_issuer: Arc<AccessTokenIssuer>,
    payload: Payload,
) -> Result<HttpResponse, Error> {
    let bytes = block_on(async { into_bytes(payload).await })?;

    let username_password = match serde_json::from_slice::<UsernamePassword>(&bytes) {
        Ok(val) => val,
        Err(err) => {
            debug!("Error parsing payload {}", err);
            return Ok(
                HttpResponse::BadRequest().json(ErrorResponse::bad_request(&format!(
                    "Failed to parse payload: {}",
                    err
                ))),
            );
        }
    };

    let credentials =
        match credentials_store.fetch_credential_by_username(&username_password.username) {
            Ok(credentials) => credentials,
            Err(err) => {
                debug!("Failed to fetch credentials {}", err);
                match err {
                    CredentialsStoreError::NotFoundError(_) => {
                        return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(
                            &format!("Username not found: {}", username_password.username),
                        )));
                    }
                    _ => {
                        return Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()))
                    }
                }
            }
        };

    match credentials.verify_password(&username_password.hashed_password) {
        Ok(is_valid) => {
            if is_valid {
                let claim_builder: ClaimsBuilder = Default::default();
                let claim = match claim_builder
                    .with_user_id(&credentials.user_id)
                    .with_issuer(&rest_config.issuer())
                    .with_duration(rest_config.access_token_duration())
                    .build()
                {
                    Ok(claim) => claim,
                    Err(err) => {
                        debug!("Failed to build claim {}", err);
                        return Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()));
                    }
                };

                let token = match token_issuer.issue_token_with_claims(claim) {
                    Ok(token) => token,
                    Err(err) => {
                        debug!("Failed to issue token {}", err);
                        return Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()));
                    }
                };
                Ok(
                    HttpResponse::Ok().json(json!({ "message": "Successful login",
                                      "user_id": credentials.user_id ,
                                      "token": token  })),
                )
            } else {
                Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request("Invalid password")))
            }
        }
        Err(err) => {
            debug!("Failed to verify password {}", err);
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}
