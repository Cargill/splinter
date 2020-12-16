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
    diesel::schema::oauth_user, AccessToken, OAuthUserAccess, OAuthUserSessionStoreError,
};

use super::OAuthUserSessionStoreOperations;

pub(in crate::biome::oauth) trait OAuthUserSessionStoreUpdateOAuthUserOperation {
    fn update_oauth_user(
        &self,
        oauth_user: OAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError>;
}

impl<'a, C> OAuthUserSessionStoreUpdateOAuthUserOperation for OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
{
    fn update_oauth_user(
        &self,
        oauth_user: OAuthUserAccess,
    ) -> Result<(), OAuthUserSessionStoreError> {
        let access_token = match oauth_user.access_token() {
            AccessToken::Authorized(token) => Some(token),
            AccessToken::Unauthorized => None,
        };
        update(oauth_user::table.filter(oauth_user::id.eq(oauth_user.id)))
            .set((
                oauth_user::access_token.eq(access_token),
                oauth_user::refresh_token.eq(&oauth_user.refresh_token()),
            ))
            .execute(self.conn)
            .map(|_| ())
            .map_err(OAuthUserSessionStoreError::from)
    }
}
