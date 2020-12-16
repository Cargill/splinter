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

//! Database backend support for the OAuthUserSessionStore, powered by
//! [`Diesel`](https://crates.io/crates/diesel).

pub(in crate::biome) mod models;
mod operations;
pub(in crate::biome) mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    AccessToken, NewOAuthUserAccess, OAuthProvider, OAuthUserAccess, OAuthUserSessionStore,
    OAuthUserSessionStoreError,
};

use models::{NewOAuthUserModel, OAuthUserModel, ProviderId};
use operations::add_oauth_user::OAuthUserSessionStoreAddOAuthUserOperation as _;
use operations::get_by_access_token::OAuthUserSessionStoreGetByAccessToken as _;
use operations::list_by_provider_user_ref::OAuthUserSessionStoreListByProviderUserRef as _;
use operations::list_by_user_id::OAuthUserSessionStoreListByUserId as _;
use operations::update_oauth_user::OAuthUserSessionStoreUpdateOAuthUserOperation as _;
use operations::OAuthUserSessionStoreOperations;

/// A database-backed [`OAuthUserSessionStore`](`crate::biome::oauth::store::OAuthUserSessionStore`), powered by
/// [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselOAuthUserSessionStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection + 'static> DieselOAuthUserSessionStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl OAuthUserSessionStore for DieselOAuthUserSessionStore<diesel::sqlite::SqliteConnection> {
    fn add_oauth_user(
        &self,
        oauth_user: NewOAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).add_oauth_user((&oauth_user).into())
    }

    fn update_oauth_user(
        &self,
        oauth_user: OAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).update_oauth_user(oauth_user)
    }

    fn list_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Box<dyn Iterator<Item = OAuthUserAccess>>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        let records = OAuthUserSessionStoreOperations::new(&*connection)
            .list_by_provider_user_ref(provider_user_ref)?;
        Ok(Box::new(records.into_iter().map(OAuthUserAccess::from)))
    }

    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUserAccess>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_by_access_token(access_token)
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Box<dyn Iterator<Item = OAuthUserAccess>>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        let records =
            OAuthUserSessionStoreOperations::new(&*connection).list_by_user_id(user_id)?;
        Ok(Box::new(records.into_iter().map(OAuthUserAccess::from)))
    }

    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(feature = "biome-oauth-user-store-postgres")]
impl OAuthUserSessionStore for DieselOAuthUserSessionStore<diesel::pg::PgConnection> {
    fn add_oauth_user(
        &self,
        oauth_user: NewOAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).add_oauth_user((&oauth_user).into())
    }

    fn update_oauth_user(
        &self,
        oauth_user: OAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).update_oauth_user(oauth_user)
    }

    fn list_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Box<dyn Iterator<Item = OAuthUserAccess>>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        let records = OAuthUserSessionStoreOperations::new(&*connection)
            .list_by_provider_user_ref(provider_user_ref)?;
        Ok(Box::new(records.into_iter().map(OAuthUserAccess::from)))
    }

    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUserAccess>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_by_access_token(access_token)
    }

    fn list_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Box<dyn Iterator<Item = OAuthUserAccess>>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        let records =
            OAuthUserSessionStoreOperations::new(&*connection).list_by_user_id(user_id)?;
        Ok(Box::new(records.into_iter().map(OAuthUserAccess::from)))
    }

    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

impl From<OAuthUserModel> for OAuthUserAccess {
    fn from(model: OAuthUserModel) -> Self {
        let OAuthUserModel {
            id,
            user_id,
            provider_user_ref,
            access_token,
            refresh_token,
            provider_id,
        } = model;
        Self {
            id,
            user_id,
            provider_user_ref,
            access_token: match access_token {
                Some(token) => AccessToken::Authorized(token),
                None => AccessToken::Unauthorized,
            },
            refresh_token,
            provider: match provider_id {
                ProviderId::Github => OAuthProvider::Github,
                ProviderId::OpenId => OAuthProvider::OpenId,
            },
        }
    }
}

impl<'a> From<&'a NewOAuthUserAccess> for NewOAuthUserModel<'a> {
    fn from(user: &'a NewOAuthUserAccess) -> Self {
        NewOAuthUserModel {
            user_id: &user.user_id,
            provider_user_ref: &user.provider_user_ref,
            access_token: match &user.access_token {
                AccessToken::Authorized(ref token) => Some(&token),
                AccessToken::Unauthorized => None,
            },
            refresh_token: user.refresh_token.as_deref(),
            provider_id: match user.provider {
                OAuthProvider::Github => ProviderId::Github,
                OAuthProvider::OpenId => ProviderId::OpenId,
            },
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::oauth::store::{AccessToken, NewOAuthUserAccessBuilder};
    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports fetching
    /// an oauth user by provider user ref.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuthUser
    /// 4. Verify that the `get_by_provider_user_ref` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Verify that the `get_by_provider_user_ref` method returns `None` for
    ///    for non-existent provider_user_ref.
    /// 6. Verify that a duplicate entry can't be added (results in a ConstraintViolation).
    #[test]
    fn sqlite_add_and_fetch_by_provider_user_ref() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let user_id = "test_biome_user_id";
        let oauth_user = NewOAuthUserAccessBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_session_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_users = oauth_user_session_store
            .list_by_provider_user_ref("TestUser")
            .expect("Unable to list users")
            .collect::<Vec<_>>();

        assert_eq!(1, stored_oauth_users.len());
        let stored_oauth_user = stored_oauth_users
            .into_iter()
            .next()
            .expect("Did not find the oauth user (was empty)");

        assert_eq!("test_biome_user_id", stored_oauth_user.user_id());
        assert_eq!("TestUser", stored_oauth_user.provider_user_ref());
        assert_eq!(
            &AccessToken::Authorized("someaccesstoken".to_string()),
            stored_oauth_user.access_token()
        );
        assert_eq!(None, stored_oauth_user.refresh_token());
        assert_eq!(&OAuthProvider::Github, stored_oauth_user.provider());

        let mut unknown_oauth_user = oauth_user_session_store
            .list_by_provider_user_ref("NonExistentUserRef".into())
            .expect("Could not query non-existent oauth user");

        assert!(
            unknown_oauth_user.next().is_none(),
            "No user should have been returned"
        );

        // Create an entry for the same user but with an alternative access token
        let oauth_user = NewOAuthUserAccessBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someotheraccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_session_store
            .add_oauth_user(oauth_user)
            .expect("Did not insert the user access record");

        let stored_oauth_user = oauth_user_session_store
            .get_by_access_token("someotheraccesstoken")
            .expect("Unable to lookup user by access token");
        let stored_oauth_user = stored_oauth_user.expect("Did not find the oauth user (was None)");

        assert_eq!(
            &AccessToken::Authorized("someotheraccesstoken".to_string()),
            stored_oauth_user.access_token()
        );

        let stored_oauth_users = oauth_user_session_store
            .list_by_provider_user_ref("TestUser")
            .expect("unable to list users")
            .collect::<Vec<_>>();
        assert_eq!(2, stored_oauth_users.len());
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports updating an
    /// OAuthUser's `refresh_token`.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuthUser
    /// 4. Verify that the `get_by_user_id` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Update the user to have a refresh token.
    /// 6. Verify that the `get_by_user_id` method returns correct values for the
    ///    updated OAuth User.
    #[test]
    fn sqlite_update_oauth_user_refresh_token() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let user_id = "test_biome_user_id";
        let oauth_user = NewOAuthUserAccessBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_session_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_user = oauth_user_session_store
            .list_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .into_iter()
            .next()
            .expect("Did not find the oauth user (was None)");

        assert_eq!(None, stored_oauth_user.refresh_token());

        let updated_oauth_user = stored_oauth_user
            .into_update_builder()
            .with_refresh_token(Some("somerefreshtoken".into()))
            .build()
            .expect("Unable to build updated user");

        oauth_user_session_store
            .update_oauth_user(updated_oauth_user)
            .expect("Unable to update the oauth user");

        // Verify that the user was updated
        let stored_oauth_user = oauth_user_session_store
            .list_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .into_iter()
            .next()
            .expect("Did not find the oauth user (was None)");

        assert_eq!(Some("somerefreshtoken"), stored_oauth_user.refresh_token());
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports updating an
    /// OAuthUserAccess's `access_token`.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuthUserAccess
    /// 4. Verify that the `get_by_user_id` method returns correct values for the
    ///    existing OAuth User.
    /// 5. Update the user to have an `Unauthorized` `access_token`.
    /// 6. Verify that the `get_by_user_id` method returns correct values for the
    ///    updated OAuth User.
    #[test]
    fn sqlite_update_oauth_user_access_token() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let user_id = "test_biome_user_id";
        let oauth_user = NewOAuthUserAccessBuilder::new()
            .with_user_id(user_id.into())
            .with_provider_user_ref("TestUser".into())
            .with_access_token(AccessToken::Authorized("someaccesstoken".to_string()))
            .with_provider(OAuthProvider::Github)
            .build()
            .expect("Unable to construct oauth user");

        oauth_user_session_store
            .add_oauth_user(oauth_user)
            .expect("Unable to store oauth user");

        let stored_oauth_user = oauth_user_session_store
            .get_by_access_token("someaccesstoken")
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

        oauth_user_session_store
            .update_oauth_user(updated_oauth_user)
            .expect("Unable to update the oauth user");

        // Verify that the user was updated
        let stored_oauth_user = oauth_user_session_store
            .list_by_user_id("test_biome_user_id")
            .expect("Unable to look up oath user")
            .into_iter()
            .next()
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
