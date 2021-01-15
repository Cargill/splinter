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
use crate::biome::credentials::store::diesel::schema::user_credentials;
use crate::biome::credentials::store::error::CredentialsStoreError;
use crate::biome::credentials::store::{Credentials, CredentialsModel};
use diesel::{prelude::*, result::Error::NotFound};

pub(in crate::biome::credentials) trait CredentialsStoreFetchCredentialByUsernameOperation {
    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError>;
}

impl<'a, C> CredentialsStoreFetchCredentialByUsernameOperation for CredentialsStoreOperations<'a, C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    <C as diesel::Connection>::Backend: 'static,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        let credentials = user_credentials::table
            .filter(user_credentials::username.eq(username))
            .first::<CredentialsModel>(self.conn)
            .map(Some)
            .or_else(|err| if err == NotFound { Ok(None) } else { Err(err) })
            .map_err(|err| CredentialsStoreError::QueryError {
                context: "Failed to fetch credentials by username".to_string(),
                source: Box::new(err),
            })?
            .ok_or_else(|| {
                CredentialsStoreError::NotFoundError(format!(
                    "Failed to find credentials: {}",
                    username
                ))
            })?;
        Ok(Credentials::from(credentials))
    }
}
