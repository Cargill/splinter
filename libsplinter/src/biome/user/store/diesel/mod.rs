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

mod operations;

use super::{SplinterUser, UserStore, UserStoreError};
use crate::database::ConnectionPool;
use operations::add_user::UserStoreAddUserOperation as _;
use operations::delete_user::UserStoreDeleteUserOperation as _;
use operations::fetch_user::UserStoreFetchUserOperation as _;
use operations::list_users::UserStoreListUsersOperation as _;
use operations::update_user::UserStoreUpdateUserOperation as _;
use operations::UserStoreOperations;

/// Manages creating, updating and fetching SplinterUser from the databae
#[derive(Clone)]
pub struct SplinterUserStore {
    connection_pool: ConnectionPool,
}

impl SplinterUserStore {
    /// Creates a new SplinterUserStore
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool to the database
    // Allow dead code if diesel feature is not enabled
    #[allow(dead_code)]
    pub fn new(connection_pool: ConnectionPool) -> SplinterUserStore {
        SplinterUserStore { connection_pool }
    }
}

impl UserStore<SplinterUser> for SplinterUserStore {
    fn add_user(&mut self, user: SplinterUser) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).add_user(user.into())
    }

    fn update_user(&mut self, updated_user: SplinterUser) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).update_user(updated_user)
    }

    fn remove_user(&mut self, id: &str) -> Result<(), UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).delete_user(id)
    }

    fn fetch_user(&self, id: &str) -> Result<SplinterUser, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).fetch_user(id)
    }

    fn list_users(&self) -> Result<Vec<SplinterUser>, UserStoreError> {
        UserStoreOperations::new(&*self.connection_pool.get()?).list_users()
    }
}
