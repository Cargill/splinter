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

use diesel::prelude::*;

use crate::error::InternalError;

use crate::biome::oauth::store::{
    diesel::{models::OAuthUserModel, schema::oauth_user},
    OAuthUser, OAuthUserStoreError,
};

use super::OAuthUserStoreOperations;

pub(in crate::biome::oauth) trait OAuthUserStoreGetByAccessToken {
    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError>;
}

impl<'a, C> OAuthUserStoreGetByAccessToken for OAuthUserStoreOperations<'a, C>
where
    C: diesel::Connection,
    i16: diesel::deserialize::FromSql<diesel::sql_types::SmallInt, C::Backend>,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_by_access_token(
        &self,
        access_token: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let oauth_user_model = oauth_user::table
            .filter(oauth_user::access_token.eq(Some(access_token.to_string())))
            .first::<OAuthUserModel>(self.conn)
            .optional()
            .map_err(|err| {
                OAuthUserStoreError::InternalError(InternalError::from_source(Box::new(err)))
            })?;

        Ok(oauth_user_model.map(OAuthUser::from))
    }
}
