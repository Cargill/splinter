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

use diesel::{dsl::delete, prelude::*};

use crate::biome::oauth::store::{
    diesel::{models::OAuthUserSessionModel, schema::oauth_user_sessions},
    OAuthUserSessionStoreError,
};
use crate::error::InvalidStateError;

use super::OAuthUserSessionStoreOperations;

pub trait OAuthUserSessionStoreRemoveSession {
    fn remove_session(&self, splinter_access_token: &str)
        -> Result<(), OAuthUserSessionStoreError>;
}

impl<'a, C> OAuthUserSessionStoreRemoveSession for OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn remove_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<(), OAuthUserSessionStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // Check that a session already exists for the Splinter access token
            match oauth_user_sessions::table
                .find(splinter_access_token)
                .first::<OAuthUserSessionModel>(self.conn)
                .optional()?
            {
                Some(_) => delete(oauth_user_sessions::table.find(splinter_access_token))
                    .execute(self.conn)
                    .map(|_| ())
                    .map_err(OAuthUserSessionStoreError::from),
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
