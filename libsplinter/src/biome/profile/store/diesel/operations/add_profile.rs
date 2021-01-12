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

use diesel::{dsl::insert_into, prelude::*, result::Error::NotFound};

use crate::biome::profile::store::{
    diesel::{
        models::{NewProfileModel, ProfileModel},
        schema::user_profile,
    },
    Profile, UserProfileStoreError,
};

use crate::error::{ConstraintViolationError, ConstraintViolationType, InternalError};

pub trait UserProfileStoreAddProfile {
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> UserProfileStoreAddProfile
    for UserProfileStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let duplicate_profile = user_profile::table
            .filter(user_profile::user_id.eq(&profile.user_id))
            .first::<ProfileModel>(self.conn)
            .map(Some)
            .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
            .map_err(|err| {
                UserProfileStoreError::Internal(InternalError::with_message(format!(
                    "Failed check for existing user_id {}",
                    err
                )))
            })?;

        if duplicate_profile.is_some() {
            return Err(UserProfileStoreError::ConstraintViolation(
                ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
            ));
        }

        let new_profile: NewProfileModel = profile.into();

        insert_into(user_profile::table)
            .values(new_profile)
            .execute(self.conn)
            .map(|_| ())
            .map_err(|_| {
                UserProfileStoreError::Internal(InternalError::with_message(
                    "Failed to add credentials".to_string(),
                ))
            })?;
        Ok(())
    }
}

#[cfg(feature = "postgres")]
impl<'a> UserProfileStoreAddProfile for UserProfileStoreOperations<'a, diesel::pg::PgConnection> {
    fn add_profile(&self, profile: Profile) -> Result<(), UserProfileStoreError> {
        let duplicate_profile = user_profile::table
            .filter(user_profile::user_id.eq(&profile.user_id))
            .first::<ProfileModel>(self.conn)
            .map(Some)
            .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
            .map_err(|err| {
                UserProfileStoreError::Internal(InternalError::with_message(format!(
                    "Failed check for existing user_id {}",
                    err
                )))
            })?;

        if duplicate_profile.is_some() {
            return Err(UserProfileStoreError::ConstraintViolation(
                ConstraintViolationError::with_violation_type(ConstraintViolationType::Unique),
            ));
        }

        let new_profile: NewProfileModel = profile.into();

        insert_into(user_profile::table)
            .values(new_profile)
            .execute(self.conn)
            .map(|_| ())
            .map_err(|_| {
                UserProfileStoreError::Internal(InternalError::with_message(
                    "Failed to add credentials".to_string(),
                ))
            })?;
        Ok(())
    }
}
