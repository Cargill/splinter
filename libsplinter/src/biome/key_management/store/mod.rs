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
pub(in crate::biome) mod diesel;
pub mod error;
pub(in crate::biome) mod memory;

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::store::PasswordEncryptionCost;

use super::Key;

pub use error::KeyStoreError;

/// Defines methods for CRUD operations and fetching and listing keys
/// without defining a storage strategy
pub trait KeyStore: Sync + Send {
    /// Adds a key to the underlying storage
    ///
    /// # Arguments
    ///
    ///  * `key` - The key to be added
    fn add_key(&self, key: Key) -> Result<(), KeyStoreError>;

    /// Updates a key information in the underling storage
    ///
    /// # Arguments
    ///
    /// * `public_key`: The public key of the key record to be updated.
    /// * `user_id`: The ID owner of the key record to be updated.
    /// * `new_display_name`: The new display name of the key record.
    fn update_key(
        &self,
        public_key: &str,
        user_id: &str,
        new_display_name: &str,
    ) -> Result<(), KeyStoreError>;

    /// Removes a key from the underlying storage
    ///
    /// # Arguments
    ///
    /// * `public_key`: The public key of the key record to be removed.
    /// * `user_id`: The ID owner of the key record to be removed.
    fn remove_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError>;

    /// Fetches a key from the underlying storage
    ///
    /// # Arguments
    ///
    /// * `public_key`: The public key of the key record to be fetched.
    /// * `user_id`: The ID owner of the key record to be fetched.
    fn fetch_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError>;

    /// List all keys from the underlying storage
    ///
    /// # Arguments
    ///
    /// * `user_id`: The ID owner of the key records to list.
    fn list_keys(&self, user_id: Option<&str>) -> Result<Vec<Key>, KeyStoreError>;

    #[cfg(feature = "biome-credentials")]
    /// Updates keys and the associated user's password in the underlying storage
    ///
    /// # Arguments
    ///
    /// * `user_id`: The ID owner of the key records to list.
    /// * `updated_password` - The updated password for the user
    /// * `keys` - The keys to be replaced
    ///
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        password_encryption_cost: PasswordEncryptionCost,
        keys: &[Key],
    ) -> Result<(), KeyStoreError>;
}

impl<KS> KeyStore for Box<KS>
where
    KS: KeyStore + ?Sized,
{
    fn add_key(&self, key: Key) -> Result<(), KeyStoreError> {
        (**self).add_key(key)
    }

    fn update_key(
        &self,
        public_key: &str,
        user_id: &str,
        new_display_name: &str,
    ) -> Result<(), KeyStoreError> {
        (**self).update_key(public_key, user_id, new_display_name)
    }

    fn remove_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        (**self).remove_key(public_key, user_id)
    }

    fn fetch_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        (**self).fetch_key(public_key, user_id)
    }

    fn list_keys(&self, user_id: Option<&str>) -> Result<Vec<Key>, KeyStoreError> {
        (**self).list_keys(user_id)
    }

    #[cfg(feature = "biome-credentials")]
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        password_encryption_cost: PasswordEncryptionCost,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        (**self).update_keys_and_password(user_id, updated_password, password_encryption_cost, keys)
    }
}
