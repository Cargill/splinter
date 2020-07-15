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

pub(in crate::biome) mod models;
mod operations;
pub(in crate::biome) mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use super::{Credentials, CredentialsStore, CredentialsStoreError, UsernameId};

use models::CredentialsModel;
use operations::add_credentials::CredentialsStoreAddCredentialsOperation as _;
use operations::fetch_credential_by_id::CredentialsStoreFetchCredentialByIdOperation as _;
use operations::fetch_credential_by_username::CredentialsStoreFetchCredentialByUsernameOperation as _;
use operations::fetch_username::CredentialsStoreFetchUsernameOperation as _;
use operations::list_usernames::CredentialsStoreListUsernamesOperation as _;
use operations::remove_credentials::CredentialsStoreRemoveCredentialsOperation as _;
use operations::update_credentials::CredentialsStoreUpdateCredentialsOperation as _;
use operations::CredentialsStoreOperations;

/// Manages creating, updating and fetching SplinterCredentials from the database
pub struct DieselCredentialsStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselCredentialsStore<C> {
    /// Creates a new DieselCredentialsStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselCredentialsStore { connection_pool }
    }
}

impl<C> CredentialsStore for DieselCredentialsStore<C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn add_credentials(&self, credentials: Credentials) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).add_credentials(credentials)
    }

    fn update_credentials(
        &self,
        user_id: &str,
        username: &str,
        password: &str,
    ) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .update_credentials(user_id, username, password)
    }

    fn remove_credentials(&self, user_id: &str) -> Result<(), CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).remove_credentials(user_id)
    }

    fn fetch_credential_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_id(user_id)
    }

    fn fetch_credential_by_username(
        &self,
        username: &str,
    ) -> Result<Credentials, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?)
            .fetch_credential_by_username(username)
    }

    fn fetch_username_by_id(&self, user_id: &str) -> Result<UsernameId, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).fetch_username_by_id(user_id)
    }

    fn list_usernames(&self) -> Result<Vec<UsernameId>, CredentialsStoreError> {
        CredentialsStoreOperations::new(&*self.connection_pool.get()?).list_usernames()
    }
}

impl From<CredentialsModel> for UsernameId {
    fn from(user_credentials: CredentialsModel) -> Self {
        Self {
            user_id: user_credentials.user_id,
            username: user_credentials.username,
        }
    }
}

impl From<CredentialsModel> for Credentials {
    fn from(user_credentials: CredentialsModel) -> Self {
        Self {
            user_id: user_credentials.user_id,
            username: user_credentials.username,
            password: user_credentials.password,
        }
    }
}
