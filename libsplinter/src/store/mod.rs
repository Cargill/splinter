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

//! Contains a `StoreFactory` trait, which is an abstract factory for building stores
//! backed by a single storage mechanism (e.g. database)
pub mod command;
#[cfg(all(feature = "store-factory", feature = "memory"))]
pub mod memory;
#[cfg(feature = "diesel")]
pub(crate) mod pool;
#[cfg(all(feature = "store-factory", feature = "postgres"))]
pub mod postgres;
#[cfg(all(feature = "store-factory", feature = "sqlite"))]
pub mod sqlite;

/// An abstract factory for creating Splinter stores backed by the same storage
#[cfg(feature = "store-factory")]
pub trait StoreFactory {
    /// Get a new `CredentialsStore`
    #[cfg(feature = "biome-credentials")]
    fn get_biome_credentials_store(&self) -> Box<dyn crate::biome::CredentialsStore>;

    /// Get a new `KeyStore`
    #[cfg(feature = "biome-key-management")]
    fn get_biome_key_store(&self) -> Box<dyn crate::biome::KeyStore>;

    /// Get a new `RefreshTokenStore`
    #[cfg(feature = "biome-credentials")]
    fn get_biome_refresh_token_store(&self) -> Box<dyn crate::biome::RefreshTokenStore>;

    /// Get a new `OAuthUserSessionStore`
    #[cfg(feature = "oauth")]
    fn get_biome_oauth_user_session_store(&self) -> Box<dyn crate::biome::OAuthUserSessionStore>;

    #[cfg(feature = "admin-service")]
    fn get_admin_service_store(&self) -> Box<dyn crate::admin::store::AdminServiceStore>;

    #[cfg(feature = "oauth")]
    fn get_oauth_inflight_request_store(
        &self,
    ) -> Box<dyn crate::oauth::store::InflightOAuthRequestStore>;

    #[cfg(feature = "registry")]
    fn get_registry_store(&self) -> Box<dyn crate::registry::RwRegistry>;

    #[cfg(feature = "authorization-handler-rbac")]
    fn get_role_based_authorization_store(
        &self,
    ) -> Box<dyn crate::rbac::store::RoleBasedAuthorizationStore>;

    #[cfg(feature = "biome-profile")]
    fn get_biome_user_profile_store(&self) -> Box<dyn crate::biome::UserProfileStore>;

    #[cfg(feature = "node-id-store")]
    fn get_node_id_store(&self) -> Box<dyn crate::node_id::store::NodeIdStore>;

    #[cfg(feature = "service-lifecycle-store")]
    fn get_lifecycle_store(&self) -> Box<dyn crate::runtime::service::LifecycleStore + Send>;
}
