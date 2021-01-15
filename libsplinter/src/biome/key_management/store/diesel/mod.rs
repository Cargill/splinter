// Copyright 2018-2021 Cargill Incorporated
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

pub(in crate::biome) mod models;
mod operations;
mod schema;

use crate::biome::key_management::store::{KeyStore, KeyStoreError};
use crate::biome::key_management::Key;
use crate::database::ConnectionPool;

#[cfg(feature = "biome-credentials")]
use operations::update_keys_and_password::KeyStoreUpdateKeysAndPasswordOperation as _;
use operations::{
    fetch_key::KeyStoreFetchKeyOperation as _, insert_key::KeyStoreInsertKeyOperation as _,
    list_keys::KeyStoreListKeysOperation as _, list_keys::KeyStoreListKeysWithUserIDOperation as _,
    remove_key::KeyStoreRemoveKeyOperation as _, update_key::KeyStoreUpdateKeyOperation as _,
    KeyStoreOperations,
};

/// Manages creating, updating and fetching keys from a PostgreSQL database.
pub struct DieselKeyStore {
    pub connection_pool: ConnectionPool,
}

impl DieselKeyStore {
    /// Creates a new DieselKeyStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the PostgreSQL database
    ///
    pub fn new(connection_pool: ConnectionPool) -> Self {
        DieselKeyStore { connection_pool }
    }
}

impl KeyStore for DieselKeyStore {
    fn add_key(&self, key: Key) -> Result<(), KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).insert_key(key)
    }

    fn update_key(
        &self,
        public_key: &str,
        user_id: &str,
        new_display_name: &str,
    ) -> Result<(), KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).update_key(
            public_key,
            user_id,
            new_display_name,
        )
    }

    fn remove_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).remove_key(public_key, user_id)
    }

    fn fetch_key(&self, public_key: &str, user_id: &str) -> Result<Key, KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).fetch_key(public_key, user_id)
    }

    fn list_keys(&self, user_id: Option<&str>) -> Result<Vec<Key>, KeyStoreError> {
        match user_id {
            Some(user_id) => KeyStoreOperations::new(&*self.connection_pool.get()?)
                .list_keys_with_user_id(user_id),
            None => KeyStoreOperations::new(&*self.connection_pool.get()?).list_keys(),
        }
    }

    #[cfg(feature = "biome-credentials")]
    fn update_keys_and_password(
        &self,
        user_id: &str,
        updated_password: &str,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).update_keys_and_password(
            user_id,
            updated_password,
            keys,
        )
    }
}
