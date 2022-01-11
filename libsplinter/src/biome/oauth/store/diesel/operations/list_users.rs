// Copyright 2018-2022 Cargill Incorporated
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

use diesel::prelude::*;

use crate::biome::oauth::store::{
    diesel::{models::OAuthUserModel, schema::oauth_users},
    OAuthUser, OAuthUserIter, OAuthUserSessionStoreError,
};

use super::OAuthUserSessionStoreOperations;

pub trait OAuthUserSessionStoreListUsers {
    fn list_users(&self) -> Result<OAuthUserIter, OAuthUserSessionStoreError>;
}

impl<'a, C> OAuthUserSessionStoreListUsers for OAuthUserSessionStoreOperations<'a, C>
where
    C: diesel::Connection,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_users(&self) -> Result<OAuthUserIter, OAuthUserSessionStoreError> {
        Ok(OAuthUserIter::new(
            oauth_users::table
                .load::<OAuthUserModel>(self.conn)?
                .into_iter()
                .map(OAuthUser::from)
                .collect::<Vec<OAuthUser>>(),
        ))
    }
}
