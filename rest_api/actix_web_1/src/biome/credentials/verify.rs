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
use splinter_rest_api_common::{
    response_models::ErrorResponse, secrets::SecretManager, sessions::default_validation,
    SPLINTER_PROTOCOL_VERSION,
};

use crate::biome::credentials::resources::authorize::AuthorizationResult;
use crate::framework::{into_bytes, Method, ProtocolVersionRangeGuard, Resource};
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;

use crate::biome::credentials::BiomeCredentialsRestConfig;
use splinter::biome::credentials::store::{CredentialsStore, CredentialsStoreError};

use super::authorize::authorize_user;
use crate::biome::credentials::UsernamePassword;

const BIOME_VERIFY_PROTOCOL_MIN: u32 = 1;

/// Defines a REST endpoint to verify a user's password
///
/// The payload should be in the JSON format:
///   {
///       "username": <existing username of the user>
///       "hashed_password": <hash of the user's existing password>
///   }
pub fn make_verify_route(
    credentials_store: Arc<dyn CredentialsStore>,
    rest_config: Arc<BiomeCredentialsRestConfig>,
    secret_manager: Arc<dyn SecretManager>,
) -> Resource {
    let resource = Resource::build("/biome/verify").add_request_guard(
        ProtocolVersionRangeGuard::new(BIOME_VERIFY_PROTOCOL_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(
            Method::Post,
            Permission::AllowAuthenticated,
            move |request, payload| {
                let credentials_store = credentials_store.clone();
                let rest_config = rest_config.clone();
                let secret_manager = secret_manager.clone();
                Box::new(into_bytes(payload).and_then(move |bytes| {
                    let username_password = match serde_json::from_slice::<UsernamePassword>(&bytes)
                    {
                        Ok(val) => val,
                        Err(err) => {
                            debug!("Error parsing payload: {}", err);
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
                            debug!("Failed to fetch credentials: {}", err);
                            match err {
                                CredentialsStoreError::NotFoundError(_) => {
                                    return HttpResponse::BadRequest()
                                        .json(ErrorResponse::bad_request(&format!(
                                            "Username not found: {}",
                                            username_password.username
                                        )))
                                        .into_future()
                                }
                                _ => {
                                    error!("Failed to fetch credentials: {}", err);
                                    return HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future();
                                }
                            }
                        }
                    };

                    let validation = default_validation(&rest_config.issuer());
                    match authorize_user(&request, &secret_manager, &validation) {
                        AuthorizationResult::Authorized(_) => {
                            match credentials.verify_password(&username_password.hashed_password) {
                                Ok(true) => HttpResponse::Ok()
                                    .json(json!(
                                        {
                                            "message": "Successful verification",
                                            "user_id": credentials.user_id
                                    }))
                                    .into_future(),
                                Ok(false) => HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request("Invalid password"))
                                    .into_future(),
                                Err(err) => {
                                    error!("Failed to verify password: {}", err);
                                    HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())
                                        .into_future()
                                }
                            }
                        }
                        AuthorizationResult::Unauthorized => HttpResponse::Unauthorized()
                            .json(ErrorResponse::unauthorized())
                            .into_future(),
                        AuthorizationResult::Failed => {
                            error!("Failed to authorize user");
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
        resource.add_method(Method::Post, move |request, payload| {
            let credentials_store = credentials_store.clone();
            let rest_config = rest_config.clone();
            let secret_manager = secret_manager.clone();
            Box::new(into_bytes(payload).and_then(move |bytes| {
                let username_password = match serde_json::from_slice::<UsernamePassword>(&bytes) {
                    Ok(val) => val,
                    Err(err) => {
                        debug!("Error parsing payload: {}", err);
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
                        debug!("Failed to fetch credentials: {}", err);
                        match err {
                            CredentialsStoreError::NotFoundError(_) => {
                                return HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(&format!(
                                        "Username not found: {}",
                                        username_password.username
                                    )))
                                    .into_future()
                            }
                            _ => {
                                error!("Failed to fetch credentials: {}", err);
                                return HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_future();
                            }
                        }
                    }
                };

                let validation = default_validation(&rest_config.issuer());
                match authorize_user(&request, &secret_manager, &validation) {
                    AuthorizationResult::Authorized(_) => {
                        match credentials.verify_password(&username_password.hashed_password) {
                            Ok(true) => HttpResponse::Ok()
                                .json(json!(
                                    {
                                        "message": "Successful verification",
                                        "user_id": credentials.user_id
                                }))
                                .into_future(),
                            Ok(false) => HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid password"))
                                .into_future(),
                            Err(err) => {
                                error!("Failed to verify password: {}", err);
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_future()
                            }
                        }
                    }
                    AuthorizationResult::Unauthorized => HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                    AuthorizationResult::Failed => {
                        error!("Failed to authorize user");
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future()
                    }
                }
            }))
        })
    }
}
