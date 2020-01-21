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

pub mod database;
mod error;
#[cfg(feature = "rest-api")]
pub(in crate::biome) mod rest_resources;
pub mod store;

use std::convert::TryFrom;

use bcrypt::{hash, verify, DEFAULT_COST};

use database::models::{NewUserCredentialsModel, UserCredentialsModel};

pub use error::{UserCredentialsBuilderError, UserCredentialsError};

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
