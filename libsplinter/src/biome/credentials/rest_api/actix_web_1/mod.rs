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

mod authorize;
mod config;
mod login;
mod logout;
mod register;
mod token;
mod user;
mod verify;

use std::sync::Arc;

#[cfg(feature = "biome-key-management")]
use crate::biome::key_management::store::KeyStore;
use crate::biome::{
    credentials::store::CredentialsStore, refresh_tokens::store::RefreshTokenStore,
};
use crate::error::InvalidStateError;
use crate::rest_api::{
    auth::identity::biome::BiomeUserIdentityProvider,
    secrets::{AutoSecretManager, SecretManager},
    sessions::{default_validation, AccessTokenIssuer},
    Resource, RestResourceProvider,
};

pub use config::{BiomeCredentialsRestConfig, BiomeCredentialsRestConfigBuilder};

/// Provides the following REST API endpoints for Biome credentials:
///
/// * `POST /biome/login` - Login enpoint for getting access tokens and refresh tokens
/// * `PATCH /biome/logout` - Login endpoint for removing refresh tokens
/// * `POST /biome/register - Creates credentials for a user
/// * `POST /biome/token` - Creates a new access token for the authorized user
/// * `POST /biome/verify` - Verify a users password
/// * `GET /biome/user` - Get a list of all users in biome
/// * `PUT /biome/user/{id}` - Update user with specified ID
/// * `GET /biome/user/{id}` - Retrieve user with specified ID
/// * `DELETE /biome/user/{id}` - Remove user with specified ID
pub struct BiomeCredentialsRestResourceProvider {
    #[cfg(feature = "biome-key-management")]
    key_store: Arc<dyn KeyStore>,
    credentials_config: Arc<BiomeCredentialsRestConfig>,
    token_secret_manager: Arc<dyn SecretManager>,
    refresh_token_secret_manager: Arc<dyn SecretManager>,
    refresh_token_store: Arc<dyn RefreshTokenStore>,
    credentials_store: Arc<dyn CredentialsStore>,
}

impl BiomeCredentialsRestResourceProvider {
    /// Creates a new Biome user identity provider for the Splinter REST API
    pub fn get_identity_provider(&self) -> BiomeUserIdentityProvider {
        BiomeUserIdentityProvider::new(
            self.token_secret_manager.clone(),
            default_validation(&self.credentials_config.issuer()),
        )
    }
}

impl RestResourceProvider for BiomeCredentialsRestResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        let mut resources = Vec::new();

        resources.push(user::make_list_route(self.credentials_store.clone()));
        resources.push(verify::make_verify_route(
            self.credentials_store.clone(),
            self.credentials_config.clone(),
            self.token_secret_manager.clone(),
        ));
        resources.push(login::make_login_route(
            self.credentials_store.clone(),
            self.refresh_token_store.clone(),
            self.credentials_config.clone(),
            Arc::new(AccessTokenIssuer::new(
                self.token_secret_manager.clone(),
                self.refresh_token_secret_manager.clone(),
            )),
        ));
        resources.push(token::make_token_route(
            self.refresh_token_store.clone(),
            self.token_secret_manager.clone(),
            self.refresh_token_secret_manager.clone(),
            Arc::new(AccessTokenIssuer::new(
                self.token_secret_manager.clone(),
                self.refresh_token_secret_manager.clone(),
            )),
            self.credentials_config.clone(),
        ));
        resources.push(logout::make_logout_route(
            self.refresh_token_store.clone(),
            self.token_secret_manager.clone(),
            self.credentials_config.clone(),
        ));

        resources.push(register::make_register_route(
            self.credentials_store.clone(),
            self.credentials_config.clone(),
        ));

        #[cfg(feature = "biome-key-management")]
        {
            resources.push(user::make_user_routes(
                self.credentials_config.clone(),
                self.credentials_store.clone(),
                self.key_store.clone(),
            ));
        }

        resources
    }
}

/// Builder for BiomeCredentialsRestResourceProvider
#[derive(Default)]
pub struct BiomeCredentialsRestResourceProviderBuilder {
    #[cfg(feature = "biome-key-management")]
    key_store: Option<Arc<dyn KeyStore>>,
    credentials_config: Option<BiomeCredentialsRestConfig>,
    token_secret_manager: Option<Arc<dyn SecretManager>>,
    refresh_token_secret_manager: Option<Arc<dyn SecretManager>>,
    refresh_token_store: Option<Arc<dyn RefreshTokenStore>>,
    credentials_store: Option<Arc<dyn CredentialsStore>>,
}

impl BiomeCredentialsRestResourceProviderBuilder {
    /// Sets a KeyStore for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `store`: the key management store to be used by the provided endpoints
    #[cfg(feature = "biome-key-management")]
    pub fn with_key_store(
        mut self,
        store: impl KeyStore + 'static,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.key_store = Some(Arc::new(store));
        self
    }

    /// Sets a BiomeCredentialsRestConfig for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `config`: the BiomeCredentialsRestConfig that will be used to configure the Biome resources
    pub fn with_credentials_config(
        mut self,
        config: BiomeCredentialsRestConfig,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.credentials_config = Some(config);
        self
    }

    /// Sets a CredentialsStore for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `store`: the credentials store to be used by the provided endpoints
    pub fn with_credentials_store(
        mut self,
        store: impl CredentialsStore + 'static,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.credentials_store = Some(Arc::new(store));
        self
    }

    /// Sets a SecretManager for JWT tokens for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `secret_manager`: the SecretManager to be used for fetching and generating secrets to
    ///   sign and verify JWT tokens
    pub fn with_token_secret_manager(
        mut self,
        secret_manager: impl SecretManager + 'static,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.token_secret_manager = Some(Arc::new(secret_manager));
        self
    }

    /// Sets a SecretManager for the refresh tokens for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `secret_manager`: the SecretManager to be used for fetching and generating secrets to
    ///   sign and verify refresh tokens
    pub fn with_refresh_token_secret_manager(
        mut self,
        secret_manager: impl SecretManager + 'static,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.refresh_token_secret_manager = Some(Arc::new(secret_manager));
        self
    }

    /// Sets a Refresh token store for the refresh tokens for the BiomeCredentialsRestResourceProvider
    ///
    /// # Arguments
    ///
    /// * `store`: the RefreshTokenStore to be used for performing CRUD operation on a
    ///   serialized refresh token.
    pub fn with_refresh_token_store(
        mut self,
        store: impl RefreshTokenStore + 'static,
    ) -> BiomeCredentialsRestResourceProviderBuilder {
        self.refresh_token_store = Some(Arc::new(store));
        self
    }

    /// Consumes the builder and returns a BiomeCredentialsRestResourceProvider
    pub fn build(self) -> Result<BiomeCredentialsRestResourceProvider, InvalidStateError> {
        #[cfg(feature = "biome-key-management")]
        let key_store = self
            .key_store
            .ok_or_else(|| InvalidStateError::with_message("Missing key store".to_string()))?;

        let credentials_config = match self.credentials_config {
            Some(config) => config,
            None => {
                debug!("Building BiomeCredentialsRestResourceProvider with default config.");
                BiomeCredentialsRestConfigBuilder::default().build()?
            }
        };

        let token_secret_manager = self.token_secret_manager.unwrap_or_else(|| {
            debug!("Building BiomeCredentialsRestResourceProvider with default SecretManager.");
            Arc::new(AutoSecretManager::default())
        });

        let refresh_token_secret_manager = self.refresh_token_secret_manager.unwrap_or_else(|| {
            debug!(
                "Building BiomeCredentialsRestResourceProvider with default token SecretManager."
            );
            Arc::new(AutoSecretManager::default())
        });

        let refresh_token_store = self.refresh_token_store.ok_or_else(|| {
            InvalidStateError::with_message("Missing refresh token store".to_string())
        })?;

        let credentials_store = self.credentials_store.ok_or_else(|| {
            InvalidStateError::with_message("Missing credentials store".to_string())
        })?;

        Ok(BiomeCredentialsRestResourceProvider {
            #[cfg(feature = "biome-key-management")]
            key_store,
            credentials_config: Arc::new(credentials_config),
            token_secret_manager,
            refresh_token_secret_manager,
            refresh_token_store,
            credentials_store,
        })
    }
}
