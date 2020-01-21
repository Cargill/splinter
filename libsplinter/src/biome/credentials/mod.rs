// Copyright 2019 Cargill Incorporated
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

//! Defines a basic API to register and authenticate a SplinterUser using a username and a password.
//! Not recommended for use in production.

pub(in crate::biome) mod credentials_store;

pub mod database;
mod error;
#[cfg(feature = "rest-api")]
pub(in crate::biome) mod rest_resources;

use std::convert::TryFrom;

use bcrypt::{hash, verify, DEFAULT_COST};

use database::models::{NewUserCredentialsModel, UserCredentialsModel};

pub use error::{CredentialsStoreError, UserCredentialsBuilderError, UserCredentialsError};

const MEDIUM_COST: u32 = 8;
const LOW_COST: u32 = 4;

/// Represents crendentials used to authenticate a user
pub struct UserCredentials {
    user_id: String,
    username: String,
    password: String,
}

impl UserCredentials {
    pub fn verify_password(&self, password: &str) -> Result<bool, UserCredentialsError> {
        Ok(verify(password, &self.password)?)
    }
}

/// Builder for UsersCredential. It hashes the password upon build.
#[derive(Default)]
pub struct UserCredentialsBuilder {
    user_id: Option<String>,
    username: Option<String>,
    password: Option<String>,
    password_encryption_cost: Option<PasswordEncryptionCost>,
}

impl UserCredentialsBuilder {
    /// Sets the user_id for the credentials belong to
    ///
    /// # Arguments
    ///
    /// * `user_id`: unique identifier for the user the credentials belong to
    ///
    pub fn with_user_id(mut self, user_id: &str) -> UserCredentialsBuilder {
        self.user_id = Some(user_id.to_owned());
        self
    }

    /// Sets the username for the credentials
    ///
    /// # Arguments
    ///
    /// * `username`: username that will be used to authenticate the user
    ///
    pub fn with_username(mut self, username: &str) -> UserCredentialsBuilder {
        self.username = Some(username.to_owned());
        self
    }

    /// Sets the password for the credentials
    ///
    /// # Arguments
    ///
    /// * `password`: password that will be used to authenticate the user
    ///
    pub fn with_password(mut self, password: &str) -> UserCredentialsBuilder {
        self.password = Some(password.to_owned());
        self
    }

    /// Sets the cost to encrypt the password for the credentials
    ///
    /// # Arguments
    ///
    /// * `cost`: cost of the password encryption, default is high
    ///
    pub fn with_password_encryption_cost(
        mut self,
        cost: PasswordEncryptionCost,
    ) -> UserCredentialsBuilder {
        self.password_encryption_cost = Some(cost);
        self
    }

    /// Consumes the builder, hashes the password and returns UserCredentials with the hashed
    /// password
    pub fn build(self) -> Result<UserCredentials, UserCredentialsBuilderError> {
        let user_id = self.user_id.ok_or_else(|| {
            UserCredentialsBuilderError::MissingRequiredField("Missing user_id".to_string())
        })?;
        let username = self.username.ok_or_else(|| {
            UserCredentialsBuilderError::MissingRequiredField("Missing user_id".to_string())
        })?;

        let cost = self
            .password_encryption_cost
            .unwrap_or(PasswordEncryptionCost::High);

        let hashed_password = hash(
            self.password.ok_or_else(|| {
                UserCredentialsBuilderError::MissingRequiredField("Missing password".to_string())
            })?,
            cost.to_value(),
        )?;

        Ok(UserCredentials {
            user_id,
            username,
            password: hashed_password,
        })
    }
}

/// Defines methods for CRUD operations and fetching a user’s
/// credentials without defining a storage strategy
pub trait CredentialsStore<T>: Send {
    /// Adds a credential to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `credentials` - Credentials to be added
    ///
    ///
    fn add_credentials(&self, credentials: T) -> Result<(), CredentialsStoreError>;

    /// Replaces a credential of a certain type for a user in the underlying storage with new
    /// credentials. This assumes that the user has only one credential of a certain type in
    /// storage
    ///
    /// # Arguments
    ///
    ///  * `user_id` - The unique identifier of the user credential belongs to
    ///  * `updated_username` - The updated username for the user
    ///  * `updated_password` - The updated password for the user
    ///
    fn update_credentials(
        &self,
        user_id: &str,
        updated_username: &str,
        updated_password: &str,
    ) -> Result<(), CredentialsStoreError>;

    /// Removes a credential from a user from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `user_id` - The unique identifier of the user credential belongs to
    ///
    fn remove_credentials(&self, user_id: &str) -> Result<T, CredentialsStoreError>;

    /// Fetches a credential for a user
    ///
    /// # Arguments
    ///
    ///  * `user_id` - The unique identifier of the user credential belongs to
    ///
    fn fetch_credential_by_user_id(&self, user_id: &str) -> Result<T, CredentialsStoreError>;

    /// Fetches a credential for a user
    ///
    /// # Arguments
    ///
    ///  * `username` - The username the user uses for login
    ///
    fn fetch_credential_by_username(&self, username: &str) -> Result<T, CredentialsStoreError>;

    /// Creates a boxed clone of the implementation, with only the dynamic features of the trait.
    fn clone_boxed(&self) -> Box<dyn CredentialsStore<T>>;
}

impl<T> Clone for Box<dyn CredentialsStore<T>> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Construct a new CredentialsStore for UserCredentials over a connection pool
#[cfg(feature = "diesel")]
pub fn new_store_from_connection_pool(
    connection_pool: crate::database::ConnectionPool,
) -> Result<Box<dyn CredentialsStore<UserCredentials>>, CredentialsStoreError> {
    Ok(Box::new(credentials_store::SplinterCredentialsStore::new(
        connection_pool,
    )))
}

impl From<UserCredentialsModel> for UserCredentials {
    fn from(user_credentials: UserCredentialsModel) -> Self {
        Self {
            user_id: user_credentials.user_id,
            username: user_credentials.username,
            password: user_credentials.password,
        }
    }
}

impl Into<NewUserCredentialsModel> for UserCredentials {
    fn into(self) -> NewUserCredentialsModel {
        NewUserCredentialsModel {
            user_id: self.user_id,
            username: self.username,
            password: self.password,
        }
    }
}

/// Cost to encrypt password. The recommneded value is HIGH. Values LOW and MEDIUM may be used for
/// development and testing as hashing and verifying passwords will be completed faster.
#[derive(Debug, Deserialize, Clone)]
pub enum PasswordEncryptionCost {
    High,
    Medium,
    Low,
}

impl TryFrom<&str> for PasswordEncryptionCost {
    type Error = String;
    fn try_from(value: &str) -> Result<PasswordEncryptionCost, Self::Error> {
        match value.to_lowercase().as_ref() {
            "high" => Ok(PasswordEncryptionCost::High),
            "medium" => Ok(PasswordEncryptionCost::Medium),
            "low" => Ok(PasswordEncryptionCost::Low),
            _ => Err(format!(
                "Invalid cost value {}, must be high, medium or low",
                value
            )),
        }
    }
}

impl PasswordEncryptionCost {
    fn to_value(&self) -> u32 {
        match self {
            PasswordEncryptionCost::High => DEFAULT_COST,
            PasswordEncryptionCost::Medium => MEDIUM_COST,
            PasswordEncryptionCost::Low => LOW_COST,
        }
    }
}
