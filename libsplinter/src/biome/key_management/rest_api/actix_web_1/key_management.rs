// Copyright 2018-2021 Cargill Incorporated
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
use crate::biome::key_management::{
    rest_api::resources::{NewKey, ResponseKey, UpdatedKey},
    store::{KeyStore, KeyStoreError},
    Key,
};
use crate::futures::{Future, IntoFuture};
use crate::protocol;
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::Permission;
use crate::rest_api::{
    actix_web_1::{into_bytes, HandlerFunction, Method, ProtocolVersionRangeGuard, Resource},
    auth::identity::Identity,
    ErrorResponse,
};

/// Defines a REST endpoint for managing keys including inserting, listing and updating keys
pub fn make_key_management_route(key_store: Arc<dyn KeyStore>) -> Resource {
    let resource =
        Resource::build("/biome/keys").add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_KEYS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ));
    #[cfg(feature = "authorization")]
    {
        #[cfg(feature = "biome-replace-keys")]
        let resource = resource
            .add_method(
                Method::Put,
                Permission::AllowAuthenticated,
                handle_put(key_store.clone()),
            )
            .add_request_guard(
                ProtocolVersionRangeGuard::new(
                    protocol::BIOME_REPLACE_KEYS_PROTOCOL_MIN,
                    protocol::BIOME_PROTOCOL_VERSION,
                )
                .with_method(Method::Put),
            );

        resource
            .add_method(
                Method::Post,
                Permission::AllowAuthenticated,
                handle_post(key_store.clone()),
            )
            .add_method(
                Method::Get,
                Permission::AllowAuthenticated,
                handle_get(key_store.clone()),
            )
            .add_method(
                Method::Patch,
                Permission::AllowAuthenticated,
                handle_patch(key_store),
            )
    }
    #[cfg(not(feature = "authorization"))]
    {
        #[cfg(feature = "biome-replace-keys")]
        let resource = resource
            .add_method(Method::Put, handle_put(key_store.clone()))
            .add_request_guard(
                ProtocolVersionRangeGuard::new(
                    protocol::BIOME_REPLACE_KEYS_PROTOCOL_MIN,
                    protocol::BIOME_PROTOCOL_VERSION,
                )
                .with_method(Method::Put),
            );

        resource
            .add_method(Method::Post, handle_post(key_store.clone()))
            .add_method(Method::Get, handle_get(key_store.clone()))
            .add_method(Method::Patch, handle_patch(key_store))
    }
}

/// Defines a REST endpoint for adding a key to the underlying storage
fn handle_post(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, payload| {
        let key_store = key_store.clone();

        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
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
                &user,
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
fn handle_get(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();

        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
        };

        match key_store.list_keys(Some(&user)) {
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

/// Defines a REST endpoint for updating all keys in the underlying storage
#[cfg(feature = "biome-replace-keys")]
fn handle_put(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, payload| {
        let key_store = key_store.clone();
        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
        };

        Box::new(into_bytes(payload).and_then(move |bytes| {
            let new_keys = match serde_json::from_slice::<Vec<NewKey>>(&bytes) {
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

            let new_keys: Vec<Key> = new_keys
                .iter()
                .map(|new_key| {
                    Key::new(
                        &new_key.public_key,
                        &new_key.encrypted_private_key,
                        &user,
                        &new_key.display_name,
                    )
                })
                .collect();

            match key_store.replace_keys(&user, &new_keys) {
                Ok(()) => HttpResponse::Ok()
                    .json(json!({ "message": "Keys replaced successfully" }))
                    .into_future(),
                Err(err) => {
                    debug!("Failed to replace keys in database {}", err);
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

/// Defines a REST endpoint for updating a key in the underlying storage
fn handle_patch(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, payload| {
        let key_store = key_store.clone();
        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
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
                &user,
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
pub fn make_key_management_route_with_public_key(key_store: Arc<dyn KeyStore>) -> Resource {
    let resource = Resource::build("/biome/keys/{public_key}").add_request_guard(
        ProtocolVersionRangeGuard::new(
            protocol::BIOME_KEYS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ),
    );
    #[cfg(feature = "authorization")]
    {
        resource
            .add_method(
                Method::Get,
                Permission::AllowAuthenticated,
                handle_fetch(key_store.clone()),
            )
            .add_method(
                Method::Delete,
                Permission::AllowAuthenticated,
                handle_delete(key_store),
            )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource
            .add_method(Method::Get, handle_fetch(key_store.clone()))
            .add_method(Method::Delete, handle_delete(key_store))
    }
}

/// Defines a REST endpoint method to fetch a key from the underlying storage
fn handle_fetch(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();

        let public_key = match request.match_info().get("public_key") {
            Some(id) => id.to_owned(),
            None => {
                error!("Public key is not in path request");
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            &"Failed to process request: no public key".to_string(),
                        ))
                        .into_future(),
                );
            }
        };

        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
        };

        match key_store.fetch_key(&public_key, &user) {
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
fn handle_delete(key_store: Arc<dyn KeyStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let key_store = key_store.clone();

        let public_key = match request.match_info().get("public_key") {
            Some(id) => id.to_owned(),
            None => {
                error!("Public key is not in path request");
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            &"Failed to process request: no public key".to_string(),
                        ))
                        .into_future(),
                );
            }
        };

        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
        };

        match key_store.remove_key(&public_key, &user) {
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
