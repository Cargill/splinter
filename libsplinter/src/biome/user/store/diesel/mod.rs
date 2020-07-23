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

use super::{User, UserStore, UserStoreError};

use operations::add_user::UserStoreAddUserOperation as _;
use operations::delete_user::UserStoreDeleteUserOperation as _;
use operations::fetch_user::UserStoreFetchUserOperation as _;
use operations::list_users::UserStoreListUsersOperation as _;
use operations::update_user::UserStoreUpdateUserOperation as _;
use operations::UserStoreOperations;

/// Manages creating, updating and fetching User from the databae
#[derive(Clone)]
pub struct DieselUserStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselUserStore<C> {
    /// Creates a new DieselUserStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    // Allow dead code if diesel feature is not enabled
    #[allow(dead_code)]
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselUserStore { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl UserStore for DieselUserStore<diesel::pg::PgConnection> {
    fn add_user(&self, user: User) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).add_user(user.into())
    }

    fn update_user(&self, updated_user: User) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).update_user(updated_user)
    }

    fn remove_user(&self, id: &str) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).delete_user(id)
    }

    fn fetch_user(&self, id: &str) -> Result<User, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).fetch_user(id)
    }

    fn list_users(&self) -> Result<Vec<User>, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).list_users()
    }
}

#[cfg(feature = "sqlite")]
impl UserStore for DieselUserStore<diesel::sqlite::SqliteConnection> {
    fn add_user(&self, user: User) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).add_user(user.into())
    }

    fn update_user(&self, updated_user: User) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).update_user(updated_user)
    }

    fn remove_user(&self, id: &str) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).delete_user(id)
    }

    fn fetch_user(&self, id: &str) -> Result<User, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).fetch_user(id)
    }

    fn list_users(&self) -> Result<Vec<User>, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).list_users()
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselUserStore` correctly supports adding and fetching users.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserStore`.
    /// 3. Add some users.
    /// 4. Verify that the `fetch_user` method returns correct values for all existing users.
    /// 5. Verify that the `fetch_user` method returns a `UserStoreError::NotFoundError` for a
    ///    non-existent user.
    #[test]
    fn sqlite_add_and_fetch() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselUserStore::new(pool);

        let user1 = User::new("user1");
        store.add_user(user1.clone()).expect("Failed to add user1");
        let user2 = User::new("user2");
        store.add_user(user2.clone()).expect("Failed to add user2");
        let user3 = User::new("user3");
        store.add_user(user3.clone()).expect("Failed to add user3");

        assert_eq!(
            store.fetch_user("user1").expect("Failed to fetch user1"),
            user1,
        );
        assert_eq!(
            store.fetch_user("user2").expect("Failed to fetch user2"),
            user2,
        );
        assert_eq!(
            store.fetch_user("user3").expect("Failed to fetch user3"),
            user3,
        );

        match store.fetch_user("user4") {
            Err(UserStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(UserStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselUserStore` correctly supports listing users.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserStore`.
    /// 3. Add some users.
    /// 4. Verify that the `list_users` method returns the correct values.
    #[test]
    fn sqlite_list() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselUserStore::new(pool);

        let user1 = User::new("user1");
        store.add_user(user1.clone()).expect("Failed to add user1");
        let user2 = User::new("user2");
        store.add_user(user2.clone()).expect("Failed to add user2");
        let user3 = User::new("user3");
        store.add_user(user3.clone()).expect("Failed to add user3");

        let users = store.list_users().expect("Failed to list users");
        assert_eq!(users.len(), 3);
        assert!(users.contains(&user1));
        assert!(users.contains(&user2));
        assert!(users.contains(&user3));
    }

    /// Verify that a SQLite-backed `DieselUserStore` correctly supports removing users.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create the `DieselUserStore`.
    /// 3. Add some users.
    /// 4. Remove a user and verify that the user no longer appears with `fetch_user` or
    ///    `list_users`.
    #[test]
    fn sqlite_remove() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselUserStore::new(pool);

        let user1 = User::new("user1");
        store.add_user(user1.clone()).expect("Failed to add user1");
        let user2 = User::new("user2");
        store.add_user(user2.clone()).expect("Failed to add user2");
        let user3 = User::new("user3");
        store.add_user(user3.clone()).expect("Failed to add user3");

        store.remove_user("user3").expect("Failed to remove user3");
        match store.fetch_user("user3") {
            Err(UserStoreError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(UserStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
        let users = store.list_users().expect("Failed to list users");
        assert_eq!(users.len(), 2);
        assert!(!users.contains(&user3));
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
