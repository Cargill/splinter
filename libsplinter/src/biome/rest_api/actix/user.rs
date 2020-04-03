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
use crate::biome::credentials::store::{CredentialsStore, CredentialsStoreError};
use crate::biome::rest_api::resources::authorize::AuthorizationResult;
use crate::biome::rest_api::resources::credentials::UsernamePassword;
use crate::biome::rest_api::BiomeRestConfig;
use crate::biome::user::store::{UserStore, UserStoreError};
use crate::futures::{Future, IntoFuture};
use crate::protocol;
use crate::rest_api::{
    into_bytes, sessions::default_validation, ErrorResponse, HandlerFunction, Method,
    ProtocolVersionRangeGuard, Resource,
};

#[cfg(feature = "biome-key-management")]
use crate::biome::key_management::{
    store::{KeyStore, KeyStoreError},
    Key,
};
use crate::rest_api::secrets::SecretManager;

use crate::biome::rest_api::actix::authorize::authorize_user;
#[cfg(feature = "biome-key-management")]
use crate::biome::rest_api::resources::key_management::{NewKey, ResponseKey};

/// Defines a REST endpoint to list users from the db
pub fn make_list_route<C: CredentialsStore + Clone + 'static>(credentials_store: C) -> Resource {
    Resource::build("/biome/users")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_LIST_USERS_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |_, _| {
            let credentials_store = credentials_store.clone();
            Box::new(match credentials_store.get_usernames() {
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

#[cfg(feature = "biome-key-management")]
/// Defines the `/biome/users/{id}` REST resource for managing users
pub fn make_user_routes<
    C: CredentialsStore + Clone + 'static,
    U: UserStore + Clone + 'static,
    K: KeyStore<Key> + Clone + 'static,
    SM: SecretManager + Clone + 'static,
>(
    rest_config: BiomeRestConfig,
    secret_manager: SM,
    credentials_store: C,
    user_store: U,
    key_store: K,
) -> Resource {
    Resource::build("/biome/users/{id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_USER_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(
            Method::Put,
            add_modify_user_method(
                credentials_store.clone(),
                rest_config.clone(),
                secret_manager.clone(),
                key_store,
            ),
        )
        .add_method(Method::Get, add_fetch_user_method(credentials_store))
        .add_method(
            Method::Delete,
            add_delete_user_method(rest_config, secret_manager, user_store),
        )
}

/// Defines a REST endpoint to fetch a user from the database
/// returns the user's ID and username
fn add_fetch_user_method<C: CredentialsStore + Clone + Sync + 'static>(
    credentials_store: C,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let credentials_store = credentials_store.clone();
        let user_id = if let Some(t) = request.match_info().get("id") {
            t.to_string()
        } else {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request(
                        &"Failed to process request: no user id".to_string(),
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
fn add_modify_user_method<
    C: CredentialsStore + Clone + 'static,
    K: KeyStore<Key> + Clone + 'static,
    SM: SecretManager + Clone + 'static,
>(
    credentials_store: C,
    rest_config: BiomeRestConfig,
    secret_manager: SM,
    key_store: K,
) -> HandlerFunction {
    Box::new(move |request, payload| {
        let credentials_store = credentials_store.clone();
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
            let body = match serde_json::from_slice::<serde_json::Value>(&bytes) {
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
            let new_key_pairs: Vec<Key> = match serde_json::from_slice::<Vec<NewKey>>(&bytes) {
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
            }
            .iter()
            .map(|new_key| {
                Key::new(
                    &new_key.public_key,
                    &new_key.encrypted_private_key,
                    &user_id,
                    &new_key.display_name,
                )
            })
            .collect::<Vec<Key>>();

            let credentials =
                match credentials_store.fetch_credential_by_username(&username_password.username) {
                    Ok(credentials) => credentials,
                    Err(err) => {
                        debug!("Failed to fetch credentials {}", err);
                        match err {
                            CredentialsStoreError::NotFoundError(_) => {
                                return HttpResponse::NotFound()
                                    .json(ErrorResponse::not_found(&format!(
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
                Ok(true) => {
                    let new_password = match body.get("new_password") {
                        Some(val) => match val.as_str() {
                            Some(val) => val,
                            None => &username_password.hashed_password,
                        },
                        None => &username_password.hashed_password,
                    };

                    let response_keys = new_key_pairs
                        .iter()
                        .map(ResponseKey::from)
                        .collect::<Vec<ResponseKey>>();

                    match key_store.update_keys_and_password(
                        &user_id,
                        &new_password,
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

/// Defines a REST endpoint to delete a user from the database
fn add_delete_user_method<U: UserStore + Clone + 'static, SM: SecretManager + Clone + 'static>(
    rest_config: BiomeRestConfig,
    secret_manager: SM,
    user_store: U,
) -> HandlerFunction {
    Box::new(move |request, _| {
        let mut user_store = user_store.clone();
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

        Box::new(match user_store.remove_user(&user_id) {
            Ok(()) => HttpResponse::Ok()
                .json(json!({ "message": "User deleted sucessfully" }))
                .into_future(),
            Err(err) => match err {
                UserStoreError::NotFoundError(msg) => {
                    debug!("User not found: {}", msg);
                    HttpResponse::NotFound()
                        .json(ErrorResponse::not_found(&format!(
                            "User ID not found: {}",
                            &user_id
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
