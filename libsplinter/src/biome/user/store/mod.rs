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

//! Defines a basic representation of a user and provides an API to manage users.

#[cfg(feature = "diesel")]
pub(in crate::biome) mod diesel;
mod error;
pub(in crate::biome) mod memory;

pub use error::UserStoreError;

/// Represents a user of a splinter application
#[derive(Clone, Serialize)]
pub struct User {
    id: String,
}

impl User {
    /// Creates a new User
    ///
    /// # Arguments
    ///
    /// * `user_id`: unique identifier for the user being created
    pub fn new(user_id: &str) -> Self {
        User {
            id: user_id.to_string(),
        }
    }

    /// Returns the user's id.
    pub fn id(&self) -> String {
        self.id.to_string()
    }
}

/// Defines methods for CRUD operations and fetching and listing users
/// without defining a storage strategy
pub trait UserStore: Send + Sync {
    /// Adds a user to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `user` - The user to be added
    fn add_user(&self, user: User) -> Result<(), UserStoreError>;

    /// Updates a user information in the underling storage
    ///
    /// # Arguments
    ///
    ///  * `user` - The user with the updated information
    fn update_user(&self, updated_user: User) -> Result<(), UserStoreError>;

    /// Removes a user from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `id` - The unique id of the user to be removed
    fn remove_user(&self, id: &str) -> Result<(), UserStoreError>;

    /// Fetches a user from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `id` - The unique id of the user to be returned
    fn fetch_user(&self, id: &str) -> Result<User, UserStoreError>;

    /// List all users from the underlying storage
    fn list_users(&self) -> Result<Vec<User>, UserStoreError>;
}

pub trait CloneBoxUserStore: UserStore {
    fn clone_box(&self) -> Box<dyn CloneBoxUserStore>;
}

impl Clone for Box<dyn CloneBoxUserStore> {
    fn clone(&self) -> Box<dyn CloneBoxUserStore> {
        self.clone_box()
    }
}

impl<US> UserStore for Box<US>
where
    US: UserStore + ?Sized,
{
    fn add_user(&self, user: User) -> Result<(), UserStoreError> {
        (**self).add_user(user)
    }

    fn update_user(&self, updated_user: User) -> Result<(), UserStoreError> {
        (**self).update_user(updated_user)
    }

    fn remove_user(&self, id: &str) -> Result<(), UserStoreError> {
        (**self).remove_user(id)
    }

    fn fetch_user(&self, id: &str) -> Result<User, UserStoreError> {
        (**self).fetch_user(id)
    }

    fn list_users(&self) -> Result<Vec<User>, UserStoreError> {
        (**self).list_users()
    }
}
