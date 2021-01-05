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
use uuid::Uuid;

use crate::actix_web::HttpResponse;
use crate::biome::credentials::store::{
    CredentialsBuilder, CredentialsStore, CredentialsStoreError,
};
use crate::biome::rest_api::resources::credentials::{NewUser, UsernamePassword};
use crate::biome::rest_api::BiomeRestConfig;
use crate::futures::{Future, IntoFuture};
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{into_bytes, Method, ProtocolVersionRangeGuard, Resource},
    ErrorResponse,
};

/// This is the UUID namespace for Biome user IDs generated for users that register with Biome
/// credentials. This will prevent collisions with Biome user IDs generated for users that login
/// with OAuth. The `u128` was calculated by creating a v5 UUID with the nil namespace and the name
/// `b"biome credentials"`.
const UUID_NAMESPACE: Uuid = Uuid::from_u128(140899893353887994607859851180695869034);

/// Defines a REST endpoint to add a user and credentials to the database
///
/// The payload should be in the JSON format:
///   {
///       "username": <username of new user>
///       "hashed_password": <hash of the password the user will use to log in>
///   }
pub fn make_register_route(
    credentials_store: Arc<dyn CredentialsStore>,
    rest_config: Arc<BiomeRestConfig>,
) -> Resource {
    Resource::build("/biome/register")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::BIOME_REGISTER_PROTOCOL_MIN,
            protocol::BIOME_PROTOCOL_VERSION,
        ))
        .add_method(Method::Post, move |_, payload| {
            let credentials_store = credentials_store.clone();
            let rest_config = rest_config.clone();
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
                let user_id = Uuid::new_v5(&UUID_NAMESPACE, Uuid::new_v4().as_bytes()).to_string();
                let credentials_builder = CredentialsBuilder::default();
                let credentials = match credentials_builder
                    .with_user_id(&user_id)
                    .with_username(&username_password.username)
                    .with_password(&username_password.hashed_password)
                    .with_password_encryption_cost(rest_config.password_encryption_cost())
                    .build()
                {
                    Ok(credential) => credential,
                    Err(err) => {
                        debug!("Failed to create credentials {}", err);
                        return HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future();
                    }
                };

                match credentials_store.add_credentials(credentials) {
                    Ok(()) => {
                        let new_user = NewUser {
                            user_id: &user_id,
                            username: &username_password.username,
                        };
                        HttpResponse::Ok()
                            .json(json!({
                                "message": "User created successfully",
                                "data": new_user,
                            }))
                            .into_future()
                    }
                    Err(err) => {
                        debug!("Failed to add new credentials to database {}", err);
                        match err {
                            CredentialsStoreError::DuplicateError(err) => {
                                HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(&format!(
                                        "Failed to create user: {}",
                                        err
                                    )))
                                    .into_future()
                            }
                            _ => HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future(),
                        }
                    }
                }
            }))
        })
}
