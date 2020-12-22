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

use std::convert::TryFrom;
use std::time::{Duration, UNIX_EPOCH};

use diesel::prelude::*;

use crate::biome::oauth::store::{
    diesel::{
        models::{OAuthUserModel, OAuthUserSessionModel},
        schema::{oauth_user_sessions, oauth_users},
    },
    OAuthUserSession, OAuthUserSessionStoreError,
};
use crate::error::InternalError;

use super::OAuthUserSessionStoreOperations;

pub trait OAuthUserSessionStoreGetSession {
    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError>;
}

impl<'a, C> OAuthUserSessionStoreGetSession for OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_session(
        &self,
        splinter_access_token: &str,
    ) -> Result<Option<OAuthUserSession>, OAuthUserSessionStoreError> {
        oauth_user_sessions::table
            .find(splinter_access_token)
            .first::<OAuthUserSessionModel>(self.conn)
            .optional()?
            .map(|session| {
                let OAuthUserSessionModel {
                    splinter_access_token,
                    subject,
                    oauth_access_token,
                    oauth_refresh_token,
                    last_authenticated,
                } = session;

                let last_authenticated = u64::try_from(last_authenticated).map_err(|err| {
                    OAuthUserSessionStoreError::Internal(InternalError::from_source_with_message(
                        Box::new(err),
                        "'last_authenticated' timestamp could not be converted from i64 to u64".to_string(),
                    ))
                })?;
                let last_authenticated = UNIX_EPOCH
                    .checked_add(Duration::from_secs(last_authenticated))
                    .ok_or_else(|| {
                        OAuthUserSessionStoreError::Internal(InternalError::with_message(
                            "'last_authenticated' timestamp could not be represented as a `SystemTime`"
                                .to_string(),
                        ))
                    })?;

                let user = oauth_users::table
                    .find(subject)
                    .first::<OAuthUserModel>(self.conn)?
                    .into();

                Ok(OAuthUserSession {
                    splinter_access_token,
                    user,
                    oauth_access_token,
                    oauth_refresh_token,
                    last_authenticated,
                })
            })
            .transpose()
    }
}
