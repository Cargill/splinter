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

//! Provides an API for managing Biome REST API endpoints
//!
//! Below is an example of building an instance of BiomeRestResourceManager and passing its
//! resources to a running instance of `RestApi`.
//!
//! ```no_run
//! use splinter::rest_api::{Resource, Method, RestApiBuilder, RestResourceProvider};
//! use splinter::biome::rest_api::{BiomeRestResourceManager, BiomeRestResourceManagerBuilder};
//! use splinter::database::{self, ConnectionPool};
//!
//! let connection_pool: ConnectionPool = database::ConnectionPool::new_pg(
//!            "postgres://db_admin:db_password@0.0.0.0:5432/db",
//!        )
//!        .unwrap();
//!
//! let biome_rest_provider_builder: BiomeRestResourceManagerBuilder = Default::default();
//! let biome_rest_provider = biome_rest_provider_builder
//!             .with_user_store(connection_pool.clone())
//!             .build()
//!             .unwrap();
//!
//! RestApiBuilder::new()
//!     .add_resources(biome_rest_provider.resources())
//!     .with_bind("localhost:8080")
//!     .build()
//!     .unwrap()
//!     .run();
//! ```

#[cfg(feature = "rest-api-actix")]
mod actix;
mod config;
mod error;
mod resources;

use std::sync::Arc;

#[cfg(feature = "biome-refresh-tokens")]
use crate::biome::refresh_tokens::store::{diesel::DieselRefreshTokenStore, RefreshTokenStore};
use crate::database::ConnectionPool;
use crate::rest_api::{Resource, RestResourceProvider};

#[cfg(all(feature = "biome-key-management", feature = "rest-api-actix",))]
use self::actix::key_management::{
    make_key_management_route, make_key_management_route_with_public_key,
};

#[cfg(feature = "biome-key-management")]
use super::key_management::{
    store::{diesel::postgres::PostgresKeyStore, KeyStore},
    Key,
};
use super::user::store::{diesel::DieselUserStore, UserStore};
use crate::rest_api::secrets::{AutoSecretManager, SecretManager};

pub use config::{BiomeRestConfig, BiomeRestConfigBuilder};
pub use error::BiomeRestResourceManagerBuilderError;

#[cfg(all(feature = "rest-api-actix", feature = "biome-refresh-tokens"))]
use self::actix::logout::make_logout_route;
#[cfg(all(feature = "biome-credentials", feature = "rest-api-actix"))]
use self::actix::register::make_register_route;
#[cfg(all(
    feature = "biome-credentials",
    feature = "biome-refresh-tokens",
    feature = "rest-api-actix"
))]
use self::actix::token::make_token_route;
#[cfg(all(
    feature = "biome-credentials",
    feature = "biome-key-management",
    feature = "rest-api-actix",
))]
use self::actix::user::make_user_routes;
#[cfg(all(feature = "biome-credentials", feature = "rest-api-actix",))]
use self::actix::{login::make_login_route, user::make_list_route, verify::make_verify_route};
#[cfg(feature = "biome-credentials")]
use super::credentials::store::CredentialsStore;

#[allow(unused_imports)]
use crate::rest_api::sessions::AccessTokenIssuer;

/// Provides the REST API endpoints for biome
///
/// The following endponts are provided
///
/// * `GET /biome/users/keys` - Get all keys for authorized user
/// * `POST /biome/users/keys` - Create a new key for authorized user
/// * `PATCH /biome/users/keys` - Update the display name associated with a key for
///    an authorized user.
/// * `GET /biome/users/keys/{public_key}` - Retrieve a key for an authroized user that has
///    `public_key`
/// * `DELETE /biome/users/keys/{public_key}` - delete a  key for an authorized user that has
///    `public key`
/// * `POST /biome/login` - Login enpoint for getting access tokens and refresh tokens
/// * `PATCH /biome/login` - Login endpoint for removing refresh tokens
/// * `POST /biome/register - Creates credentials for a user
/// * `POST /biome/token` - Creates a new access token for the authorized user
/// * `POST /biome/verify` - Verify a users password
/// * `POST /biome/users` - Create new user
/// * `GET /biome/user` - Get a list of all users in biome
/// * `PUT /biome/user/{id}` - Update user with specified ID
/// * `GET /biome/user/{id}` - Retrieve user with specified ID
/// * `DELETE /biome/user/{id}` - Remove user with specified ID
pub struct BiomeRestResourceManager<
    C: CredentialsStore + Clone + 'static,
    K: KeyStore<Key> + Clone + 'static,
    R: RefreshTokenStore + Clone + 'static,
    U: UserStore + Clone + 'static,
> {
    #[cfg(feature = "biome-rest-api")]
    user_store: U,
    #[cfg(feature = "biome-key-management",)]
    key_store: K,
    #[cfg(feature = "biome-rest-api")]
    rest_config: Arc<BiomeRestConfig>,
    token_secret_manager: Arc<dyn SecretManager>,
    #[cfg(feature = "biome-refresh-tokens")]
    refresh_token_secret_manager: Arc<dyn SecretManager>,
    #[cfg(feature = "biome-refresh-tokens")]
    refresh_token_store: R,
    #[cfg(feature = "biome-credentials")]
    credentials_store: Option<C>,
}

#[cfg(feature = "biome-rest-api")]
impl<
        C: CredentialsStore + Clone + 'static,
        K: KeyStore<Key> + Clone + 'static,
        R: RefreshTokenStore + Clone + 'static,
        U: UserStore + Clone + 'static,
    > RestResourceProvider for BiomeRestResourceManager<C, K, R, U>
{
    fn resources(&self) -> Vec<Resource> {
        // This needs to be mutable if biome-credentials feature is enable
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        match &self.credentials_store {
            Some(credentials_store) => {
                #[cfg(all(feature = "biome-credentials", feature = "rest-api-actix",))]
                {
                    resources.push(make_list_route(credentials_store.clone()));
                    resources.push(make_verify_route(
                        credentials_store.clone(),
                        self.rest_config.clone(),
                        self.token_secret_manager.clone(),
                    ));
                    #[cfg(not(feature = "biome-refresh-tokens"))]
                    {
                        resources.push(make_login_route(
                            credentials_store.clone(),
                            self.rest_config.clone(),
                            Arc::new(AccessTokenIssuer::new(self.token_secret_manager.clone())),
                        ));
                    }
                }
                #[cfg(all(
                    feature = "biome-credentials",
                    feature = "biome-key-management",
                    feature = "rest-api-actix",
                ))]
                {
                    resources.push(make_user_routes(
                        self.rest_config.clone(),
                        self.token_secret_manager.clone(),
                        credentials_store.clone(),
                        self.user_store.clone(),
                        self.key_store.clone(),
                    ));
                }
                #[cfg(all(
                    feature = "biome-credentials",
                    feature = "biome-refresh-tokens",
                    feature = "rest-api-actix",
                ))]
                {
                    resources.push(make_login_route(
                        credentials_store.clone(),
                        self.refresh_token_store.clone(),
                        self.rest_config.clone(),
                        Arc::new(AccessTokenIssuer::new(
                            self.token_secret_manager.clone(),
                            self.refresh_token_secret_manager.clone(),
                        )),
                    ));
                    resources.push(make_token_route(
                        self.refresh_token_store.clone(),
                        self.token_secret_manager.clone(),
                        self.refresh_token_secret_manager.clone(),
                        Arc::new(AccessTokenIssuer::new(
                            self.token_secret_manager.clone(),
                            self.refresh_token_secret_manager.clone(),
                        )),
                        self.rest_config.clone(),
                    ));
                }
                #[cfg(all(feature = "biome-refresh-tokens", feature = "rest-api-actix",))]
                {
                    resources.push(make_logout_route(
                        self.refresh_token_store.clone(),
                        self.token_secret_manager.clone(),
                        self.rest_config.clone(),
                    ));
                }

                #[cfg(all(feature = "biome-credentials", feature = "rest-api-actix"))]
                {
                    resources.push(make_register_route(
                        credentials_store.clone(),
                        self.user_store.clone(),
                        self.rest_config.clone(),
                    ));
                }
            }
            None => {
                debug!(
                    "Credentials store not provided. Credentials REST API resources will not be'
                ' included in the biome endpoints."
                );
            }
        };
        #[cfg(all(feature = "biome-key-management", feature = "rest-api-actix",))]
        {
            resources.push(make_key_management_route(
                self.rest_config.clone(),
                self.key_store.clone(),
                self.token_secret_manager.clone(),
            ));
            resources.push(make_key_management_route_with_public_key(
                self.rest_config.clone(),
                self.key_store.clone(),
                self.token_secret_manager.clone(),
            ));
        }
        resources
    }
}

/// Builder for BiomeRestResourceManager
pub struct BiomeRestResourceManagerBuilder<
    C: CredentialsStore + Clone + 'static,
    K: KeyStore<Key> + Clone + 'static,
    R: RefreshTokenStore + Clone + 'static,
    U: UserStore + Clone + 'static,
> {
    user_store: Option<U>,
    #[cfg(feature = "biome-key-management")]
    key_store: Option<K>,
    rest_config: Option<BiomeRestConfig>,
    token_secret_manager: Option<Arc<dyn SecretManager>>,
    #[cfg(feature = "biome-refresh-tokens")]
    refresh_token_secret_manager: Option<Arc<dyn SecretManager>>,
    #[cfg(feature = "biome-refresh-tokens")]
    refresh_token_store: Option<R>,
    #[cfg(feature = "biome-credentials")]
    credentials_store: Option<C>,
}

impl<
        C: CredentialsStore + Clone + 'static,
        K: KeyStore<Key> + Clone + 'static,
        R: RefreshTokenStore + Clone + 'static,
        U: UserStore + Clone + 'static,
    > BiomeRestResourceManagerBuilder<C, K, R, U>
{
    /// Sets a UserStore for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `pool`: ConnectionPool to database that will serve as backend for UserStore
    pub fn with_user_store(mut self, store: U) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.user_store = Some(store);
        self
    }

    /// Sets a KeyStore for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `pool`: ConnectionPool to database that will serve as backend for KeyStore
    #[cfg(feature = "biome-key-management")]
    pub fn with_key_store(mut self, store: K) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.key_store = Some(store);
        self
    }

    /// Sets a BiomeRestConfig for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `config`: the BiomeRestConfig that will be used to configure the Biome resources
    pub fn with_rest_config(
        mut self,
        config: BiomeRestConfig,
    ) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.rest_config = Some(config);
        self
    }

    #[cfg(feature = "biome-credentials")]
    /// Sets a CredentialsStore for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `pool`: ConnectionPool to database that will serve as backend for CredentialsStore
    pub fn with_credentials_store(
        mut self,
        credentials_store: C,
    ) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.credentials_store = Some(credentials_store);
        self
    }

    /// Sets a SecretManager for JWT tokens for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `secret_manager`: the SecretManager to be used for fetching and generating secrets to
    ///   sign and verify JWT tokens
    pub fn with_token_secret_manager(
        mut self,
        secret_manager: impl SecretManager + 'static,
    ) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.token_secret_manager = Some(Arc::new(secret_manager));
        self
    }

    /// Sets a SecretManager for the refresh tokens for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `secret_manager`: the SecretManager to be used for fetching and generating secrets to
    ///   sign and verify JWT tokens
    #[cfg(feature = "biome-refresh-tokens")]
    pub fn with_refresh_token_secret_manager(
        mut self,
        secret_manager: impl SecretManager + 'static,
    ) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.refresh_token_secret_manager = Some(Arc::new(secret_manager));
        self
    }

    /// Sets a Refresh token store for the refresh tokens for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `store`: the RefreshTokenStore to be used for performing CRUD operation on a
    ///   serialized refresh token.
    ///
    #[cfg(feature = "biome-refresh-tokens")]
    pub fn with_refresh_token_store(
        mut self,
        store: R,
    ) -> BiomeRestResourceManagerBuilder<C, K, R, U> {
        self.refresh_token_store = Some(store);
        self
    }

    /// Consumes the builder and returns a BiomeRestResourceManager
    pub fn build(
        self,
    ) -> Result<BiomeRestResourceManager<C, K, R, U>, BiomeRestResourceManagerBuilderError> {
        let user_store = self.user_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing user store".to_string(),
            )
        })?;
        #[cfg(feature = "biome-key-management")]
        let key_store = self.key_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing key store".to_string(),
            )
        })?;
        let rest_config = match self.rest_config {
            Some(config) => config,
            None => {
                debug!("Building BiomeRestResourceManager with default config.");
                BiomeRestConfigBuilder::default().build()?
            }
        };

        let token_secret_manager = self.token_secret_manager.unwrap_or_else(|| {
            debug!("Building BiomeRestResourceManager with default SecretManager.");
            Arc::new(AutoSecretManager::default())
        });

        #[cfg(feature = "biome-refresh-tokens")]
        let refresh_token_secret_manager = self.refresh_token_secret_manager.unwrap_or_else(|| {
            debug!("Building BiomeRestResourceManager with default token SecretManager.");
            Arc::new(AutoSecretManager::default())
        });

        #[cfg(feature = "biome-refresh-tokens")]
        let refresh_token_store = self.refresh_token_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing refresh token store".to_string(),
            )
        })?;

        Ok(BiomeRestResourceManager {
            user_store,
            #[cfg(feature = "biome-key-management")]
            key_store,
            rest_config: Arc::new(rest_config),
            token_secret_manager,
            #[cfg(feature = "biome-refresh-tokens")]
            refresh_token_secret_manager,
            #[cfg(feature = "biome-refresh-tokens")]
            refresh_token_store,
            #[cfg(feature = "biome-credentials")]
            credentials_store: self.credentials_store,
        })
    }
}
