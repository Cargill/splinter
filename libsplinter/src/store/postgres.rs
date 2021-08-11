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

//! Implementation of a `StoreFactory` for PostgreSQL

use diesel::{
    pg::PgConnection,
    r2d2::{ConnectionManager, Pool},
};

use super::StoreFactory;

/// A `StoryFactory` backed by a PostgreSQL database.
pub struct PgStoreFactory {
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl PgStoreFactory {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Self { pool }
    }
}

impl StoreFactory for PgStoreFactory {
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

    #[cfg(feature = "oauth")]
    fn get_biome_oauth_user_session_store(&self) -> Box<dyn crate::biome::OAuthUserSessionStore> {
        Box::new(crate::biome::DieselOAuthUserSessionStore::new(
            self.pool.clone(),
        ))
    }

    #[cfg(feature = "admin-service")]
    fn get_admin_service_store(&self) -> Box<dyn crate::admin::store::AdminServiceStore> {
        Box::new(crate::admin::store::diesel::DieselAdminServiceStore::new(
            self.pool.clone(),
        ))
    }

    #[cfg(feature = "oauth")]
    fn get_oauth_inflight_request_store(
        &self,
    ) -> Box<dyn crate::oauth::store::InflightOAuthRequestStore> {
        Box::new(crate::oauth::store::DieselInflightOAuthRequestStore::new(
            self.pool.clone(),
        ))
    }

    #[cfg(feature = "registry")]
    fn get_registry_store(&self) -> Box<dyn crate::registry::RwRegistry> {
        Box::new(crate::registry::DieselRegistry::new(self.pool.clone()))
    }

    #[cfg(feature = "authorization-handler-rbac")]
    fn get_role_based_authorization_store(
        &self,
    ) -> Box<dyn crate::rest_api::auth::authorization::rbac::store::RoleBasedAuthorizationStore>
    {
        Box::new(
            crate::rest_api::auth::authorization::rbac::store::DieselRoleBasedAuthorizationStore::new(
                self.pool.clone(),
            ),
        )
    }

    #[cfg(feature = "biome-profile")]
    fn get_biome_user_profile_store(&self) -> Box<dyn crate::biome::UserProfileStore> {
        Box::new(crate::biome::DieselUserProfileStore::new(self.pool.clone()))
    }

    #[cfg(feature = "node-id-store")]
    fn get_node_id_store(&self) -> Box<dyn crate::node_id::store::NodeIdStore> {
        Box::new(crate::node_id::store::diesel::DieselNodeIdStore::new(
            self.pool.clone(),
        ))
    }
}
