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

use crate::actix_web::{web::Payload, Error as ActixError, HttpRequest, HttpResponse};
use crate::futures::executor::block_on;
use crate::protocol;
use crate::rest_api::{into_bytes, ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

use crate::biome::key_management::{
    store::{KeyStore, KeyStoreError},
    Key,
};
use crate::biome::rest_api::BiomeRestConfig;
use crate::biome::secrets::SecretManager;

use super::super::resources::authorize::AuthorizationResult;
use super::super::resources::key_management::{NewKey, UpdatedKey};
use super::authorize::authorize_user;

/// Defines a REST endpoint for managing keys
pub fn make_key_management_route(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore<Key>>,
    secret_manager: Arc<dyn SecretManager>,
) -> Resource {
    Resource::build("/biome/users/{user_id}/keys")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_KEYS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Post, {
            let rest_config = rest_config.clone();
            let key_store = key_store.clone();
            let secret_manager = secret_manager.clone();
            move |request, payload| {
                handle_post(
                    rest_config.clone(),
                    key_store.clone(),
                    secret_manager.clone(),
                    request,
                    payload,
                )
            }
        })
        .add_method(Method::Get, {
            let rest_config = rest_config.clone();
            let key_store = key_store.clone();
            let secret_manager = secret_manager.clone();
            move |request, _| {
                handle_get(
                    rest_config.clone(),
                    key_store.clone(),
                    secret_manager.clone(),
                    request,
                )
            }
        })
        .add_method(Method::Patch, {
            let key_store = key_store.clone();
            let secret_manager = secret_manager.clone();

            move |request, payload| {
                handle_patch(
                    rest_config.clone(),
                    key_store.clone(),
                    secret_manager.clone(),
                    request,
                    payload,
                )
            }
        })
}

fn handle_post(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore<Key>>,
    secret_manager: Arc<dyn SecretManager>,
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, ActixError> {
    let user_id = match request.match_info().get("user_id") {
        Some(id) => id.to_owned(),
        None => {
            error!("User ID is not in path request");
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };

    match authorize_user(&request, &user_id, &secret_manager, &rest_config) {
        AuthorizationResult::Authorized => (),
        AuthorizationResult::Unauthorized(msg) => {
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::unauthorized(&msg)));
        }
        AuthorizationResult::Failed => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    }
    let bytes = block_on(async { into_bytes(payload).await })?;

    let new_key = match serde_json::from_slice::<NewKey>(&bytes) {
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
    let key = Key::new(
        &new_key.public_key,
        &new_key.encrypted_private_key,
        &user_id,
        &new_key.display_name,
    );

    match key_store.add_key(key) {
        Ok(()) => Ok(HttpResponse::Ok().json(json!({ "message": "Key added successfully" }))),
        Err(err) => {
            debug!("Failed to add new key to database {}", err);
            match err {
                KeyStoreError::DuplicateKeyError(msg) => {
                    Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&msg)))
                }
                KeyStoreError::UserDoesNotExistError(msg) => {
                    Ok(HttpResponse::BadRequest().json(ErrorResponse::bad_request(&msg)))
                }
                _ => Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error())),
            }
        }
    }
}

fn handle_get(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore<Key>>,
    secret_manager: Arc<dyn SecretManager>,
    request: HttpRequest,
) -> Result<HttpResponse, ActixError> {
    let user_id = match request.match_info().get("user_id") {
        Some(id) => id.to_owned(),
        None => {
            error!("User ID is not in path request");
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };

    match authorize_user(&request, &user_id, &secret_manager, &rest_config) {
        AuthorizationResult::Authorized => (),
        AuthorizationResult::Unauthorized(msg) => {
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::unauthorized(&msg)));
        }
        AuthorizationResult::Failed => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    }

    match key_store.list_keys(Some(&user_id)) {
        Ok(keys) => Ok(HttpResponse::Ok().json(json!({ "data": keys }))),
        Err(err) => {
            debug!("Failed to fetch keys {}", err);
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}

fn handle_patch(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore<Key>>,
    secret_manager: Arc<dyn SecretManager>,
    request: HttpRequest,
    payload: Payload,
) -> Result<HttpResponse, ActixError> {
    let user_id = match request.match_info().get("user_id") {
        Some(id) => id.to_owned(),
        None => {
            error!("User ID is not in path request");
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    };

    match authorize_user(&request, &user_id, &secret_manager, &rest_config) {
        AuthorizationResult::Authorized => (),
        AuthorizationResult::Unauthorized(msg) => {
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::unauthorized(&msg)));
        }
        AuthorizationResult::Failed => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()));
        }
    }

    let bytes = block_on(async { into_bytes(payload).await })?;
    let updated_key = match serde_json::from_slice::<UpdatedKey>(&bytes) {
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

    match key_store.update_key(
        &updated_key.public_key,
        &user_id,
        &updated_key.new_display_name,
    ) {
        Ok(()) => Ok(HttpResponse::Ok().json(json!({ "message": "Key updated successfully" }))),
        Err(err) => {
            debug!("Failed to update key {}", err);
            match err {
                KeyStoreError::NotFoundError(msg) => {
                    Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(&msg)))
                }
                _ => Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error())),
            }
        }
    }
}
