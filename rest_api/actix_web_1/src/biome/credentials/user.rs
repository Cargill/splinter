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
#[cfg(feature = "biome-key-management")]
use futures::Future;
use futures::IntoFuture;

#[cfg(feature = "biome-key-management")]
use crate::biome::credentials::BiomeCredentialsRestConfig;
#[cfg(feature = "biome-key-management")]
use crate::framework::into_bytes;
#[cfg(feature = "biome-key-management")]
use crate::framework::HandlerFunction;
use crate::framework::{Method, ProtocolVersionRangeGuard, Resource};
use splinter::biome::credentials::store::CredentialsStore;
#[cfg(feature = "biome-key-management")]
use splinter::biome::credentials::store::CredentialsStoreError;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::auth::Permission;
use splinter_rest_api_common::{response_models::ErrorResponse, SPLINTER_PROTOCOL_VERSION};

#[cfg(feature = "biome-key-management")]
use splinter::biome::key_management::{
    store::{KeyStore, KeyStoreError},
    Key,
};

#[cfg(feature = "biome-key-management")]
use crate::biome::credentials::resources::{key_management::ResponseKey, user::ModifyUser};

const BIOME_LIST_USERS_PROTOCOL_MIN: u32 = 1;
#[cfg(feature = "biome-key-management")]
const BIOME_USER_PROTOCOL_MIN: u32 = 1;

#[cfg(feature = "authorization")]
const BIOME_USER_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "biome.user.read",
    permission_display_name: "Biome user read",
    permission_description: "Allows the client to view all Biome users",
};
#[cfg(all(feature = "authorization", feature = "biome-key-management"))]
const BIOME_USER_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "biome.user.write",
    permission_display_name: "Biome user write",
    permission_description: "Allows the client to modify all Biome users",
};

/// Defines a REST endpoint to list users from the db
pub fn make_list_route(credentials_store: Arc<dyn CredentialsStore>) -> Resource {
    let resource = Resource::build("/biome/users").add_request_guard(
        ProtocolVersionRangeGuard::new(BIOME_LIST_USERS_PROTOCOL_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Get, BIOME_USER_READ_PERMISSION, move |_, _| {
            let credentials_store = credentials_store.clone();
            Box::new(match credentials_store.list_usernames() {
                Ok(users) => HttpResponse::Ok().json(users).into_future(),
                Err(err) => {
                    debug!("Failed to get users from the database {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |_, _| {
            let credentials_store = credentials_store.clone();
            Box::new(match credentials_store.list_usernames() {
                Ok(users) => HttpResponse::Ok().json(users).into_future(),
                Err(err) => {
                    debug!("Failed to get users from the database {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        })
    }
}

#[cfg(feature = "biome-key-management")]
/// Defines the `/biome/users/{id}` REST resource for managing users
pub fn make_user_routes(
    rest_config: Arc<BiomeCredentialsRestConfig>,
    credentials_store: Arc<dyn CredentialsStore>,
    key_store: Arc<dyn KeyStore>,
) -> Resource {
    let resource = Resource::build("/biome/users/{id}").add_request_guard(
        ProtocolVersionRangeGuard::new(BIOME_USER_PROTOCOL_MIN, SPLINTER_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource
            .add_method(
                Method::Put,
                BIOME_USER_WRITE_PERMISSION,
                add_modify_user_method(credentials_store.clone(), rest_config, key_store),
            )
            .add_method(
                Method::Get,
                BIOME_USER_READ_PERMISSION,
                add_fetch_user_method(credentials_store.clone()),
            )
            .add_method(
                Method::Delete,
                BIOME_USER_WRITE_PERMISSION,
                add_delete_user_method(credentials_store),
            )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource
            .add_method(
                Method::Put,
                add_modify_user_method(credentials_store.clone(), rest_config, key_store),
            )
            .add_method(
                Method::Get,
                add_fetch_user_method(credentials_store.clone()),
            )
            .add_method(Method::Delete, add_delete_user_method(credentials_store))
    }
}

#[cfg(feature = "biome-key-management")]
/// Defines a REST endpoint to fetch a user from the database
/// returns the user's ID and username
fn add_fetch_user_method(credentials_store: Arc<dyn CredentialsStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let credentials_store = credentials_store.clone();
        let user_id = if let Some(t) = request.match_info().get("id") {
            t.to_string()
        } else {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request(
                        "Failed to process request: no user id",
                    ))
                    .into_future(),
            );
        };
        Box::new(match credentials_store.fetch_username_by_id(&user_id) {
            Ok(user) => HttpResponse::Ok().json(user).into_future(),
            Err(err) => {
                debug!("Failed to get user from the database {}", err);
                match err {
                    CredentialsStoreError::NotFoundError(_) => HttpResponse::NotFound()
                        .json(ErrorResponse::not_found(&format!(
                            "User ID not found: {}",
                            &user_id
                        )))
                        .into_future(),
                    _ => HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future(),
                }
            }
        })
    })
}

#[cfg(feature = "biome-key-management")]
/// Defines a REST endpoint to edit a user's credentials in the database
///
/// The payload should be in the JSON format:
///   {
///       "username": <existing username of the user>
///       "hashed_password": <hash of the user's existing password>
///       "new_password": <hash of the user's updated password>
///       "new_key_pairs":
///       [
///           {
///               "display_name": <display name for key>
///               "public_key": <public key of the user>
///               "encrypted_private_key": <updated encrypted private key of the the user>
///           },
///           { ... }, { ... }, ...
///       ]
///   }
fn add_modify_user_method(
    credentials_store: Arc<dyn CredentialsStore>,
    rest_config: Arc<BiomeCredentialsRestConfig>,
    key_store: Arc<dyn KeyStore>,
) -> HandlerFunction {
    let encryption_cost = rest_config.password_encryption_cost();
    Box::new(move |request, payload| {
        let credentials_store = credentials_store.clone();
        let key_store = key_store.clone();
        let user = match request.match_info().get("id") {
            Some(t) => t.to_string(),
            None => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Failed to process request: no user id",
                        ))
                        .into_future(),
                )
            }
        };

        Box::new(into_bytes(payload).and_then(move |bytes| {
            let modify_user = match serde_json::from_slice::<ModifyUser>(&bytes) {
                Ok(val) => val,
                Err(err) => {
                    debug!("Error parsing request body {}", err);
                    return HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&format!(
                            "Failed to parse payload body: {}",
                            err
                        )))
                        .into_future();
                }
            };
            let new_key_pairs: Vec<Key> = modify_user
                .new_key_pairs
                .iter()
                .map(|new_key| {
                    Key::new(
                        &new_key.public_key,
                        &new_key.encrypted_private_key,
                        &user,
                        &new_key.display_name,
                    )
                })
                .collect::<Vec<Key>>();

            let credentials =
                match credentials_store.fetch_credential_by_username(&modify_user.username) {
                    Ok(credentials) => credentials,
                    Err(err) => {
                        debug!("Failed to fetch credentials {}", err);
                        match err {
                            CredentialsStoreError::NotFoundError(_) => {
                                return HttpResponse::NotFound()
                                    .json(ErrorResponse::not_found(&format!(
                                        "Username not found: {}",
                                        modify_user.username
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
            match credentials.verify_password(&modify_user.hashed_password) {
                Ok(true) => {
                    let new_password = match modify_user.new_password {
                        Some(val) => val,
                        // If no new password, pull old password for update operation
                        None => credentials.password,
                    };

                    let response_keys = new_key_pairs
                        .iter()
                        .map(ResponseKey::from)
                        .collect::<Vec<ResponseKey>>();

                    match key_store.update_keys_and_password(
                        &user,
                        &new_password,
                        encryption_cost,
                        &new_key_pairs,
                    ) {
                        Ok(()) => HttpResponse::Ok()
                            .json(json!({
                                "message": "Credentials and key updated successfully",
                                "data": response_keys,
                            }))
                            .into_future(),
                        Err(err) => match err {
                            KeyStoreError::DuplicateKeyError(msg) => HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request(&msg))
                                .into_future(),
                            KeyStoreError::UserDoesNotExistError(msg) => HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request(&msg))
                                .into_future(),
                            _ => HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future(),
                        },
                    }
                }
                Ok(false) => HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request("Invalid password"))
                    .into_future(),
                Err(err) => {
                    error!("Failed to verify password {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            }
        }))
    })
}

#[cfg(feature = "biome-key-management")]
/// Defines a REST endpoint to delete a user from the database
fn add_delete_user_method(credentials_store: Arc<dyn CredentialsStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let credentials_store = credentials_store.clone();
        let user = match request.match_info().get("id") {
            Some(t) => t.to_string(),
            None => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Failed to process request: no user id",
                        ))
                        .into_future(),
                )
            }
        };

        Box::new(match credentials_store.remove_credentials(&user) {
            Ok(()) => HttpResponse::Ok()
                .json(json!({ "message": "User deleted successfully" }))
                .into_future(),
            Err(err) => match err {
                CredentialsStoreError::NotFoundError(msg) => {
                    debug!("User not found: {}", msg);
                    HttpResponse::NotFound()
                        .json(ErrorResponse::not_found(&format!(
                            "User ID not found: {}",
                            user
                        )))
                        .into_future()
                }
                _ => {
                    error!("Failed to delete user in database {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            },
        })
    })
}
