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

#[cfg(feature = "diesel")]
pub(crate) mod diesel;
mod error;
pub(in crate::biome) mod memory;

pub use error::RefreshTokenError;

/// Defines methods for CRUD operations for handling refresh tokens
pub trait RefreshTokenStore: Send + Sync {
    /// Adds a refresh to token to underlying storage
    ///
    /// # Arguments
    ///
    ///   * `user_id` - The user whom which the token is for
    ///   * `token` - A refresh token for user
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError>;

    /// Removes a token in underlying storage
    ///
    /// # Arguments
    ///
    ///   * `user_id` - The user whom which the token is for
    fn remove_token(&self, user_id: &str) -> Result<(), RefreshTokenError>;

    /// Update a refresh to token to underlying storage
    ///
    /// # Arguments
    ///
    ///   * `user_id` - The user whom which the token is for
    ///   * `token` - A refresh token for user
    fn update_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError>;

    /// Fetch a token from underlying storage
    ///
    /// # Arguments
    ///
    ///   * `user_id` - The user whom which the token is for
    fn fetch_token(&self, user_id: &str) -> Result<String, RefreshTokenError>;
}
