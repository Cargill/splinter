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

use crate::error::InternalError;

use super::{AccessToken, OAuthProvider, OAuthUser, OAuthUserStore, OAuthUserStoreError};

use models::{NewOAuthUserModel, OAuthUserModel, ProviderId};
use operations::add_oauth_user::OAuthUserStoreAddOAuthUserOperation as _;
use operations::get_by_access_token::OAuthUserStoreGetByAccessToken as _;
use operations::get_by_provider_user_ref::OAuthUserStoreGetByProviderUserRef as _;
use operations::get_by_user_id::OAuthUserStoreGetByUserId as _;
use operations::update_oauth_user::OAuthUserStoreUpdateOAuthUserOperation as _;
use operations::OAuthUserStoreOperations;

pub struct DieselOAuthUserStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection + 'static> DieselOAuthUserStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl OAuthUserStore for DieselOAuthUserStore<diesel::sqlite::SqliteConnection> {
    fn add_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).add_oauth_user(oauth_user)
    }

    fn update_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).update_oauth_user(oauth_user)
    }

    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_provider_user_ref(provider_user_ref)
    }

    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_access_token(access_token)
    }

    fn get_by_user_id(&self, user_id: &str) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_user_id(user_id)
    }

    fn clone_box(&self) -> Box<dyn OAuthUserStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(feature = "postgres")]
impl OAuthUserStore for DieselOAuthUserStore<diesel::pg::PgConnection> {
    fn add_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).add_oauth_user(oauth_user)
    }

    fn update_oauth_user(&self, oauth_user: OAuthUser) -> Result<(), OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).update_oauth_user(oauth_user)
    }

    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_provider_user_ref(provider_user_ref)
    }

    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_access_token(access_token)
    }

    fn get_by_user_id(&self, user_id: &str) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserStoreOperations::new(&*connection).get_by_user_id(user_id)
    }

    fn clone_box(&self) -> Box<dyn OAuthUserStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

impl From<OAuthUserModel> for OAuthUser {
    fn from(model: OAuthUserModel) -> Self {
        let OAuthUserModel {
            id: _,
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider_id,
        } = model;
        Self {
            user_id,
            provider_user_ref,
            access_token: match access_token {
                Some(token) => AccessToken::Authorized(token),
                None => AccessToken::Unauthorized,
            },
            refresh_token,
            provider: match provider_id {
                ProviderId::Github => OAuthProvider::Github,
            },
        }
    }
}

impl<'a> From<&'a OAuthUser> for NewOAuthUserModel<'a> {
    fn from(user: &'a OAuthUser) -> Self {
        NewOAuthUserModel {
            user_id: user.user_id(),
            provider_user_ref: user.provider_user_ref(),
            access_token: match user.access_token() {
                AccessToken::Authorized(token) => Some(token),
                AccessToken::Unauthorized => None,
            },
            refresh_token: user.refresh_token(),
            provider_id: match user.provider() {
                OAuthProvider::Github => ProviderId::Github,
            },
        }
    }
}

impl From<diesel::r2d2::PoolError> for OAuthUserStoreError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        OAuthUserStoreError::InternalError(InternalError::from_source(Box::new(err)))
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::migrations::run_sqlite_migrations;
    use crate::biome::oauth::store::{AccessToken, OAuthUserBuilder};
    use crate::biome::user::store::{diesel::DieselUserStore, User, UserStore};

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselOAuthUserStore` correctly supports fetching
    /// an oauth user by provider user ref.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and a `DieselOAuthUserStore`.
    /// 3. Add a User and an OAuthUser
    /// 4. Verify that the `get_by_provider_user_ref` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Verify that the `get_by_provider_user_ref` method returns `None` for
    ///    for non-existent provider_user_ref.
    /// 6. Verify that a duplicate entry can't be added (results in a ConstraintViolation).
    #[test]
    fn sqlite_add_and_fetch_by_provider_user_ref() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        let oauth_user_store = DieselOAuthUserStore::new(pool);

        let user_id = "test_biome_user_id";
        user_store
            .add_user(User::new(user_id))
            .expect("unable to insert user");

        let oauth_user = OAuthUserBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_user = oauth_user_store
            .get_by_provider_user_ref("TestUser")
            .expect("Unable to look up oath user");

        let stored_oauth_user = stored_oauth_user.expect("Did not find the oauth user (was None)");

        assert_eq!("test_biome_user_id", stored_oauth_user.user_id());
        assert_eq!("TestUser", stored_oauth_user.provider_user_ref());
        assert_eq!(
            &AccessToken::Authorized("someaccesstoken".to_string()),
            stored_oauth_user.access_token()
        );
        assert_eq!(None, stored_oauth_user.refresh_token());
        assert_eq!(&OAuthProvider::Github, stored_oauth_user.provider());

        let unknown_oauth_user = oauth_user_store
            .get_by_provider_user_ref("NonExistentUserRef".into())
            .expect("Could not query non-existent oauth user");
        assert!(unknown_oauth_user.is_none());

        // Create another user and try to connect it to the same user id.
        let oauth_user = OAuthUserBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser2".into())
            .with_access_token(AccessToken::Authorized("someotheraccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        let err = oauth_user_store
            .add_oauth_user(oauth_user)
            .expect_err("Did not return an error");

        assert!(matches!(err, OAuthUserStoreError::ConstraintViolation(_)));
    }

    /// Verify that a SQLite-backed `DieselOAuthUserStore` correctly supports updating an
    /// OAuthUser's `refresh_token`.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and a `DieselOAuthUserStore`.
    /// 3. Add a User and an OAuthUser
    /// 4. Verify that the `get_by_user_id` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Update the user to have a refresh token.
    /// 6. Verify that the `get_by_user_id` method returns correct values for the
    ///    updated OAuth User.
    #[test]
    fn sqlite_update_oauth_user_refresh_token() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        let oauth_user_store = DieselOAuthUserStore::new(pool);

        let user_id = "test_biome_user_id";
        user_store
            .add_user(User::new(user_id))
            .expect("unable to insert user");

        let oauth_user = OAuthUserBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_user = oauth_user_store
            .get_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .expect("Did not find the oauth user (was None)");

        assert_eq!(None, stored_oauth_user.refresh_token());

        let updated_oauth_user = stored_oauth_user
            .into_update_builder()
            .with_refresh_token(Some("somerefreshtoken".into()))
            .build()
            .expect("Unable to build updated user");

        oauth_user_store
            .update_oauth_user(updated_oauth_user)
            .expect("Unable to update the oauth user");

        // Verify that the user was updated
        let stored_oauth_user = oauth_user_store
            .get_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .expect("Did not find the oauth user (was None)");

        assert_eq!(Some("somerefreshtoken"), stored_oauth_user.refresh_token());
    }

    /// Verify that a SQLite-backed `DieselOAuthUserStore` correctly supports updating an
    /// OAuthUser's `access_token`.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselUserStore` and a `DieselOAuthUserStore`.
    /// 3. Add a User and an OAuthUser
    /// 4. Verify that the `get_by_user_id` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Update the user to have an `Unauthorized` `access_token`.
    /// 6. Verify that the `get_by_user_id` method returns correct values for the
    ///    updated OAuth User.
    #[test]
    fn sqlite_update_oauth_user_access_token() {
        let pool = create_connection_pool_and_migrate();

        let user_store = DieselUserStore::new(pool.clone());
        let oauth_user_store = DieselOAuthUserStore::new(pool);

        let user_id = "test_biome_user_id";
        user_store
            .add_user(User::new(user_id))
            .expect("unable to insert user");

        let oauth_user = OAuthUserBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_user = oauth_user_store
            .get_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .expect("Did not find the oauth user (was None)");

        assert_eq!(
            &AccessToken::Authorized("someaccesstoken".to_string()),
            stored_oauth_user.access_token()
        );

        let updated_oauth_user = stored_oauth_user
            .into_update_builder()
            .with_access_token(AccessToken::Unauthorized)
            .build()
            .expect("Unable to build updated user");

        oauth_user_store
            .update_oauth_user(updated_oauth_user)
            .expect("Unable to update the oauth user");

        // Verify that the user was updated
        let stored_oauth_user = oauth_user_store
            .get_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .expect("Did not find the oauth user (was None)");

        assert_eq!(&AccessToken::Unauthorized, stored_oauth_user.access_token());
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
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
