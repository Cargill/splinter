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

//! Database-backed implementation of the [OAuthUserSessionStore], powered by [diesel].

pub(in crate::biome) mod models;
mod operations;
pub(in crate::biome) mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{
    InsertableOAuthUserSession, OAuthUser, OAuthUserSession, OAuthUserSessionStore,
    OAuthUserSessionStoreError,
};

use operations::{
    add_session::OAuthUserSessionStoreAddSession as _,
    get_session::OAuthUserSessionStoreGetSession as _, get_user::OAuthUserSessionStoreGetUser as _,
    remove_session::OAuthUserSessionStoreRemoveSession as _,
    update_session::OAuthUserSessionStoreUpdateSession as _, OAuthUserSessionStoreOperations,
};

/// A database-backed [OAuthUserSessionStore], powered by [diesel].
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
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).add_session(session)
    }

    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).update_session(session)
    }

    fn remove_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).remove_session(splinter_access_token)
    }

    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_session(splinter_access_token)
    }

    fn get_user(&self, subject: &str) -> Result<Option<OAuthUser>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_user(subject)
    }

    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(feature = "biome-oauth-user-store-postgres")]
impl OAuthUserSessionStore for DieselOAuthUserSessionStore<diesel::pg::PgConnection> {
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).add_session(session)
    }

    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).update_session(session)
    }

    fn remove_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).remove_session(splinter_access_token)
    }

    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_session(splinter_access_token)
    }

    fn get_user(&self, subject: &str) -> Result<Option<OAuthUser>, OAuthUserSessionStoreError> {
        let connection = self.connection_pool.get()?;
        OAuthUserSessionStoreOperations::new(&*connection).get_user(subject)
    }

    fn clone_box(&self) -> Box<dyn OAuthUserSessionStore> {
        Box::new(Self {
            connection_pool: self.connection_pool.clone(),
        })
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::biome::oauth::store::InsertableOAuthUserSessionBuilder;
    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports adding and
    /// getting OAuth user sessions.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuth user session.
    /// 4. Verify that the `get_session` method returns a matching session.
    /// 5. Verify that the `get_session` method returns `None` a non-existent token.
    /// 6. Verify that a session with the same Splinter access token can't be added (results in a
    ///    ConstraintViolation).
    #[test]
    fn sqlite_add_and_get_session() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let splinter_access_token = "splinter_access_token";
        let subject = "subject";
        let oauth_access_token = "oauth_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token.into())
            .build()
            .expect("Unable to build session");
        oauth_user_session_store
            .add_session(session)
            .expect("Unable to add session");

        let session = oauth_user_session_store
            .get_session(splinter_access_token)
            .expect("Unable to get session")
            .expect("Session not found");
        assert_eq!(session.splinter_access_token(), splinter_access_token);
        assert_eq!(session.user().subject(), subject);
        assert_eq!(session.oauth_access_token(), oauth_access_token);
        assert_eq!(session.oauth_refresh_token(), None);

        assert!(oauth_user_session_store
            .get_session("NonExistentToken")
            .expect("Unable to query non-existent token")
            .is_none());

        let non_unique_session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token("splinter_access_token".into())
            .with_subject("different_subject".into())
            .with_oauth_access_token("different_oauth_access_token".into())
            .build()
            .expect("Unable to build non-unique session");
        assert!(matches!(
            oauth_user_session_store.add_session(non_unique_session),
            Err(OAuthUserSessionStoreError::ConstraintViolation(_)),
        ));
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports updating
    /// session data.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuth user session.
    /// 4. Get the session from the store, update its OAuth tokens (access and refresh), and submit
    ///    the update to the store.
    /// 5. Verify that the `get_session` method returns the correct, updated values for the session.
    /// 6. Verify that attempting to update a session that doesn't exist results in an InvalidState
    ///    error.
    /// 7. Verify that attempting to update immutable fields for session results in an
    ///    InvalidArgument error.
    #[test]
    fn sqlite_update_session() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let splinter_access_token = "splinter_access_token";
        let subject = "subject";
        let oauth_access_token = "oauth_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token.into())
            .build()
            .expect("Unable to build session");
        oauth_user_session_store
            .add_session(session)
            .expect("Unable to add session");

        let updated_oauth_access_token = "updated_oauth_access_token";
        let updated_oauth_refresh_token = "updated_oauth_refresh_token";
        let updated_session = oauth_user_session_store
            .get_session(splinter_access_token)
            .expect("Unable to get session")
            .expect("Session not found")
            .into_update_builder()
            .with_oauth_access_token(updated_oauth_access_token.into())
            .with_oauth_refresh_token(Some(updated_oauth_refresh_token.into()))
            .build();
        oauth_user_session_store
            .update_session(updated_session)
            .expect("Unable to update session");

        let updated_session = oauth_user_session_store
            .get_session(splinter_access_token)
            .expect("Unable to get updated session")
            .expect("Updated session not found");
        assert_eq!(
            updated_session.oauth_access_token(),
            updated_oauth_access_token
        );
        assert_eq!(
            updated_session.oauth_refresh_token(),
            Some(updated_oauth_refresh_token)
        );

        let non_existent_session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token("NonExistentToken".into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token.into())
            .build()
            .expect("Unable to build non-existent session");
        assert!(matches!(
            oauth_user_session_store.update_session(non_existent_session),
            Err(OAuthUserSessionStoreError::InvalidState(_)),
        ));

        let update_subject = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("updated_subject".into())
            .with_oauth_access_token(oauth_access_token.into())
            .build()
            .expect("Unable to build session for updating subject");
        assert!(matches!(
            oauth_user_session_store.update_session(update_subject),
            Err(OAuthUserSessionStoreError::InvalidArgument(_)),
        ));
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports removing
    /// session data.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuth user session.
    /// 4. Verify that the `remove_session` method removes the session.
    /// 5. Verify that attempting to remove a session that doesn't exist results in an InvalidState
    ///    error.
    #[test]
    fn sqlite_remove_session() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let splinter_access_token = "splinter_access_token";
        let session = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token.into())
            .with_subject("subject".into())
            .with_oauth_access_token("oauth_access_token".into())
            .build()
            .expect("Unable to build session");
        oauth_user_session_store
            .add_session(session)
            .expect("Unable to add session");

        oauth_user_session_store
            .remove_session(splinter_access_token)
            .expect("Unable to remove session");
        assert!(oauth_user_session_store
            .get_session(splinter_access_token)
            .expect("Unable to attempt to get session")
            .is_none());

        assert!(matches!(
            oauth_user_session_store.remove_session("NonExistentToken"),
            Err(OAuthUserSessionStoreError::InvalidState(_)),
        ));
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports inserting and
    /// and getting OAuth users.
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add an OAuth user session.
    /// 4. Verify that the `get_user` method returns an OAuth user that matches the subject of the
    ///    OAuth session.
    /// 5. Add a new session for the same subject and verify that the `get_user` method returns the
    ///    same user data as before.
    /// 6. Delete both sessions and verify that the `get_user` method still returns the user's data.
    #[test]
    fn sqlite_get_user() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let splinter_access_token1 = "splinter_access_token1";
        let subject = "subject";
        let session1 = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token1.into())
            .with_subject(subject.into())
            .with_oauth_access_token("oauth_access_token1".into())
            .build()
            .expect("Unable to build session1");
        oauth_user_session_store
            .add_session(session1)
            .expect("Unable to add session1");

        let user = oauth_user_session_store
            .get_user(subject)
            .expect("Unable to get user")
            .expect("User not found");
        assert_eq!(user.subject(), subject);

        let splinter_access_token2 = "splinter_access_token2";
        let session2 = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token2.into())
            .with_subject(subject.into())
            .with_oauth_access_token("oauth_access_token2".into())
            .build()
            .expect("Unable to build session2");
        oauth_user_session_store
            .add_session(session2)
            .expect("Unable to add session2");

        let same_user = oauth_user_session_store
            .get_user(subject)
            .expect("Unable to get user")
            .expect("User not found");
        assert_eq!(user.subject(), same_user.subject());
        assert_eq!(user.user_id(), same_user.user_id());

        oauth_user_session_store
            .remove_session(splinter_access_token1)
            .expect("Unable to remove session1");
        oauth_user_session_store
            .remove_session(splinter_access_token2)
            .expect("Unable to remove session2");

        let still_the_same_user = oauth_user_session_store
            .get_user(subject)
            .expect("Unable to get user")
            .expect("User not found");
        assert_eq!(user.subject(), still_the_same_user.subject());
        assert_eq!(user.user_id(), still_the_same_user.user_id());
    }

    /// Verify that a SQLite-backed `DieselOAuthUserSessionStore` correctly supports multiple
    /// sessions for a single user
    ///
    /// 1. Create a connection pool for an in-memory SQLite database and run migrations.
    /// 2. Create a `DieselOAuthUserSessionStore`.
    /// 3. Add two OAuth sessions for the same user.
    /// 4. Verify that the `get_session` method returns both sessions correctly, with same subject.
    #[test]
    fn sqlite_multiple_sessions() {
        let pool = create_connection_pool_and_migrate();

        let oauth_user_session_store = DieselOAuthUserSessionStore::new(pool);

        let splinter_access_token1 = "splinter_access_token1";
        let subject = "subject";
        let oauth_access_token1 = "oauth_access_token1";
        let session1 = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token1.into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token1.into())
            .build()
            .expect("Unable to build session1");
        oauth_user_session_store
            .add_session(session1)
            .expect("Unable to add session1");

        let splinter_access_token2 = "splinter_access_token2";
        let oauth_access_token2 = "oauth_access_token2";
        let session2 = InsertableOAuthUserSessionBuilder::new()
            .with_splinter_access_token(splinter_access_token2.into())
            .with_subject(subject.into())
            .with_oauth_access_token(oauth_access_token2.into())
            .build()
            .expect("Unable to build session2");
        oauth_user_session_store
            .add_session(session2)
            .expect("Unable to add session2");

        let stored_session1 = oauth_user_session_store
            .get_session(splinter_access_token1)
            .expect("Unable to get session1")
            .expect("Session1 not found");
        assert_eq!(
            stored_session1.splinter_access_token(),
            splinter_access_token1
        );
        assert_eq!(stored_session1.user().subject(), subject);
        assert_eq!(stored_session1.oauth_access_token(), oauth_access_token1);
        let stored_session2 = oauth_user_session_store
            .get_session(splinter_access_token2)
            .expect("Unable to get session2")
            .expect("Session2 not found");
        assert_eq!(
            stored_session2.splinter_access_token(),
            splinter_access_token2
        );
        assert_eq!(stored_session2.user().subject(), subject);
        assert_eq!(stored_session2.oauth_access_token(), oauth_access_token2);
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
