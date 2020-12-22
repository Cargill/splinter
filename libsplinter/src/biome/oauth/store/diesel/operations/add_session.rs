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

use diesel::{dsl::insert_into, prelude::*};

use crate::biome::oauth::store::{
    diesel::{
        models::{InsertableOAuthUserSessionModel, OAuthUserModel, OAuthUserSessionModel},
        schema::{oauth_user_sessions, oauth_users},
    },
    InsertableOAuthUserSession, OAuthUser, OAuthUserSessionStoreError,
};
use crate::error::{ConstraintViolationError, ConstraintViolationType};

use super::OAuthUserSessionStoreOperations;

pub trait OAuthUserSessionStoreAddSession {
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> OAuthUserSessionStoreAddSession
    for OAuthUserSessionStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // Check if a session already exists for the Splinter access token
            if oauth_user_sessions::table
                .find(session.splinter_access_token())
                .first::<OAuthUserSessionModel>(self.conn)
                .optional()?
                .is_some()
            {
                return Err(OAuthUserSessionStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }

            // If the subject has not already been assigned a Biome user ID in the users table,
            // assign one by creating a new entry
            if oauth_users::table
                .find(session.subject())
                .first::<OAuthUserModel>(self.conn)
                .optional()?
                .is_none()
            {
                let user = OAuthUser::new(session.subject().to_string());
                insert_into(oauth_users::table)
                    .values(OAuthUserModel::from(user))
                    .execute(self.conn)
                    .map_err(OAuthUserSessionStoreError::from)?;
            }

            // Store the session data
            insert_into(oauth_user_sessions::table)
                .values(InsertableOAuthUserSessionModel::from(session))
                .execute(self.conn)
                .map(|_| ())
                .map_err(OAuthUserSessionStoreError::from)
        })
    }
}

#[cfg(feature = "biome-oauth-user-store-postgres")]
impl<'a> OAuthUserSessionStoreAddSession
    for OAuthUserSessionStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // Check if a session already exists for the Splinter access token
            if oauth_user_sessions::table
                .find(session.splinter_access_token())
                .first::<OAuthUserSessionModel>(self.conn)
                .optional()?
                .is_some()
            {
                return Err(OAuthUserSessionStoreError::ConstraintViolation(
                    ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
                ));
            }

            // If the subject has not already been assigned a Biome user ID in the users table,
            // assign one by creating a new entry
            if oauth_users::table
                .find(session.subject())
                .first::<OAuthUserModel>(self.conn)
                .optional()?
                .is_none()
            {
                let user = OAuthUser::new(session.subject().to_string());
                insert_into(oauth_users::table)
                    .values(OAuthUserModel::from(user))
                    .execute(self.conn)
                    .map_err(OAuthUserSessionStoreError::from)?;
            }

            // Store the session data
            insert_into(oauth_user_sessions::table)
                .values(InsertableOAuthUserSessionModel::from(session))
                .execute(self.conn)
                .map(|_| ())
                .map_err(OAuthUserSessionStoreError::from)
        })
    }
}
