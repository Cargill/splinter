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

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use crate::biome::refresh_tokens::store::{RefreshTokenError, RefreshTokenStore};

use operations::{
    add_token::RefreshTokenStoreAddTokenOperation,
    fetch_token::RefreshTokenStoreFetchTokenOperation,
    remove_token::RefreshTokenStoreRemoveTokenOperation,
    update_token::RefreshTokenStoreUpdateTokenOperation, RefreshTokenStoreOperations,
};

pub struct DieselRefreshTokenStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselRefreshTokenStore<C> {
    pub fn new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        Self { connection_pool }
    }
}

impl<C> RefreshTokenStore for DieselRefreshTokenStore<C>
where
    C: diesel::Connection,
    <C as diesel::Connection>::Backend: diesel::backend::SupportsDefaultKeyword,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn add_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).add_token(user_id, token)
    }
    fn remove_token(&self, user_id: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).remove_token(user_id)
    }
    fn update_token(&self, user_id: &str, token: &str) -> Result<(), RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).update_token(user_id, token)
    }
    fn fetch_token(&self, user_id: &str) -> Result<String, RefreshTokenError> {
        RefreshTokenStoreOperations::new(&*self.connection_pool.get()?).fetch_token(user_id)
    }
}
