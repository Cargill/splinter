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

use diesel::{dsl::update, prelude::*};

use crate::biome::oauth::store::{
    diesel::{models::OAuthUserSessionModel, schema::oauth_user_sessions},
    InsertableOAuthUserSession, OAuthUserSessionStoreError,
};
use crate::error::{InvalidArgumentError, InvalidStateError};

use super::OAuthUserSessionStoreOperations;

pub trait OAuthUserSessionStoreUpdateSession {
    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError>;
}

impl<'a, C> OAuthUserSessionStoreUpdateSession for OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn update_session(
        &self,
        session: InsertableOAuthUserSession,
    ) -> Result<(), OAuthUserSessionStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // Check that a session already exists for the Splinter access token
            match oauth_user_sessions::table
                .find(session.splinter_access_token())
                .first::<OAuthUserSessionModel>(self.conn)
                .optional()?
            {
                Some(existing_session) => {
                    // Check that the caller is not attempting to change an immutable field
                    if session.subject() != existing_session.subject {
                        Err(OAuthUserSessionStoreError::InvalidArgument(
                            InvalidArgumentError::new(
                                "session".to_string(),
                                "Cannot update the 'subject' field for an OAuth user session"
                                    .into(),
                            ),
                        ))
                    } else {
                        // All checks have completed, update the entry
                        update(
                            oauth_user_sessions::table.filter(
                                oauth_user_sessions::splinter_access_token
                                    .eq(session.splinter_access_token()),
                            ),
                        )
                        .set((
                            oauth_user_sessions::oauth_access_token
                                .eq(session.oauth_access_token()),
                            oauth_user_sessions::oauth_refresh_token
                                .eq(session.oauth_refresh_token()),
                        ))
                        .execute(self.conn)
                        .map(|_| ())
                        .map_err(OAuthUserSessionStoreError::from)
                    }
                }
                None => Err(OAuthUserSessionStoreError::InvalidState(
                    InvalidStateError::with_message(
                        "An OAuth user session for the given Splinter access token does not exist"
                            .to_string(),
                    ),
                )),
            }
        })
    }
}
