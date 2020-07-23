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

use crate::biome::credentials::store::{
    error::CredentialsStoreError, Credentials, CredentialsBuilder, CredentialsStore,
    PasswordEncryptionCost, UsernameId,
};

#[derive(Default, Clone)]
pub struct MemoryCredentialsStore {
    inner: Arc<Mutex<HashMap<String, Credentials>>>,
}

impl MemoryCredentialsStore {
    pub fn new() -> Self {
        MemoryCredentialsStore {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl CredentialsStore for MemoryCredentialsStore {
    fn add_credentials(&self, credentials: Credentials) -> Result<(), CredentialsStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        inner.insert(credentials.user_id.clone(), credentials);
        Ok(())
    }

    fn update_credentials(
        &self,
        user_id: &str,
        updated_username: &str,
        updated_password: &str,
        password_encryption_cost: PasswordEncryptionCost,
    ) -> Result<(), CredentialsStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        if inner.contains_key(user_id) {
            let new_credentials = CredentialsBuilder::default()
                .with_user_id(user_id)
                .with_username(updated_username)
                .with_password(updated_password)
                .with_password_encryption_cost(password_encryption_cost)
                .build()
                .map_err(|err| CredentialsStoreError::OperationError {
                    context: "Failed to build updated credentials".to_string(),
                    source: err.into(),
                })?;
            inner.insert(user_id.into(), new_credentials);
            Ok(())
        } else {
            Err(CredentialsStoreError::NotFoundError(format!(
                "User with user id {} not found",
                user_id
            )))
        }
    }

    fn remove_credentials(&self, user_id: &str) -> Result<(), CredentialsStoreError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        if inner.remove(user_id).is_some() {
            Ok(())
        } else {
            Err(CredentialsStoreError::NotFoundError(format!(
                "User with user id {} not found",
                user_id
            )))
        }
    }

    fn fetch_credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        if let Some(user) = inner.get(user_id) {
            Ok(user.clone())
        } else {
            Err(CredentialsStoreError::NotFoundError(format!(
                "User with user_id {} not found.",
                user_id
            )))
        }
    }

    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        for (_, v) in inner.iter() {
            if v.username == username {
                return Ok(v.clone());
            }
        }
        Err(CredentialsStoreError::NotFoundError(format!(
            "User with username {} not found.",
            username
        )))
    }

    fn fetch_username_by_id(&self, user_id: &str) -> Result<UsernameId, CredentialsStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        for (_, v) in inner.iter() {
            if v.user_id == user_id {
                return Ok(UsernameId {
                    username: v.username.clone(),
                    user_id: v.user_id.clone(),
                });
            }
        }
        Err(CredentialsStoreError::NotFoundError(format!(
            "User with id {} not found.",
            user_id
        )))
    }

    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| CredentialsStoreError::StorageError {
                context: "Cannot access credentials: mutex lock poisoned".to_string(),
                source: None,
            })?;
        Ok(inner
            .iter()
            .map(|(_, v)| UsernameId {
                username: v.username.clone(),
                user_id: v.user_id.clone(),
            })
            .collect())
    }
}
