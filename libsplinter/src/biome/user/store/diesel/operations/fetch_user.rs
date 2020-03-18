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

use super::super::{SplinterUser, UserStoreError};
use super::UserStoreOperations;
use crate::biome::datastore::models::UserModel;
use crate::biome::datastore::schema::splinter_user;
use diesel::{prelude::*, result::Error::NotFound};

pub(in super::super) trait UserStoreFetchUserOperation {
    fn fetch_user(&self, user_id: &str) -> Result<SplinterUser, UserStoreError>;
}

impl<'a, C> UserStoreFetchUserOperation for UserStoreOperations<'a, C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    <C as diesel::Connection>::Backend: 'static,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn fetch_user(&self, user_id: &str) -> Result<SplinterUser, UserStoreError> {
        let user = splinter_user::table
            .find(user_id)
            .first::<UserModel>(self.conn)
            .map(Some)
            .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
            .map_err(|err| UserStoreError::OperationError {
                context: "Failed to fetch user".to_string(),
                source: Box::new(err),
            })?
            .ok_or_else(|| {
                UserStoreError::NotFoundError(format!("Failed to find user: {}", user_id))
            })?;
        Ok(SplinterUser::from(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFetchUser;

    impl UserStoreFetchUserOperation for MockFetchUser {
        fn fetch_user(&self, _: &str) -> Result<SplinterUser, UserStoreError> {
            Ok(SplinterUser {
                id: "this_is_an_id".to_string(),
            })
        }
    }

    #[test]
    fn test_fetch_user() {
        let user_id = String::from("this_is_an_id");

        let mock = MockFetchUser {};

        let user = mock.fetch_user(&user_id).unwrap();

        assert_eq!(user_id, user.id);
    }
}
