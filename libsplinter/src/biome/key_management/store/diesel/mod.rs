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

pub(in crate::biome) mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::store::PasswordEncryptionCost;
use crate::biome::key_management::store::{KeyStore, KeyStoreError};
use crate::biome::key_management::Key;

#[cfg(feature = "biome-credentials")]
use operations::update_keys_and_password::KeyStoreUpdateKeysAndPasswordOperation as _;
use operations::{
    fetch_key::KeyStoreFetchKeyOperation as _, insert_key::KeyStoreInsertKeyOperation as _,
    list_keys::KeyStoreListKeysOperation as _, list_keys::KeyStoreListKeysWithUserIDOperation as _,
    remove_key::KeyStoreRemoveKeyOperation as _, update_key::KeyStoreUpdateKeyOperation as _,
    KeyStoreOperations,
};

/// Manages creating, updating and fetching keys from a database.
pub struct DieselKeyStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselKeyStore<C> {
    /// Creates a new DieselKeyStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    ///
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselKeyStore { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl KeyStore for DieselKeyStore<diesel::pg::PgConnection> {
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
        password_encryption_cost: PasswordEncryptionCost,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).update_keys_and_password(
            user_id,
            updated_password,
            password_encryption_cost,
            keys,
        )
    }
}

#[cfg(feature = "sqlite")]
impl KeyStore for DieselKeyStore<diesel::sqlite::SqliteConnection> {
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
        password_encryption_cost: PasswordEncryptionCost,
        keys: &[Key],
    ) -> Result<(), KeyStoreError> {
        KeyStoreOperations::new(&*self.connection_pool.get()?).update_keys_and_password(
            user_id,
            updated_password,
            password_encryption_cost,
            keys,
        )
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    #[cfg(feature = "biome-credentials")]
    use crate::biome::credentials::store::{
        diesel::DieselCredentialsStore, CredentialsBuilder, CredentialsStore,
        PasswordEncryptionCost,
    };
    use crate::biome::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselKeyStore` correctly supports adding and fetching keys.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselKeyStore`.
    /// 3. Add some keys.
    /// 4. Verify that the `fetch_key` method returns correct values for all existing key/user
    ///    pairs.
    /// 5. Verify that the `fetch_key` method returns a `KeyStoreError::NotFoundError` for
    ///    non-existent key/user pairs.
    #[test]
    fn sqlite_add_and_fetch() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselKeyStore::new(pool);

        let key1 = Key::new("pubkey1", "privkey1", "user1", "name1");
        store.add_key(key1.clone()).expect("Failed to add key1");
        let key2 = Key::new("pubkey2", "privkey2", "user1", "name2");
        store.add_key(key2.clone()).expect("Failed to add key2");
        let key3 = Key::new("pubkey3", "privkey3", "user2", "name3");
        store.add_key(key3.clone()).expect("Failed to add key3");

        assert_eq!(
            store
                .fetch_key("pubkey1", "user1")
                .expect("Failed to fetch key1"),
            key1,
        );
        assert_eq!(
            store
                .fetch_key("pubkey2", "user1")
                .expect("Failed to fetch key2"),
            key2,
        );
        assert_eq!(
            store
                .fetch_key("pubkey3", "user2")
                .expect("Failed to fetch key3"),
            key3,
        );

        match store.fetch_key("pubkey4", "user4") {
            Err(KeyStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        match store.fetch_key("pubkey1", "user2") {
            Err(KeyStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselKeyStore` correctly supports listing keys.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselKeyStore`.
    /// 3. Add some keys.
    /// 4. Verify that the `list_keys` method returns the correct values when no user is specified
    ///    (show all keys).
    /// 5. Verify that the `list_keys` method returns the correct values when users are speciefied.
    #[test]
    fn sqlite_list() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselKeyStore::new(pool);

        let key1 = Key::new("pubkey1", "privkey1", "user1", "name1");
        store.add_key(key1.clone()).expect("Failed to add key1");
        let key2 = Key::new("pubkey2", "privkey2", "user1", "name2");
        store.add_key(key2.clone()).expect("Failed to add key2");
        let key3 = Key::new("pubkey3", "privkey3", "user2", "name3");
        store.add_key(key3.clone()).expect("Failed to add key3");

        let keys = store
            .list_keys(None)
            .expect("Failed to list keys for user1");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&key1));
        assert!(keys.contains(&key2));
        assert!(keys.contains(&key3));

        let keys = store
            .list_keys(Some("user1"))
            .expect("Failed to list keys for user1");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&key1));
        assert!(keys.contains(&key2));

        let keys = store
            .list_keys(Some("user2"))
            .expect("Failed to list keys for user2");
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&key3));

        let keys = store
            .list_keys(Some("user3"))
            .expect("Failed to list keys for user3");
        assert!(keys.is_empty());
    }

    /// Verify that a SQLite-backed `DieselKeyStore` correctly supports updating keys.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselKeyStore`.
    /// 3. Add a key and verify its value.
    /// 4. Update the key and verify that the key's display name is updated in the store.
    #[test]
    fn sqlite_update_key() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselKeyStore::new(pool);

        let mut key = Key::new("pubkey", "privkey", "user", "name1");
        store.add_key(key.clone()).expect("Failed to add key");
        assert_eq!(
            store
                .fetch_key("pubkey", "user")
                .expect("Failed to fetch key"),
            key,
        );

        key.display_name = "name2".into();
        store
            .update_key("pubkey", "user", "name2")
            .expect("Failed to update key");
        assert_eq!(
            store
                .fetch_key("pubkey", "user")
                .expect("Failed to fetch key"),
            key,
        );
    }

    /// Verify that a SQLite-backed `DieselKeyStore` correctly supports removing keys.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselKeyStore`.
    /// 3. Add some keys.
    /// 4. Remove a key and verify that the key no longer appears with `fetch_key` or
    ///    `list_keys`.
    #[test]
    fn sqlite_remove() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselKeyStore::new(pool);

        let key1 = Key::new("pubkey1", "privkey1", "user1", "name1");
        store.add_key(key1.clone()).expect("Failed to add key1");
        let key2 = Key::new("pubkey2", "privkey2", "user1", "name2");
        store.add_key(key2.clone()).expect("Failed to add key2");
        let key3 = Key::new("pubkey3", "privkey3", "user2", "name3");
        store.add_key(key3.clone()).expect("Failed to add key3");

        store
            .remove_key("pubkey3", "user2")
            .expect("Failed to remove key3");
        match store.fetch_key("pubkey3", "user2") {
            Err(KeyStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        assert!(store
            .list_keys(Some("user2"))
            .expect("Failed to list keys")
            .is_empty());
    }

    #[cfg(feature = "biome-credentials")]
    /// Verify that a SQLite-backed `DieselKeyStore` correctly supports updating keys and
    /// passwords.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselCredentialsStore` and `DieselKeyStore`.
    /// 3. Add a key and corresponding credentials, then verify the values.
    /// 4. Replace the keys and password with `update_keys_and_password` and verify that the change
    ///    is properly reflected in the stores.
    #[test]
    fn sqlite_update_keys_and_password() {
        let pool = create_connection_pool_and_migrate();

        let cred_store = DieselCredentialsStore::new(pool.clone());
        let key_store = DieselKeyStore::new(pool);

        let key1 = Key::new("pubkey1", "privkey1", "user", "name1");
        key_store.add_key(key1.clone()).expect("Failed to add key1");
        assert_eq!(
            key_store
                .fetch_key("pubkey1", "user")
                .expect("Failed to fetch key1"),
            key1,
        );

        let cred = CredentialsBuilder::default()
            .with_user_id("user")
            .with_username("username")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred");
        cred_store
            .add_credentials(cred.clone())
            .expect("Failed to add cred");
        assert_eq!(
            cred_store
                .fetch_credential_by_user_id("user")
                .expect("Failed to fetch cred"),
            cred,
        );

        let key2 = Key::new("pubkey2", "privkey2", "user", "name2");
        let key3 = Key::new("pubkey3", "privkey3", "user", "name3");
        key_store
            .update_keys_and_password(
                "user",
                "pwd2",
                PasswordEncryptionCost::Low,
                &[key2.clone(), key3.clone()],
            )
            .expect("Failed to update keys and password");

        let keys = key_store
            .list_keys(Some("user"))
            .expect("Failed to list keys");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&key2));
        assert!(keys.contains(&key3));

        let cred = cred_store
            .fetch_credential_by_user_id("user")
            .expect("Failed to fetch cred");
        assert!(cred
            .verify_password("pwd2")
            .expect("Failed to verify password"));
    }

    /// Creates a conneciton pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection insures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
