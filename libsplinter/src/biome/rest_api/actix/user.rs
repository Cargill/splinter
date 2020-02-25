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

use super::super::resources::authorize::AuthorizationResult;
use super::super::resources::credentials::UsernamePassword;
use super::authorize::authorize_user;
use crate::actix_web::{web::Payload, Error, HttpRequest, HttpResponse};
use crate::biome::credentials::store::{
    diesel::SplinterCredentialsStore, CredentialsStore, CredentialsStoreError,
};
use crate::biome::rest_api::BiomeRestConfig;
use crate::biome::secrets::SecretManager;
use crate::biome::user::store::{
    diesel::SplinterUserStore, SplinterUser, UserStore, UserStoreError,
};
use crate::protocol;
use crate::rest_api::{into_bytes, ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

/// Defines a REST endpoint to list users from the db
pub fn make_list_route(credentials_store: Arc<SplinterCredentialsStore>) -> Resource {
    Resource::build("/biome/users")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_LIST_USERS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |_, _| {
            let credentials_store = credentials_store.clone();
            match credentials_store.get_usernames() {
                Ok(users) => Ok(HttpResponse::Ok().json(users)),
                Err(err) => {
                    debug!("Failed to get users from the database {}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            }
        })
}

/// Defines REST endpoints to modify, delete, or fetch a specific user
pub fn make_user_routes(
    rest_config: Arc<BiomeRestConfig>,
    secret_manager: Arc<dyn SecretManager>,
    credentials_store: Arc<SplinterCredentialsStore>,
    user_store: Arc<SplinterUserStore>,
) -> Resource {
    let credentials_store_modify = credentials_store.clone();
    let credentials_store_fetch = credentials_store;
    let user_store_modify = user_store.clone();
    let user_store_delete = user_store;
    Resource::build("/biome/users/{id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_USER_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Put, move |request, payload| {
            add_modify_user_method(
                request,
                payload,
                credentials_store_modify.clone(),
                user_store_modify.clone(),
            )
        })
        .add_method(Method::Get, move |request, _| {
            add_fetch_user_method(request, credentials_store_fetch.clone())
        })
        .add_method(Method::Delete, move |request, _| {
            add_delete_user_method(
                request,
                rest_config.clone(),
                secret_manager.clone(),
                user_store_delete.clone(),
            )
        })
}

/// Defines a REST endpoint to fetch a user from the database
/// returns the user's ID and username
fn add_fetch_user_method(
    request: HttpRequest,
    credentials_store: Arc<SplinterCredentialsStore>,
) -> Result<HttpResponse, Error> {
    let user_id = if let Some(t) = request.match_info().get("id") {
        t.to_string()
    } else {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(
            &"Failed to process request: no user id".to_string(),
        )));
    };
    match credentials_store.fetch_username_by_id(&user_id) {
        Ok(user) => Ok(HttpResponse::Ok().json(user)),
        Err(err) => {
            debug!("Failed to get user from the database {}", err);
            match err {
                CredentialsStoreError::NotFoundError(_) => Ok(HttpResponse::NotFound().json(
                    ErrorResponse::not_found(&format!("User ID not found: {}", &user_id)),
                )),
                _ => Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error())),
            }
        }
    }
}

/// Defines a REST endpoint to edit a user's credentials in the database
/// The payload should be in the JSON format:
///   {
///       "username": <existing username of the user>
///       "hashed_password": <hash of the user's existing password>
///       "new_password": OPTIONAL <hash of the user's updated password>
///   }
fn add_modify_user_method(
    request: HttpRequest,
    payload: Payload,
    credentials_store: Arc<SplinterCredentialsStore>,
    user_store: Arc<SplinterUserStore>,
) -> Result<HttpResponse, Error> {
    let user_id = if let Some(t) = request.match_info().get("id") {
        t.to_string()
    } else {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(
            &"Failed to parse payload: no user id".to_string(),
        )));
    };
    let bytes = block_on(async { into_bytes(payload).await })?;

    let body = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(val) => val,
        Err(err) => {
            debug!("Error parsing request body {}", err);
            return Ok(
                HttpResponse::BadRequest().json(ErrorResponse::bad_request(&format!(
                    "Failed to parse payload body: {}",
                    err
                ))),
            );
        }
    };
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
                        return Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(
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
    let splinter_user = SplinterUser::new(&user_id);
    match credentials.verify_password(&username_password.hashed_password) {
        Ok(is_valid) => {
            if is_valid {
                let new_password = match body.get("new_password") {
                    Some(val) => match val.as_str() {
                        Some(val) => val,
                        None => &username_password.hashed_password,
                    },
                    None => &username_password.hashed_password,
                };

                match user_store.update_user(splinter_user) {
                    Ok(()) => {
                        match credentials_store.update_credentials(
                            &user_id,
                            &username_password.username,
                            &new_password,
                        ) {
                            Ok(()) => Ok(HttpResponse::Ok()
                                .json(json!({ "message": "User updated successfully" }))),
                            Err(err) => {
                                debug!("Failed to update credentials in database {}", err);
                                match err {
                                    CredentialsStoreError::DuplicateError(err) => {
                                        Ok(HttpResponse::BadRequest().json(
                                            ErrorResponse::bad_request(&format!(
                                                "Failed to update user: {}",
                                                err
                                            )),
                                        ))
                                    }
                                    _ => Ok(HttpResponse::InternalServerError()
                                        .json(ErrorResponse::internal_error())),
                                }
                            }
                        }
                    }
                    Err(err) => {
                        debug!("Failed to update user in database {}", err);
                        Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()))
                    }
                }
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

/// Defines a REST endpoint to delete a user from the database
fn add_delete_user_method(
    request: HttpRequest,
    rest_config: Arc<BiomeRestConfig>,
    secret_manager: Arc<dyn SecretManager>,
    user_store: Arc<SplinterUserStore>,
) -> Result<HttpResponse, Error> {
    let user_id = if let Some(t) = request.match_info().get("id") {
        t.to_string()
    } else {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(
            &"Failed to parse payload: no user id".to_string(),
        )));
    };
    match authorize_user(&request, &user_id, &secret_manager, &rest_config) {
        AuthorizationResult::Authorized => match user_store.remove_user(&user_id) {
            Ok(()) => Ok(HttpResponse::Ok().json(json!({ "message": "User deleted sucessfully" }))),
            Err(err) => match err {
                UserStoreError::NotFoundError(msg) => {
                    debug!("User not found: {}", msg);
                    Ok(
                        HttpResponse::NotFound().json(ErrorResponse::not_found(&format!(
                            "User ID not found: {}",
                            &user_id
                        ))),
                    )
                }
                _ => {
                    error!("Failed to delete user in database {}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            },
        },
        AuthorizationResult::Unauthorized(msg) => {
            Ok(HttpResponse::Unauthorized().json(ErrorResponse::unauthorized(&msg)))
        }
        AuthorizationResult::Failed => {
            error!("Failed to authorize user");
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}
