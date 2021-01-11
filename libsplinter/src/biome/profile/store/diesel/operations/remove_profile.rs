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

use super::UserProfileStoreOperations;

use diesel::{dsl::delete, prelude::*, result::Error::NotFound};

use crate::biome::profile::store::{
    diesel::{models::ProfileModel, schema::user_profile},
    UserProfileStoreError,
};

use crate::error::{InternalError, InvalidArgumentError};

pub trait UserProfileStoreRemoveProfile {
    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError>;
}

impl<'a, C> UserProfileStoreRemoveProfile for UserProfileStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn remove_profile(&self, user_id: &str) -> Result<(), UserProfileStoreError> {
        let profile = user_profile::table
            .filter(user_profile::user_id.eq(user_id))
            .first::<ProfileModel>(self.conn)
            .map(Some)
            .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
            .map_err(|err| {
                UserProfileStoreError::Internal(InternalError::with_message(format!(
                    "Failed check for existing user_id {}",
                    err
                )))
            })?;
        if profile.is_none() {
            return Err(UserProfileStoreError::InvalidArgument(
                InvalidArgumentError::new(
                    "user_id".to_string(),
                    "A profile for the given user_id does not exist".to_string(),
                ),
            ));
        }

        delete(user_profile::table.filter(user_profile::user_id.eq(user_id)))
            .execute(self.conn)
            .map(|_| ())
            .map_err(|err| {
                UserProfileStoreError::Internal(InternalError::with_message(format!(
                    "Failed check for existing user_id {}",
                    err
                )))
            })?;
        Ok(())
    }
}
