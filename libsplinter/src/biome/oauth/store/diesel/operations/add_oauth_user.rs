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
    diesel::{models::NewOAuthUserModel, schema::oauth_user},
    OAuthUserSessionStoreError,
};

use super::OAuthUserSessionStoreOperations;

pub(in crate::biome::oauth) trait OAuthUserSessionStoreAddOAuthUserOperation {
    fn add_oauth_user<'a>(
        &self,
        oauth_user: NewOAuthUserModel<'a>,
    ) -> Result<(), OAuthUserSessionStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> OAuthUserSessionStoreAddOAuthUserOperation
    for OAuthUserSessionStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_oauth_user<'b>(
        &self,
        oauth_user: NewOAuthUserModel<'b>,
    ) -> Result<(), OAuthUserSessionStoreError> {
        insert_into(oauth_user::table)
            .values(oauth_user)
            .execute(self.conn)
            .map(|_| ())
            .map_err(OAuthUserSessionStoreError::from)
    }
}

#[cfg(feature = "biome-oauth-user-store-postgres")]
impl<'a> OAuthUserSessionStoreAddOAuthUserOperation
    for OAuthUserSessionStoreOperations<'a, diesel::pg::PgConnection>
{
    fn add_oauth_user<'b>(
        &self,
        oauth_user: NewOAuthUserModel<'b>,
    ) -> Result<(), OAuthUserSessionStoreError> {
        insert_into(oauth_user::table)
            .values(oauth_user)
            .execute(self.conn)
            .map(|_| ())
            .map_err(OAuthUserSessionStoreError::from)
    }
}
