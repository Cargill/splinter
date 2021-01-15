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

use super::CredentialsStoreOperations;
use crate::biome::credentials::store::diesel::{
    schema::user_credentials, CredentialsStoreError, UsernameId,
};
use crate::biome::credentials::store::CredentialsModel;
use diesel::prelude::*;

pub(in crate::biome::credentials) trait CredentialsStoreListUsernamesOperation {
    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError>;
}

impl<'a, C> CredentialsStoreListUsernamesOperation for CredentialsStoreOperations<'a, C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    <C as diesel::Connection>::Backend: 'static,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError> {
        let usernames = user_credentials::table
            .select(user_credentials::all_columns)
            .load::<CredentialsModel>(self.conn)
            .map(Some)
            .map_err(|err| CredentialsStoreError::QueryError {
                context: "Failed to fetch usernames".to_string(),
                source: Box::new(err),
            })?
            .ok_or_else(|| {
                CredentialsStoreError::NotFoundError(
                    "Could not get all user credentials from storage".to_string(),
                )
            })?
            .into_iter()
            .map(UsernameId::from)
            .collect();
        Ok(usernames)
    }
}
