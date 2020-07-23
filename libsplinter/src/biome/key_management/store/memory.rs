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
use crate::biome::credentials::store::{
    memory::MemoryCredentialsStore, CredentialsStore, PasswordEncryptionCost,
};
use crate::biome::key_management::{
    store::{error::KeyStoreError, KeyStore},
    Key,
};

#[derive(Default, Clone)]
pub struct MemoryKeyStore {
    inner: Arc<Mutex<HashMap<(String, String), Key>>>,
    #[cfg(feature = "biome-credentials")]
    credentials_store: MemoryCredentialsStore,
}

impl MemoryKeyStore {
    #[cfg(feature = "biome-credentials")]
    pub fn new(credentials_store: MemoryCredentialsStore) -> Self {
        MemoryKeyStore {
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

impl KeyStore for MemoryKeyStore {
    fn add_key(&self, key: Key) -> Result<(), KeyStoreError> {
        let mut inner = self.inner.lock().map_err(|_| KeyStoreError::StorageError {
            context: "Cannot access key store: mutex lock poisoned".to_string(),
            source: None,
        })?;
        inner.insert((key.user_id.clone(), key.public_key.clone()), key);
        Ok(())
    }

    fn update_key(
        &self,
        public_key: &str,
        user_id: &str,
        new_display_name: &str,
    ) -> Result<(), KeyStoreError> {
        let mut inner = self.inner.lock().map_err(|_| KeyStoreError::StorageError {
            context: "Cannot access key store: mutex lock poisoned".to_string(),
            source: None,
        })?;

        if let Some(key) = inner.get_mut(&(user_id.into(), public_key.into())) {
            key.display_name = new_display_name.to_string();
            Ok(())
        } else {
            Err(KeyStoreError::NotFoundError(format!(
                "Key with user id {} not found",
                user_id
            )))
        }
    }

    fn remove_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        let mut inner = self.inner.lock().map_err(|_| KeyStoreError::StorageError {
            context: "Cannot access key store: mutex lock poisoned".to_string(),
            source: None,
        })?;

        if let Some(key) = inner.remove(&(user_id.to_string(), public_key.to_string())) {
            Ok(key)
        } else {
            Err(KeyStoreError::NotFoundError(format!(
                "Key with user id {} not found",
                user_id
            )))
        }
    }

    fn fetch_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        let inner = self.inner.lock().map_err(|_| KeyStoreError::StorageError {
            context: "Cannot access key store: mutex lock poisoned".to_string(),
            source: None,
        })?;

        if let Some(key) = inner.get(&(user_id.to_string(), public_key.to_string())) {
            Ok(key.clone())
        } else {
            Err(KeyStoreError::NotFoundError(format!(
                "Key with user id {} not found",
                user_id
            )))
        }
    }

    fn list_keys(&self, user_id: Option<&str>) -> Result<Vec<Key>, KeyStoreError> {
        let inner = self.inner.lock().map_err(|_| KeyStoreError::StorageError {
            context: "Cannot access key store: mutex lock poisoned".to_string(),
            source: None,
        })?;
        if let Some(user_id) = user_id {
            Ok(inner
                .iter()
                .filter(|((id, _), _)| id == user_id)
                .map(|(_, v)| v.clone())
                .collect())
        } else {
            Ok(inner.iter().map(|(_, v)| v.clone()).collect())
        }
    }

    #[cfg(feature = "biome-credentials")]
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        password_encryption_cost: PasswordEncryptionCost,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        for key in keys {
            self.update_key(&key.public_key, &key.user_id, &key.display_name)?;
        }

        let creds = self
            .credentials_store
            .fetch_credential_by_user_id(&user_id)
            .map_err(|err| KeyStoreError::QueryError {
                context: "Cannot find user in credentials store".to_string(),
                source: Box::new(err),
            })?;

        self.credentials_store
            .update_credentials(
                user_id,
                &creds.username,
                updated_password,
                password_encryption_cost,
            )
            .map_err(|err| KeyStoreError::QueryError {
                context: "Cannot update credentials store".to_string(),
                source: Box::new(err),
            })?;

        Ok(())
    }
}
