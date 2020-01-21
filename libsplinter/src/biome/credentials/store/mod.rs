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

mod diesel;
mod error;

use super::UserCredentials;

pub use error::CredentialsStoreError;

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
pub fn from_connection_pool(
    connection_pool: crate::database::ConnectionPool,
) -> Result<Box<dyn CredentialsStore<UserCredentials>>, CredentialsStoreError> {
    Ok(Box::new(diesel::SplinterCredentialsStore::new(
        connection_pool,
    )))
}
