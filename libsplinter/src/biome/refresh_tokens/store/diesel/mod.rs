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

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use crate::biome::refresh_tokens::store::{RefreshTokenError, RefreshTokenStore};

use operations::{
    add_token::RefreshTokenStoreAddTokenOperation,
    fetch_token::RefreshTokenStoreFetchTokenOperation,
    remove_token::RefreshTokenStoreRemoveTokenOperation,
    update_token::RefreshTokenStoreUpdateTokenOperation, RefreshTokenStoreOperations,
};

pub struct DieselRefreshTokenStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselRefreshTokenStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

#[cfg(feature = "postgres")]
impl RefreshTokenStore for DieselRefreshTokenStore<diesel::pg::PgConnection> {
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).add_token(user_id, token)
    }
    fn remove_token(&self, user_id: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).remove_token(user_id)
    }
    fn update_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).update_token(user_id, token)
    }
    fn fetch_token(&self, user_id: &str) -> Result<String, RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).fetch_token(user_id)
    }
}

#[cfg(feature = "sqlite")]
impl RefreshTokenStore for DieselRefreshTokenStore<diesel::sqlite::SqliteConnection> {
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).add_token(user_id, token)
    }
    fn remove_token(&self, user_id: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).remove_token(user_id)
    }
    fn update_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).update_token(user_id, token)
    }
    fn fetch_token(&self, user_id: &str) -> Result<String, RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).fetch_token(user_id)
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::migrations::run_sqlite_migrations;
    use crate::biome::user::store::{diesel::DieselUserStore, User, UserStore};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselRefreshTokenStore` correctly supports adding and
    /// fetching tokens.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and add the necessary users.
    /// 3. Create the `DieselRefreshTokenStore`.
    /// 4. Add some tokens.
    /// 5. Verify that the `fetch_token` method returns correct values for all existing tokens.
    /// 6. Verify that the `fetch_token` method returns a `RefreshTokenError::NotFoundError` for a
    ///    non-existent token.
    #[test]
    fn sqlite_add_and_fetch() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        user_store
            .add_user(User::new("user1"))
            .expect("Failed to add user1");
        user_store
            .add_user(User::new("user2"))
            .expect("Failed to add user2");
        user_store
            .add_user(User::new("user3"))
            .expect("Failed to add user3");

        let store = DieselRefreshTokenStore::new(pool);

        store
            .add_token("user1", "token1")
            .expect("Failed to add token1");
        store
            .add_token("user2", "token2")
            .expect("Failed to add token2");
        store
            .add_token("user3", "token3")
            .expect("Failed to add token3");

        assert_eq!(
            store.fetch_token("user1").expect("Failed to fetch token1"),
            "token1",
        );
        assert_eq!(
            store.fetch_token("user2").expect("Failed to fetch token2"),
            "token2",
        );
        assert_eq!(
            store.fetch_token("user3").expect("Failed to fetch token3"),
            "token3",
        );

        match store.fetch_token("user4") {
            Err(RefreshTokenError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(UserStoreError::NotFoundError), got {:?} instead",
                res
            ),
        }
    }

    /// Verify that a SQLite-backed `DieselRefreshTokenStore` correctly supports updating tokens.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and add the necessary user.
    /// 3. Create the `DieselRefreshTokenStore`.
    /// 4. Add a token and verify its existence in the store.
    /// 5. Update the token and verify that it is updated for the user.
    #[test]
    fn sqlite_update() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        user_store
            .add_user(User::new("user"))
            .expect("Failed to add user");

        let store = DieselRefreshTokenStore::new(pool);

        store
            .add_token("user", "token1")
            .expect("Failed to add token");
        assert_eq!(
            store.fetch_token("user").expect("Failed to fetch token1"),
            "token1",
        );

        store
            .update_token("user", "token2")
            .expect("Failed to update token");
        assert_eq!(
            store.fetch_token("user").expect("Failed to fetch token2"),
            "token2",
        );
    }

    /// Verify that a SQLite-backed `DieselRefreshTokenStore` correctly supports removing tokens.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and add the necessary users.
    /// 3. Create the `DieselRefreshTokenStore`.
    /// 4. Add some tokens.
    /// 4. Remove a token and verify that the token no longer appears in the store.
    #[test]
    fn sqlite_remove() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        user_store
            .add_user(User::new("user1"))
            .expect("Failed to add user1");
        user_store
            .add_user(User::new("user2"))
            .expect("Failed to add user2");
        user_store
            .add_user(User::new("user3"))
            .expect("Failed to add user3");

        let store = DieselRefreshTokenStore::new(pool);

        store
            .add_token("user1", "token1")
            .expect("Failed to add token1");
        store
            .add_token("user2", "token2")
            .expect("Failed to add token2");
        store
            .add_token("user3", "token3")
            .expect("Failed to add token3");

        store
            .remove_token("user3")
            .expect("Failed to remove token3");
        match store.fetch_token("user3") {
            Err(RefreshTokenError::NotFoundError(_)) => {}
            res => panic!(
                "Expected Err(RefreshTokenError::NotFoundError), got {:?} instead",
                res
            ),
        }
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
