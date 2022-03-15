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

use super::authorize::authorize_user;
use crate::actix_web::HttpResponse;
use crate::biome::key_management::{
    store::{KeyStore, KeyStoreError},
    Key,
};
use crate::biome::rest_api::resources::authorize::AuthorizationResult;
use crate::biome::rest_api::resources::key_management::{NewKey, ResponseKey, UpdatedKey};
use crate::biome::rest_api::BiomeRestConfig;
use crate::futures::{Future, IntoFuture};
use crate::protocol;
use crate::rest_api::{
    into_bytes, ErrorResponse, HandlerFunction, Method, ProtocolVersionRangeGuard, Resource,
};
use crate::rest_api::{secrets::SecretManager, sessions::default_validation};

/// Defines a REST endpoint for managing keys including inserting, listing and updating keys
pub fn make_key_management_route(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> Resource {
    Resource::build("/biome/keys")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_KEYS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Post,
            handle_post(
                rest_config.clone(),
                key_store.clone(),
                secret_manager.clone(),
            ),
        )
        .add_method(
            Method::Get,
            handle_get(
                rest_config.clone(),
                key_store.clone(),
                secret_manager.clone(),
            ),
        )
        .add_method(
            Method::Patch,
            handle_patch(rest_config, key_store, secret_manager),
        )
}

/// Defines a REST endpoint for adding a key to the underlying storage
fn handle_post(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> HandlerFunction {
    Box::new(move |request, payload| {
        let key_store = key_store.clone();
        let validation = default_validation(&rest_config.issuer());

        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized(msg) => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized(&msg))
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

        Box::new(into_bytes(payload).and_then(move |bytes| {
            let new_key = match serde_json::from_slice::<NewKey>(&bytes) {
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
            let key = Key::new(
                &new_key.public_key,
                &new_key.encrypted_private_key,
                &user_id,
                &new_key.display_name,
            );
            let response_key = ResponseKey::from(&key);

            match key_store.add_key(key.clone()) {
                Ok(()) => HttpResponse::Ok()
                    .json(json!({ "message": "Key added successfully", "data": response_key }))
                    .into_future(),
                Err(err) => {
                    debug!("Failed to add new key to database {}", err);
                    match err {
                        KeyStoreError::DuplicateKeyError(msg) => HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(&msg))
                            .into_future(),
                        KeyStoreError::UserDoesNotExistError(msg) => HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(&msg))
                            .into_future(),
                        _ => HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    }
                }
            }
        }))
    })
}

/// Defines a REST endpoint for retrieving keys from the underlying storage
fn handle_get(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();
        let validation = default_validation(&rest_config.issuer());

        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized(msg) => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized(&msg))
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

        match key_store.list_keys(Some(&user_id)) {
            Ok(keys) => Box::new(
                HttpResponse::Ok()
                    .json(json!(
                        {
                            "data": keys.iter()
                                .map(ResponseKey::from)
                                .collect::<Vec<ResponseKey>>()
                        }
                    ))
                    .into_future(),
            ),
            Err(err) => {
                debug!("Failed to fetch keys {}", err);
                Box::new(
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future(),
                )
            }
        }
    })
}

/// Defines a REST endpoint for updating a key in the underlying storage
fn handle_patch(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> HandlerFunction {
    Box::new(move |request, payload| {
        let key_store = key_store.clone();
        let validation = default_validation(&rest_config.issuer());

        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized(msg) => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized(&msg))
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

        Box::new(into_bytes(payload).and_then(move |bytes| {
            let updated_key = match serde_json::from_slice::<UpdatedKey>(&bytes) {
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

            match key_store.update_key(
                &updated_key.public_key,
                &user_id,
                &updated_key.new_display_name,
            ) {
                Ok(()) => HttpResponse::Ok()
                    .json(json!({ "message": "Key updated successfully" }))
                    .into_future(),
                Err(err) => {
                    debug!("Failed to update key {}", err);
                    match err {
                        KeyStoreError::NotFoundError(msg) => HttpResponse::NotFound()
                            .json(ErrorResponse::not_found(&msg))
                            .into_future(),
                        _ => HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    }
                }
            }
        }))
    })
}

/// Defines a REST endpoint for managing keys including fetching and deleting a user's key
pub fn make_key_management_route_with_public_key(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> Resource {
    Resource::build("/biome/keys/{public_key}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_KEYS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Get,
            handle_fetch(
                rest_config.clone(),
                key_store.clone(),
                secret_manager.clone(),
            ),
        )
        .add_method(
            Method::Delete,
            handle_delete(rest_config, key_store, secret_manager),
        )
}

/// Defines a REST endpoint method to fetch a key from the underlying storage
fn handle_fetch(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();
        let validation = default_validation(&rest_config.issuer());

        let public_key = match request.match_info().get("public_key") {
            Some(id) => id.to_owned(),
            None => {
                error!("Public key is not in path request");
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Failed to process request: no public key",
                        ))
                        .into_future(),
                );
            }
        };

        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized(msg) => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized(&msg))
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

        match key_store.fetch_key(&public_key, &user_id) {
            Ok(key) => Box::new(
                HttpResponse::Ok()
                    .json(json!({ "data": ResponseKey::from(&key) }))
                    .into_future(),
            ),
            Err(err) => match err {
                KeyStoreError::NotFoundError(msg) => {
                    debug!("Failed to fetch key: {}", msg);
                    Box::new(
                        HttpResponse::NotFound()
                            .json(ErrorResponse::not_found(&msg))
                            .into_future(),
                    )
                }
                _ => {
                    error!("Failed to fetch key: {}", err);
                    Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    )
                }
            },
        }
    })
}

/// Defines a REST endpoint method to delete a key from the underlying storage
fn handle_delete(
    rest_config: Arc<BiomeRestConfig>,
    key_store: Arc<dyn KeyStore>,
    secret_manager: Arc<dyn SecretManager>,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();
        let validation = default_validation(&rest_config.issuer());

        let public_key = match request.match_info().get("public_key") {
            Some(id) => id.to_owned(),
            None => {
                error!("Public key is not in path request");
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Failed to process request: no public key",
                        ))
                        .into_future(),
                );
            }
        };

        let user_id = match authorize_user(&request, &secret_manager, &validation) {
            AuthorizationResult::Authorized(claims) => claims.user_id(),
            AuthorizationResult::Unauthorized(msg) => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized(&msg))
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

        match key_store.remove_key(&public_key, &user_id) {
            Ok(key) => Box::new(
                HttpResponse::Ok()
                    .json(json!(
                    {
                        "message": "Key successfully deleted",
                        "data": ResponseKey::from(&key)
                    }))
                    .into_future(),
            ),
            Err(err) => match err {
                KeyStoreError::NotFoundError(msg) => {
                    debug!("Failed to delete key: {}", msg);
                    Box::new(
                        HttpResponse::NotFound()
                            .json(ErrorResponse::not_found(&msg))
                            .into_future(),
                    )
                }
                _ => {
                    error!("Failed to delete key: {}", err);
                    Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    )
                }
            },
        }
    })
}
