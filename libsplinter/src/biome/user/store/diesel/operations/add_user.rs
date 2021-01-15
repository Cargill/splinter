// Copyright 2018-2021 Cargill Incorporated
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

use super::UserStoreOperations;
use crate::biome::user::store::diesel::models::UserModel;
use crate::biome::user::store::diesel::schema::splinter_user;
use crate::biome::user::store::error::UserStoreError;

use diesel::{dsl::insert_into, prelude::*};

pub(in crate::biome::user) trait UserStoreAddUserOperation {
    fn add_user(&self, user_model: UserModel) -> Result<(), UserStoreError>;
}

impl<'a, C> UserStoreAddUserOperation for UserStoreOperations<'a, C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    <C as diesel::Connection>::Backend: 'static,
{
    fn add_user(&self, user_model: UserModel) -> Result<(), UserStoreError> {
        insert_into(splinter_user::table)
            .values(&vec![user_model])
            .execute(self.conn)
            .map(|_| ())
            .map_err(|err| UserStoreError::OperationError {
                context: "Failed to add user".to_string(),
                source: Box::new(err),
            })?;
        Ok(())
    }
}
