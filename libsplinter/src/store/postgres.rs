// Copyright 2018-2022 Cargill Incorporated
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

use crate::error::InternalError;
use crate::migrations::any_pending_postgres_migrations;

use super::StoreFactory;

/// Create a Postgres connection pool.
///
/// # Arguments
///
/// * url - a valid postges connection url
///
/// # Errors
///
/// An [InternalError] is returned if
/// * The pool cannot be created
/// * The database requires any pending migrations
pub fn create_postgres_connection_pool(
    url: &str,
) -> Result<Pool<ConnectionManager<PgConnection>>, InternalError> {
    let connection_manager = ConnectionManager::<diesel::pg::PgConnection>::new(url);
    let pool = Pool::builder().build(connection_manager).map_err(|err| {
        InternalError::from_source_with_prefix(
            Box::new(err),
            "Failed to build connection pool".to_string(),
        )
    })?;
    let conn = pool
        .get()
        .map_err(|err| InternalError::from_source(Box::new(err)))?;
    if !any_pending_postgres_migrations(&conn)? {
        return Err(InternalError::with_message(String::from(
            "This version of splinter requires migrations that are not yet applied  to the \
            database. Run `splinter database migrate` to apply migrations before running splinterd",
        )));
    }

    Ok(pool)
}

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
    ) -> Box<dyn crate::rbac::store::RoleBasedAuthorizationStore> {
        Box::new(crate::rbac::store::DieselRoleBasedAuthorizationStore::new(
            self.pool.clone(),
        ))
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

    #[cfg(feature = "service-lifecycle-store")]
    fn get_lifecycle_store(&self) -> Box<dyn crate::runtime::service::LifecycleStore + Send> {
        Box::new(crate::runtime::service::DieselLifecycleStore::new(
            self.pool.clone(),
        ))
    }
}
