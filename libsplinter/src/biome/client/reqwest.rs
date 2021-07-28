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

//! Contains the Reqwest-based implementation of BiomeClient.

use std::convert::From;

use reqwest::{blocking::Client, StatusCode};

use crate::error::InternalError;
use crate::protocol::BIOME_PROTOCOL_VERSION;

use super::{Authorization, BiomeClient, Credentials, Key, NewKey, Profile, UpdateUser};

const PAGING_LIMIT: u32 = 100;

#[derive(Deserialize)]
struct ServerError {
    pub message: String,
}

pub struct ReqwestBiomeClient {
    url: String,
    auth: Option<String>,
}

impl ReqwestBiomeClient {
    pub fn new(url: String) -> Self {
        ReqwestBiomeClient { url, auth: None }
    }

    pub fn add_auth(&mut self, auth: String) {
        self.auth = Some(auth);
    }

    pub fn auth(&self) -> Result<String, InternalError> {
        match &self.auth {
            Some(auth) => Ok(auth.into()),
            None => Err(InternalError::with_message(
                "ReqwestBiomeClient does not have authorization".into(),
            )),
        }
    }
}

impl BiomeClient for ReqwestBiomeClient {
    /// Register a user with Biome.
    fn register(&self, username: &str, password: &str) -> Result<Credentials, InternalError> {
        let request = Client::new()
            .post(&format!("{}/biome/register", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .json(&json!({
                "username": username,
                "hashed_password": password,
            }));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to register Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    let data: ClientCredentialsResponse =
                        res.json::<ClientCredentialsResponse>().map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?;

                    Ok(Credentials::from(data.data))
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                "Biome register request failed with status code '{}', but error \
                             response was not valid",
                                status
                            ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to register Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Login a user with Biome.
    fn login(&self, username: &str, password: &str) -> Result<Authorization, InternalError> {
        let request = Client::new()
            .post(&format!("{}/biome/login", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .json(&json!({
                "username": username,
                "hashed_password": password,
            }));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to login Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ClientAuthorization>()
                        .map(Authorization::from)
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome login request failed with status code '{}', but error \
                             response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to login Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Logout a user with Biome, removes the user's Splinter access token.
    fn logout(&self) -> Result<(), InternalError> {
        let request = Client::new()
            .patch(&format!("{}/biome/logout", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to logout Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome logout request failed with status code '{}', but error \
                             response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to logout Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Returns a new access token for the Biome user, based on the supplied refresh token
    fn get_new_access_token(&self, refresh_token: &str) -> Result<String, InternalError> {
        let request = Client::new()
            .post(&format!("{}/biome/token", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&json!({ "token": refresh_token }));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to get new access token for Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(res
                        .json::<ClientAccessToken>()
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?
                        .token)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome access token request failed with status code '{}', but \
                             error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to get new access token for Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Verify the credentials of a Biome user.
    fn verify(&self, username: &str, password: &str) -> Result<(), InternalError> {
        let request = Client::new()
            .post(&format!("{}/biome/verify", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&json!({"username": username, "hashed_password": password}));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to verify Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome verify request failed with status code '{}', but error \
                             response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to verify Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// List all Biome users.
    fn list_users(&self) -> Result<Box<dyn Iterator<Item = Credentials>>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/biome/users?limit={}", self.url, PAGING_LIMIT))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list Biome users".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    // Deserialize the response into a list of `ClientCredentials` and then
                    // convert the contents of this list into the type returned, `Credentials`
                    let response_data: Box<dyn Iterator<Item = Credentials>> = Box::new(
                        res.json::<Vec<ClientCredentials>>()
                            .map(|entries| {
                                entries
                                    .into_iter()
                                    .map(Credentials::from)
                                    .collect::<Vec<Credentials>>()
                            })
                            .map_err(|_| {
                                InternalError::with_message(
                                    "Request was successful, but received an invalid response"
                                        .into(),
                                )
                            })?
                            .into_iter(),
                    );
                    Ok(response_data)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome list users request failed with status code '{}', but \
                             error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list Biome users: {}",
                        message
                    )))
                }
            })
    }

    /// Get a Biome user.
    fn get_user(&self, user_id: &str) -> Result<Option<Credentials>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/biome/users/{}", self.url, user_id))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to fetch Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ClientCredentials>()
                        .map(Credentials::from)
                        .map(Some)
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome fetch user request failed with status code '{}', but \
                                    error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to fetch Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Update a Biome user's password or associated key pairs.
    fn update_user(
        &self,
        user_id: &str,
        updated_user: UpdateUser,
    ) -> Result<Box<dyn Iterator<Item = Key>>, InternalError> {
        let request = Client::new()
            .put(&format!("{}/biome/users/{}", self.url, user_id))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&ClientUpdateUser::from(updated_user));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to update Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    let response_data = res
                        .json::<ClientKeyListResponse>()
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?
                        .data
                        .into_iter();
                    let keys: Box<dyn Iterator<Item = Key>> =
                        Box::new(response_data.map(Key::from));
                    Ok(keys)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome update user request failed with status code '{}', but \
                             error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to update Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// Remove a Biome user.
    fn delete_user(&self, user_id: &str) -> Result<(), InternalError> {
        let request = Client::new()
            .delete(&format!("{}/biome/users/{}", self.url, user_id))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to delete Biome user".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome delete user request failed with status code '{}', but error \
                             response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to delete Biome user: {}",
                        message
                    )))
                }
            })
    }

    /// List all Biome user profiles.
    fn list_profiles(&self) -> Result<Box<dyn Iterator<Item = Profile>>, InternalError> {
        let request = Client::new()
            .get(&format!(
                "{}/biome/profiles?limit={}",
                self.url, PAGING_LIMIT
            ))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list Biome profiles".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    // Deserialize the response data into `ClientProfile` and then convert to the
                    // return type, `Profile`
                    let response_data: Box<dyn Iterator<Item = Profile>> = Box::new(
                        res.json::<Vec<ClientProfile>>()
                            .map(|entries| {
                                entries
                                    .into_iter()
                                    .map(Profile::from)
                                    .collect::<Vec<Profile>>()
                            })
                            .map_err(|_| {
                                InternalError::with_message(
                                    "Request was successful, but received an invalid response"
                                        .into(),
                                )
                            })?
                            .into_iter(),
                    );
                    Ok(response_data)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome list profiles request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list Biome profiles: {}",
                        message
                    )))
                }
            })
    }

    /// Get a Biome user's profile.
    fn get_profile(&self, user_id: &str) -> Result<Option<Profile>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/biome/profiles/{}", self.url, user_id))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to fetch Biome profile".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<ClientProfile>().map(Profile::from).map(Some).map_err(|_| {
                        InternalError::with_message(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome fetch profile request failed with status code '{}', but error \
                             response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to fetch Biome profile: {}",
                        message
                    )))
                }
            })
    }

    /// List the keys associated with the authorized Biome user.
    fn list_user_keys(&self) -> Result<Box<dyn Iterator<Item = Key>>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/biome/keys?limit={}", self.url, PAGING_LIMIT))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to list Biome user's keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    let response_data = res
                        .json::<ClientKeyListResponse>()
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?
                        .data
                        .into_iter();
                    let keys: Box<dyn Iterator<Item = Key>> =
                        Box::new(response_data.map(Key::from));
                    Ok(keys)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome list user's keys request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to list Biome user's keys: {}",
                        message
                    )))
                }
            })
    }

    /// Update a Biome user's key pair display name.
    fn update_key(&self, public_key: &str, new_display_name: &str) -> Result<(), InternalError> {
        let request = Client::new()
            .patch(&format!("{}/biome/keys", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&json!({
                "public_key": public_key,
                "new_display_name": new_display_name,
            }));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to update Biome user's keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome update user's keys request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to update Biome user's keys: {}",
                        message
                    )))
                }
            })
    }

    /// Add a key pair for a Biome user.
    fn add_key(&self, user_id: &str, new_key: NewKey) -> Result<(), InternalError> {
        let request = Client::new()
            .post(&format!("{}/biome/keys", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&ClientKey::from((user_id.to_string(), new_key)));

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to add Biome user's keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome add user's keys request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to add Biome user's keys: {}",
                        message
                    )))
                }
            })
    }

    /// Replace a Biome user's keys
    #[cfg(feature = "biome-replace-keys")]
    fn replace_keys(&self, keys: Vec<NewKey>) -> Result<(), InternalError> {
        let keys: Vec<ClientNewKey> = keys.into_iter().map(ClientNewKey::from).collect();
        let request = Client::new()
            .put(&format!("{}/biome/keys", self.url))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?)
            .json(&keys);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to replace Biome user keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome replace user key request failed with status code '{}', but \
                             error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to replace user keys: {}",
                        message
                    )))
                }
            })
    }

    /// Get a Biome user's key pair.
    fn get_key(&self, public_key: &str) -> Result<Option<Key>, InternalError> {
        let request = Client::new()
            .get(&format!("{}/biome/keys/{}", self.url, public_key))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to get Biome user's keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    let response_data = res
                        .json::<ClientKeyResponse>()
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?
                        .data;
                    Ok(Some(Key::from(response_data)))
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome get user's keys request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to get Biome user's keys: {}",
                        message
                    )))
                }
            })
    }

    /// Delete one of a Biome user's key pairs.
    fn delete_key(&self, public_key: &str) -> Result<Option<Key>, InternalError> {
        let request = Client::new()
            .delete(&format!("{}/biome/keys/{}", self.url, public_key))
            .header("SplinterProtocolVersion", BIOME_PROTOCOL_VERSION)
            .header("Authorization", &self.auth()?);

        let response = request.send();

        response
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "Failed to delete Biome user's keys".to_string(),
                )
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    let response_data = res
                        .json::<ClientKeyResponse>()
                        .map_err(|_| {
                            InternalError::with_message(
                                "Request was successful, but received an invalid response".into(),
                            )
                        })?
                        .data;
                    Ok(Some(Key::from(response_data)))
                } else if status == StatusCode::NOT_FOUND {
                    Ok(None)
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|err| {
                            InternalError::from_source_with_message(
                                err.into(),
                                format!(
                                    "Biome delete user's keys request failed with status code '{}', \
                             but error response was not valid",
                                    status
                                ),
                            )
                        })?
                        .message;

                    Err(InternalError::with_message(format!(
                        "Failed to delete Biome user's keys: {}",
                        message
                    )))
                }
            })
    }
}

/// Represents Biome `Credentials`, used for deserializing JSON objects
#[derive(Debug, Deserialize)]
pub struct ClientCredentials {
    pub user_id: String,
    pub username: String,
}

impl From<ClientCredentials> for Credentials {
    fn from(client_credentials: ClientCredentials) -> Self {
        Credentials {
            user_id: client_credentials.user_id,
            username: client_credentials.username,
        }
    }
}

/// Used for deserializing the response from Biome's REST API containing `Credentials`
#[derive(Debug, Deserialize)]
pub struct ClientCredentialsResponse {
    pub message: String,
    pub data: ClientCredentials,
}

/// Information pertaining to a user's active session, returned by Biome when a user logs in.
#[derive(Debug, Deserialize)]
pub struct ClientAuthorization {
    pub user_id: String,
    pub token: String,
    pub refresh_token: String,
}

/// Information pertaining to a user's active session, returned by Biome when a user logs in.
#[derive(Debug, Deserialize)]
pub struct ClientAccessToken {
    pub token: String,
}

impl From<ClientAuthorization> for Authorization {
    fn from(client_authorization: ClientAuthorization) -> Self {
        Authorization {
            user_id: client_authorization.user_id,
            token: client_authorization.token,
            refresh_token: client_authorization.refresh_token,
        }
    }
}

/// Struct representing the items to update a Biome user, used to serialize the data submitted
/// to the Biome REST API
#[derive(Debug, Serialize)]
pub struct ClientUpdateUser {
    pub username: String,
    pub hashed_password: String,
    pub new_password: Option<String>,
    pub new_key_pairs: Vec<ClientNewKey>,
}

impl From<UpdateUser> for ClientUpdateUser {
    fn from(update_user: UpdateUser) -> Self {
        ClientUpdateUser {
            username: update_user.username,
            hashed_password: update_user.hashed_password,
            new_password: update_user.new_password,
            new_key_pairs: update_user
                .new_key_pairs
                .into_iter()
                .map(ClientNewKey::from)
                .collect(),
        }
    }
}

/// Struct representing new key pairs to be added while updating a Biome user, used to serialize
/// the data submitted to the Biome REST API
#[derive(Debug, Serialize)]
pub struct ClientNewKey {
    pub public_key: String,
    pub encrypted_private_key: String,
    pub display_name: String,
}

impl From<NewKey> for ClientNewKey {
    fn from(new_key: NewKey) -> Self {
        ClientNewKey {
            public_key: new_key.public_key,
            encrypted_private_key: new_key.encrypted_private_key,
            display_name: new_key.display_name,
        }
    }
}

/// Struct representing the response from Biome when listing keys, used to deserialize data
#[derive(Debug, Deserialize)]
pub struct ClientKeyListResponse {
    pub message: Option<String>,
    pub data: Vec<ClientKey>,
}

/// Struct representing the response from Biome when fetching a key, used to deserialize data
#[derive(Debug, Deserialize)]
pub struct ClientKeyResponse {
    pub message: Option<String>,
    pub data: ClientKey,
}

/// Struct representing Biome users' key pair details, used to serialize request data and
/// deserialize response data
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientKey {
    pub display_name: String,
    pub encrypted_private_key: String,
    pub public_key: String,
    pub user_id: String,
}

impl From<ClientKey> for Key {
    fn from(client_key: ClientKey) -> Self {
        Key {
            public_key: client_key.public_key,
            encrypted_private_key: client_key.encrypted_private_key,
            display_name: client_key.display_name,
            user_id: client_key.user_id,
        }
    }
}

/// Takes the arguments provided to the `add_key` method and creates the serializable `ClientKey`
/// to be submitted to Biome's REST API
impl From<(String, NewKey)> for ClientKey {
    fn from((user_id, new_key): (String, NewKey)) -> Self {
        ClientKey {
            display_name: new_key.display_name,
            encrypted_private_key: new_key.encrypted_private_key,
            public_key: new_key.public_key,
            user_id,
        }
    }
}

/// Struct representing Biome users' profile details, used to deserialize response data
#[derive(Debug, Deserialize)]
pub struct ClientProfile {
    pub user_id: String,
    pub subject: String,
    pub name: Option<String>,
}

impl From<ClientProfile> for Profile {
    fn from(client_profile: ClientProfile) -> Self {
        Profile {
            user_id: client_profile.user_id,
            subject: client_profile.subject,
            name: client_profile.name,
        }
    }
}
