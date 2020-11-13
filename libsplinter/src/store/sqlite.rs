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

//! Implementation of a `StoreFactory` for SQLite

use diesel::{
    r2d2::{ConnectionManager, Pool},
    sqlite::SqliteConnection,
};

use super::StoreFactory;

/// A `StoreFactory` backed by a SQLite database.
pub struct SqliteStoreFactory {
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl SqliteStoreFactory {
    /// Create a new `SqliteStoreFactory`.
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Self { pool }
    }
}

impl StoreFactory for SqliteStoreFactory {
    #[cfg(feature = "biome-credentials")]
    fn get_biome_credentials_store(&self) -> Box<dyn crate::biome::CredentialsStore> {
        Box::new(crate::biome::DieselCredentialsStore::new(self.pool.clone()))
    }

    #[cfg(feature = "biome-key-management")]
    fn get_biome_key_store(&self) -> Box<dyn crate::biome::KeyStore> {
        Box::new(crate::biome::DieselKeyStore::new(self.pool.clone()))
    }

    #[cfg(feature = "biome-credentials")]
    fn get_biome_refresh_token_store(&self) -> Box<dyn crate::biome::RefreshTokenStore> {
        Box::new(crate::biome::DieselRefreshTokenStore::new(
            self.pool.clone(),
        ))
    }

    fn get_biome_user_store(&self) -> Box<dyn crate::biome::UserStore> {
        Box::new(crate::biome::DieselUserStore::new(self.pool.clone()))
    }

    #[cfg(feature = "biome-oauth")]
    fn get_biome_oauth_user_store(&self) -> Box<dyn crate::biome::OAuthUserStore> {
        Box::new(crate::biome::DieselOAuthUserStore::new(self.pool.clone()))
    }
}
