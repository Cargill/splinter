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

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::store::{memory::MemoryCredentialsStore, CredentialsStore};
use crate::biome::user::store::{error::UserStoreError, User, UserStore};

///Implementation of UserStore that stores Users in memory. Useful for when
///persistence isn't necessary.
#[derive(Clone, Default)]
pub struct MemoryUserStore {
    inner: Arc<Mutex<HashMap<String, User>>>,
    #[cfg(feature = "biome-credentials")]
    credentials_store: MemoryCredentialsStore,
}

impl MemoryUserStore {
    #[cfg(feature = "biome-credentials")]
    pub fn new(credentials_store: MemoryCredentialsStore) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            credentials_store,
        }
    }

    #[cfg(not(feature = "biome-credentials"))]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl UserStore for MemoryUserStore {
    fn add_user(&self, user: User) -> Result<(), UserStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| UserStoreError::StorageError {
                context: "Cannot access user store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        inner.insert(user.id().to_string(), user);
        Ok(())
    }

    fn update_user(&self, updated_user: User) -> Result<(), UserStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| UserStoreError::StorageError {
                context: "Cannot access user store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if inner.contains_key(updated_user.id()) {
            inner.insert(updated_user.id().to_string(), updated_user);
            Ok(())
        } else {
            Err(UserStoreError::NotFoundError(format!(
                "User {} not found.",
                updated_user.id()
            )))
        }
    }

    fn remove_user(&self, id: &str) -> Result<(), UserStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| UserStoreError::StorageError {
                context: "Cannot access user store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if inner.remove(id).is_some() {
            #[cfg(feature = "biome-credentials")]
            self.credentials_store
                .remove_credentials(id)
                .map_err(|err| UserStoreError::QueryError {
                    context: format!("Cannot delete user {} from credentials store", id),
                    source: Box::new(err),
                })?;

            Ok(())
        } else {
            Err(UserStoreError::NotFoundError(format!(
                "User {} not found.",
                id
            )))
        }
    }

    fn fetch_user(&self, id: &str) -> Result<User, UserStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| UserStoreError::StorageError {
                context: "Cannot access user store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        if let Some(user) = inner.get(id) {
            Ok(user.clone())
        } else {
            Err(UserStoreError::NotFoundError(format!(
                "User {} not found.",
                id
            )))
        }
    }

    fn list_users(&self) -> Result<Vec<User>, UserStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| UserStoreError::StorageError {
                context: "Cannot access user store: mutex lock poisoned".to_string(),
                source: None,
            })?;

        Ok(inner.iter().map(|(_, v)| v.clone()).collect())
    }

    fn clone_box(&self) -> Box<dyn UserStore> {
        Box::new(self.clone())
    }
}
