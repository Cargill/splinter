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

//! Traits and resources useful for communicating with Splinter Biome as a client.

#[cfg(feature = "biome-client-reqwest")]
mod reqwest;

use crate::error::InternalError;

#[cfg(feature = "biome-client-reqwest")]
pub use self::reqwest::ReqwestBiomeClient;

/// Biome `Credentials` holds information specific to a Biome user.
#[derive(Debug)]
pub struct Credentials {
    pub user_id: String,
    pub username: String,
}

/// Information pertaining to a user's active session, returned by Biome when a user logs in.
#[derive(Debug)]
pub struct Authorization {
    pub user_id: String,
    pub token: String,
    pub refresh_token: String,
}

/// Biome users' key pair details.
#[derive(Debug)]
pub struct Key {
    pub display_name: String,
    pub encrypted_private_key: String,
    pub public_key: String,
    pub user_id: String,
}

/// Biome users' profile details.
#[derive(Debug)]
pub struct Profile {
    pub user_id: String,
    pub subject: String,
    pub name: Option<String>,
}

/// Biome user details required to update a user.
#[derive(Debug)]
pub struct UpdateUser {
    pub username: String,
    pub hashed_password: String,
    pub new_password: Option<String>,
    pub new_key_pairs: Vec<NewKey>,
}

/// New key pairs to be added while updating a Biome user.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NewKey {
    pub public_key: String,
    pub encrypted_private_key: String,
    pub display_name: String,
}

pub trait BiomeClient {
    /// Register a user with Biome.
    ///
    /// # Arguments
    ///
    /// * `username`: Username of the Biome user being created
    /// * `password`: Hashed password of the Biome user being created
    fn register(&self, username: &str, password: &str) -> Result<Credentials, InternalError>;

    /// Login a user with Biome.
    ///
    /// # Arguments
    ///
    /// * `username`: Username of the Biome user
    /// * `password`: Hashed password of the Biome user
    fn login(&self, username: &str, password: &str) -> Result<Authorization, InternalError>;

    /// Logout a user with Biome, removes the user's Splinter access token.
    fn logout(&self) -> Result<(), InternalError>;

    /// Returns a new access token for the Biome user, based on the supplied refresh token
    ///
    /// # Arguments
    ///
    /// * `refresh_token`: Current refresh token of the Biome user
    fn get_new_access_token(&self, refresh_token: &str) -> Result<String, InternalError>;

    /// Verify the credentials of a Biome user.
    ///
    /// # Arguments
    ///
    /// * `username`: Username of the Biome user being verified
    /// * `password`: Hashed password of the Biome user being verified
    fn verify(&self, username: &str, password: &str) -> Result<(), InternalError>;

    /// List all Biome users.
    fn list_users(&self) -> Result<Box<dyn Iterator<Item = Credentials>>, InternalError>;

    /// Get a Biome user.
    ///
    /// # Arguments
    ///
    /// * `user_id`: Biome identifier for the user being retrieved
    fn get_user(&self, user_id: &str) -> Result<Option<Credentials>, InternalError>;

    /// Update a Biome user's password or associated key pairs.
    ///
    /// # Arguments
    ///
    /// * `user_id`: Biome identifier for the user being retrieved
    /// * `updated_user`: Struct containing all Biome-specific information of the user, as well
    ///                   Biome-specific values to be updated
    fn update_user(
        &self,
        user_id: &str,
        updated_user: UpdateUser,
    ) -> Result<Box<dyn Iterator<Item = Key>>, InternalError>;

    /// Remove a Biome user.
    ///
    /// # Arguments
    ///
    /// * `user_id`: Biome identifier for the user being deleted
    fn delete_user(&self, user_id: &str) -> Result<(), InternalError>;

    /// List all Biome user profiles.
    fn list_profiles(&self) -> Result<Box<dyn Iterator<Item = Profile>>, InternalError>;

    /// Get a Biome user's profile.
    ///
    /// # Arguments
    ///
    /// * `user_id`: Biome identifier for the user
    fn get_profile(&self, user_id: &str) -> Result<Option<Profile>, InternalError>;

    /// List the keys associated with the authorized Biome user.
    fn list_user_keys(&self) -> Result<Box<dyn Iterator<Item = Key>>, InternalError>;

    /// Update a Biome user's key pair display name.
    ///
    /// # Arguments
    ///
    /// * `public_key`: Public key of the key pair to be updated
    /// * `new_display_name`: Updated display name for the key pair
    fn update_key(&self, public_key: &str, new_display_name: &str) -> Result<(), InternalError>;

    /// Replace the current Biome user's keys
    ///
    /// # Arguments
    ///
    /// * `keys`: New keys for the user
    fn replace_keys(&self, keys: Vec<NewKey>) -> Result<(), InternalError>;

    /// Add a key pair for a Biome user.
    ///
    /// # Arguments
    ///
    /// * `user_id`: Biome identifier for the user
    /// * `new_key`: Public/Private key pair to be added for a Biome user
    fn add_key(&self, user_id: &str, new_key: NewKey) -> Result<(), InternalError>;

    /// Get a Biome user's key pair.
    ///
    /// # Arguments
    ///
    /// * `public_key`: Public key of the key pair to be retrieved
    fn get_key(&self, public_key: &str) -> Result<Option<Key>, InternalError>;

    /// Delete one of a Biome user's key pairs.
    ///
    /// # Arguments
    ///
    /// * `public_key`: Public key of the key pair to be deleted
    fn delete_key(&self, public_key: &str) -> Result<Option<Key>, InternalError>;
}
