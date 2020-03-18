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

pub use crate::biome::datastore::models::UserModel;
#[cfg(feature = "diesel")]
pub use error::UserStoreError;

/// Represents a user of a splinter application
#[derive(Serialize)]
pub struct SplinterUser {
    id: String,
}

impl SplinterUser {
    /// Creates a new SplinterUser
    ///
    /// # Arguments
    ///
    /// * `user_id`: unique identifier for the user being created
    ///
    pub fn new(user_id: &str) -> Self {
        SplinterUser {
            id: user_id.to_string(),
        }
    }

    /// Returns the user's id.
    pub fn id(&self) -> String {
        self.id.to_string()
    }
}

#[cfg(feature = "diesel")]
impl From<UserModel> for SplinterUser {
    fn from(user: UserModel) -> Self {
        SplinterUser { id: user.id }
    }
}

/// Defines methods for CRUD operations and fetching and listing users
/// without defining a storage strategy
pub trait UserStore<T> {
    /// Adds a user to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `user` - The user to be added
    ///
    ///
    fn add_user(&mut self, user: T) -> Result<(), UserStoreError>;

    /// Updates a user information in the underling storage
    ///
    /// # Arguments
    ///
    ///  * `user` - The user with the updated information
    ///
    fn update_user(&mut self, updated_user: T) -> Result<(), UserStoreError>;

    /// Removes a user from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `id` - The unique id of the user to be removed
    ///
    fn remove_user(&mut self, id: &str) -> Result<(), UserStoreError>;

    /// Fetches a user from the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `id` - The unique id of the user to be returned
    ///
    fn fetch_user(&self, id: &str) -> Result<T, UserStoreError>;

    /// List all users from the underlying storage
    ///
    fn list_users(&self) -> Result<Vec<T>, UserStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Clone)]
    struct MockUser {
        pub id: String,
    }

    struct MockUserStore {
        users: HashMap<String, MockUser>,
    }

    impl MockUserStore {
        fn new() -> Self {
            MockUserStore {
                users: HashMap::new(),
            }
        }
    }

    impl UserStore<MockUser> for MockUserStore {
        fn add_user(&mut self, user: MockUser) -> Result<(), UserStoreError> {
            self.users.insert(user.id.clone(), user);
            Ok(())
        }

        fn update_user(&mut self, updated_user: MockUser) -> Result<(), UserStoreError> {
            self.users.insert(updated_user.id.clone(), updated_user);
            Ok(())
        }

        fn remove_user(&mut self, id: &str) -> Result<(), UserStoreError> {
            self.users.remove(id);
            Ok(())
        }

        fn fetch_user(&self, id: &str) -> Result<MockUser, UserStoreError> {
            Ok(self.users.get(id).map(MockUser::clone).unwrap())
        }

        fn list_users(&self) -> Result<Vec<MockUser>, UserStoreError> {
            Ok(self.users.iter().map(|(_, user)| user.clone()).collect())
        }
    }

    #[test]
    fn test_add_user() {
        let user = MockUser {
            id: "user_1".to_string(),
        };

        let mut store = MockUserStore::new();

        assert!(store.add_user(user).is_ok());
    }

    #[test]
    fn test_update_user() {
        let mut user = MockUser {
            id: "user_1".to_string(),
        };
        let mut store = MockUserStore::new();
        store.add_user(user.clone()).unwrap();

        user.id = "user_update".to_string();

        assert!(store.update_user(user).is_ok());
    }

    #[test]
    fn test_fetch_user() {
        let user = MockUser {
            id: "user_1".to_string(),
        };
        let mut store = MockUserStore::new();
        store.add_user(user.clone()).unwrap();
        let store_user = store.fetch_user("user_1").unwrap();

        assert_eq!(user.id, store_user.id);
    }

    #[test]
    fn test_list_user() {
        let user = MockUser {
            id: "user_1".to_string(),
        };
        let mut store = MockUserStore::new();
        store.add_user(user.clone()).unwrap();
        let store_users = store.list_users().unwrap();

        assert_eq!(user.id, store_users[0].id);
    }

    #[test]
    fn test_remove_user() {
        let user = MockUser {
            id: "user_1".to_string(),
        };
        let mut store = MockUserStore::new();
        store.add_user(user.clone()).unwrap();
        let store_user = store.fetch_user("user_1").unwrap();

        assert_eq!(user.id, store_user.id);

        store.remove_user("user_1").unwrap();

        let user_count = store.list_users().unwrap().len();

        assert_eq!(0, user_count);
    }
}
