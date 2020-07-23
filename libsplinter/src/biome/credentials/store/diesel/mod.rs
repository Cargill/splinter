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
pub(in crate::biome) mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    Credentials, CredentialsStore, CredentialsStoreError, PasswordEncryptionCost, UsernameId,
};

use models::CredentialsModel;
use operations::add_credentials::CredentialsStoreAddCredentialsOperation as _;
use operations::fetch_credential_by_id::CredentialsStoreFetchCredentialByIdOperation as _;
use operations::fetch_credential_by_username::CredentialsStoreFetchCredentialByUsernameOperation as _;
use operations::fetch_username::CredentialsStoreFetchUsernameOperation as _;
use operations::list_usernames::CredentialsStoreListUsernamesOperation as _;
use operations::remove_credentials::CredentialsStoreRemoveCredentialsOperation as _;
use operations::update_credentials::CredentialsStoreUpdateCredentialsOperation as _;
use operations::CredentialsStoreOperations;

/// Manages creating, updating and fetching SplinterCredentials from the database
pub struct DieselCredentialsStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselCredentialsStore<C> {
    /// Creates a new DieselCredentialsStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselCredentialsStore { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl CredentialsStore for DieselCredentialsStore<diesel::pg::PgConnection> {
    fn add_credentials(&self, credentials: Credentials) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).add_credentials(credentials)
    }

    fn update_credentials(
        &self,
        user_id: &str,
        username: &str,
        password: &str,
        password_encryption_cost: PasswordEncryptionCost,
    ) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).update_credentials(
            user_id,
            username,
            password,
            password_encryption_cost,
        )
    }

    fn remove_credentials(&self, user_id: &str) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).remove_credentials(user_id)
    }

    fn fetch_credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_id(user_id)
    }

    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_username(username)
    }

    fn fetch_username_by_id(&self, user_id: &str) -> Result<UsernameId, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).fetch_username_by_id(user_id)
    }

    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).list_usernames()
    }
}

#[cfg(feature = "sqlite")]
impl CredentialsStore for DieselCredentialsStore<diesel::sqlite::SqliteConnection> {
    fn add_credentials(&self, credentials: Credentials) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).add_credentials(credentials)
    }

    fn update_credentials(
        &self,
        user_id: &str,
        username: &str,
        password: &str,
        password_encryption_cost: PasswordEncryptionCost,
    ) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).update_credentials(
            user_id,
            username,
            password,
            password_encryption_cost,
        )
    }

    fn remove_credentials(&self, user_id: &str) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).remove_credentials(user_id)
    }

    fn fetch_credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_id(user_id)
    }

    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_username(username)
    }

    fn fetch_username_by_id(&self, user_id: &str) -> Result<UsernameId, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).fetch_username_by_id(user_id)
    }

    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).list_usernames()
    }
}

impl From<CredentialsModel> for UsernameId {
    fn from(user_credentials: CredentialsModel) -> Self {
        Self {
            user_id: user_credentials.user_id,
            username: user_credentials.username,
        }
    }
}

impl From<CredentialsModel> for Credentials {
    fn from(user_credentials: CredentialsModel) -> Self {
        Self {
            user_id: user_credentials.user_id,
            username: user_credentials.username,
            password: user_credentials.password,
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::{credentials::store::CredentialsBuilder, migrations::run_sqlite_migrations};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports fetching
    /// credentials by user ID.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add some credentials.
    /// 4. Verify that the `fetch_credential_by_user_id` method returns correct values for all
    ///    existing credentials.
    /// 5. Verify that the `fetch_credential_by_user_id` method returns a
    ///    `CredentialsStoreError::NotFoundError` for non-existent credentials.
    #[test]
    fn sqlite_fetch_credential_by_user_id() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred1 = CredentialsBuilder::default()
            .with_user_id("id1")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred1");
        store
            .add_credentials(cred1.clone())
            .expect("Failed to add cred1");
        let cred2 = CredentialsBuilder::default()
            .with_user_id("id2")
            .with_username("user2")
            .with_password("pwd2")
            .with_password_encryption_cost(PasswordEncryptionCost::Medium)
            .build()
            .expect("Failed to build cred2");
        store
            .add_credentials(cred2.clone())
            .expect("Failed to add cred2");
        let cred3 = CredentialsBuilder::default()
            .with_user_id("id3")
            .with_username("user3")
            .with_password("pwd3")
            .with_password_encryption_cost(PasswordEncryptionCost::High)
            .build()
            .expect("Failed to build cred3");
        store
            .add_credentials(cred3.clone())
            .expect("Failed to add cred3");

        assert_eq!(
            store
                .fetch_credential_by_user_id("id1")
                .expect("Failed to fetch cred1"),
            cred1,
        );
        assert_eq!(
            store
                .fetch_credential_by_user_id("id2")
                .expect("Failed to fetch cred2"),
            cred2,
        );
        assert_eq!(
            store
                .fetch_credential_by_user_id("id3")
                .expect("Failed to fetch cred3"),
            cred3,
        );

        match store.fetch_credential_by_user_id("cred4") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(CredentialsStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports fetching
    /// credentials by username.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add some credentials.
    /// 4. Verify that the `fetch_credential_by_username` method returns correct values for all
    ///    existing credentials.
    /// 5. Verify that the `fetch_credential_by_username` method returns a
    ///    `CredentialsStoreError::NotFoundError` for non-existent credentials.
    #[test]
    fn sqlite_fetch_credential_by_username() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred1 = CredentialsBuilder::default()
            .with_user_id("id1")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred1");
        store
            .add_credentials(cred1.clone())
            .expect("Failed to add cred1");
        let cred2 = CredentialsBuilder::default()
            .with_user_id("id2")
            .with_username("user2")
            .with_password("pwd2")
            .with_password_encryption_cost(PasswordEncryptionCost::Medium)
            .build()
            .expect("Failed to build cred2");
        store
            .add_credentials(cred2.clone())
            .expect("Failed to add cred2");
        let cred3 = CredentialsBuilder::default()
            .with_user_id("id3")
            .with_username("user3")
            .with_password("pwd3")
            .with_password_encryption_cost(PasswordEncryptionCost::High)
            .build()
            .expect("Failed to build cred3");
        store
            .add_credentials(cred3.clone())
            .expect("Failed to add cred3");

        assert_eq!(
            store
                .fetch_credential_by_username("user1")
                .expect("Failed to fetch cred1"),
            cred1,
        );
        assert_eq!(
            store
                .fetch_credential_by_username("user2")
                .expect("Failed to fetch cred2"),
            cred2,
        );
        assert_eq!(
            store
                .fetch_credential_by_username("user3")
                .expect("Failed to fetch cred3"),
            cred3,
        );

        match store.fetch_credential_by_username("user4") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(CredentialsStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports fetching
    /// usernames by IDs.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add some credentials.
    /// 4. Verify that the `fetch_username_by_id` method returns correct values for all existing
    ///    credentials.
    /// 5. Verify that the `fetch_username_by_id` method returns a
    ///    `CredentialsStoreError::NotFoundError` for non-existent credentials.
    #[test]
    fn sqlite_fetch_username_by_id() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred1 = CredentialsBuilder::default()
            .with_user_id("id1")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred1");
        store.add_credentials(cred1).expect("Failed to add cred1");
        let cred2 = CredentialsBuilder::default()
            .with_user_id("id2")
            .with_username("user2")
            .with_password("pwd2")
            .with_password_encryption_cost(PasswordEncryptionCost::Medium)
            .build()
            .expect("Failed to build cred2");
        store.add_credentials(cred2).expect("Failed to add cred2");
        let cred3 = CredentialsBuilder::default()
            .with_user_id("id3")
            .with_username("user3")
            .with_password("pwd3")
            .with_password_encryption_cost(PasswordEncryptionCost::High)
            .build()
            .expect("Failed to build cred3");
        store.add_credentials(cred3).expect("Failed to add cred3");

        assert_eq!(
            store
                .fetch_username_by_id("id1")
                .expect("Failed to fetch id1"),
            UsernameId {
                username: "user1".into(),
                user_id: "id1".into(),
            },
        );
        assert_eq!(
            store
                .fetch_username_by_id("id2")
                .expect("Failed to fetch id2"),
            UsernameId {
                username: "user2".into(),
                user_id: "id2".into(),
            },
        );
        assert_eq!(
            store
                .fetch_username_by_id("id3")
                .expect("Failed to fetch id3"),
            UsernameId {
                username: "user3".into(),
                user_id: "id3".into(),
            },
        );

        match store.fetch_username_by_id("id4") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(CredentialsStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports listing usernames.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add some credentials.
    /// 4. Verify that the `list_usernames` method returns correct values for all credentials.
    #[test]
    fn sqlite_list_usernames() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred1 = CredentialsBuilder::default()
            .with_user_id("id1")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred1");
        store.add_credentials(cred1).expect("Failed to add cred1");
        let cred2 = CredentialsBuilder::default()
            .with_user_id("id2")
            .with_username("user2")
            .with_password("pwd2")
            .with_password_encryption_cost(PasswordEncryptionCost::Medium)
            .build()
            .expect("Failed to build cred2");
        store.add_credentials(cred2).expect("Failed to add cred2");
        let cred3 = CredentialsBuilder::default()
            .with_user_id("id3")
            .with_username("user3")
            .with_password("pwd3")
            .with_password_encryption_cost(PasswordEncryptionCost::High)
            .build()
            .expect("Failed to build cred3");
        store.add_credentials(cred3).expect("Failed to add cred3");

        let usernames = store.list_usernames().expect("Failed to list usernames");
        assert_eq!(usernames.len(), 3);
        assert!(usernames.contains(&UsernameId {
            username: "user1".into(),
            user_id: "id1".into(),
        }));
        assert!(usernames.contains(&UsernameId {
            username: "user2".into(),
            user_id: "id2".into(),
        }));
        assert!(usernames.contains(&UsernameId {
            username: "user3".into(),
            user_id: "id3".into(),
        }));
    }

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports updating
    /// credentials.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add a credential and verify its value.
    /// 4. Update the credential and verify that the username and password are updated in the
    ///    store.
    #[test]
    fn sqlite_update() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred = CredentialsBuilder::default()
            .with_user_id("id")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred");
        store
            .add_credentials(cred.clone())
            .expect("Failed to add cred");
        assert_eq!(
            store
                .fetch_credential_by_user_id("id")
                .expect("Failed to fetch cred"),
            cred,
        );

        store
            .update_credentials("id", "user2", "pwd2", PasswordEncryptionCost::Low)
            .expect("Failed to update cred");
        let cred = store
            .fetch_credential_by_user_id("id")
            .expect("Failed to fetch cred");
        assert_eq!(cred.username, "user2");
        assert!(cred
            .verify_password("pwd2")
            .expect("Failed to verify password"));
    }

    /// Verify that a SQLite-backed `DieselCredentialsStore` correctly supports removing
    /// credentials.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselCredentialsStore`.
    /// 3. Add some credentials.
    /// 4. Remove a credential and verify that the credential no longer appears with any of the
    ///    fetch or list methods.
    #[test]
    fn sqlite_remove() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselCredentialsStore::new(pool);

        let cred1 = CredentialsBuilder::default()
            .with_user_id("id1")
            .with_username("user1")
            .with_password("pwd1")
            .with_password_encryption_cost(PasswordEncryptionCost::Low)
            .build()
            .expect("Failed to build cred1");
        store.add_credentials(cred1).expect("Failed to add cred1");
        let cred2 = CredentialsBuilder::default()
            .with_user_id("id2")
            .with_username("user2")
            .with_password("pwd2")
            .with_password_encryption_cost(PasswordEncryptionCost::Medium)
            .build()
            .expect("Failed to build cred2");
        store.add_credentials(cred2).expect("Failed to add cred2");
        let cred3 = CredentialsBuilder::default()
            .with_user_id("id3")
            .with_username("user3")
            .with_password("pwd3")
            .with_password_encryption_cost(PasswordEncryptionCost::High)
            .build()
            .expect("Failed to build cred3");
        store.add_credentials(cred3).expect("Failed to add cred3");

        store
            .remove_credentials("id3")
            .expect("Failed to remove cred3");
        match store.fetch_credential_by_user_id("id3") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        match store.fetch_credential_by_username("user3") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        match store.fetch_username_by_id("id3") {
            Err(CredentialsStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(KeyStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        let usernames = store.list_usernames().expect("Failed to list usernames");
        assert_eq!(usernames.len(), 2);
        assert!(!usernames.contains(&UsernameId {
            username: "user3".into(),
            user_id: "id3".into(),
        }));
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
