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

pub(in crate::biome::oauth) trait OAuthUserStoreGetByProviderUserRef {
    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> OAuthUserStoreGetByProviderUserRef
    for OAuthUserStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn get_by_provider_user_ref(
        &self,
        provider_user_ref: &str,
    ) -> Result<Option<OAuthUser>, OAuthUserStoreError> {
        let oauth_user_model = oauth_user::table
            .filter(oauth_user::provider_user_ref.eq(provider_user_ref))
            .first::<OAuthUserModel>(self.conn)
            .optional()
            .map_err(|err| {
                OAuthUserStoreError::InternalError(InternalError::from_source(Box::new(err)))
            })?;

        Ok(oauth_user_model.map(OAuthUser::from))
    }
}
