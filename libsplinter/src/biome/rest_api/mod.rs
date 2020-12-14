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

#[cfg(feature = "rest-api-actix")]
mod actix;
#[cfg(feature = "auth")]
pub(crate) mod auth;
mod config;
mod error;
mod resources;

use std::sync::Arc;

#[cfg(feature = "biome-credentials")]
use crate::biome::refresh_tokens::store::RefreshTokenStore;
#[cfg(all(feature = "auth", feature = "biome-credentials"))]
use crate::rest_api::{
    auth::identity::biome::BiomeUserIdentityProvider, sessions::default_validation,
};
use crate::rest_api::{Resource, RestResourceProvider};

#[cfg(all(feature = "biome-key-management", feature = "rest-api-actix",))]
use self::actix::key_management::{
    make_key_management_route, make_key_management_route_with_public_key,
};

#[cfg(feature = "biome-key-management")]
use super::key_management::store::KeyStore;

#[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
use crate::rest_api::secrets::AutoSecretManager;
use crate::rest_api::secrets::SecretManager;

pub use config::{BiomeRestConfig, BiomeRestConfigBuilder};
pub use error::BiomeRestResourceManagerBuilderError;

#[cfg(all(feature = "rest-api-actix", feature = "biome-credentials"))]
use self::actix::logout::make_logout_route;
#[cfg(all(feature = "biome-credentials", feature = "rest-api-actix"))]
use self::actix::register::make_register_route;
#[cfg(all(feature = "biome-credentials", feature = "rest-api-actix"))]
use self::actix::token::make_token_route;
#[cfg(all(
    feature = "biome-credentials",
    feature = "biome-key-management",
    feature = "rest-api-actix",
))]
use self::actix::user::make_user_routes;
#[cfg(all(feature = "biome-credentials", feature = "rest-api-actix",))]
use self::actix::{login::make_login_route, user::make_list_route, verify::make_verify_route};
#[cfg(all(feature = "auth", feature = "biome-credentials"))]
use self::auth::GetUserByBiomeAuthorization;
#[cfg(feature = "biome-credentials")]
use super::credentials::store::CredentialsStore;

#[allow(unused_imports)]
use crate::rest_api::sessions::AccessTokenIssuer;

/// Provides the REST API endpoints for biome
///
/// The following endponts are provided
///
/// * `GET /biome/keys` - Get all keys for authorized user
/// * `POST /biome/keys` - Create a new key for authorized user
/// * `PATCH /biome/keys` - Update the display name associated with a key for
///    an authorized user.
/// * `GET /biome/keys/{public_key}` - Retrieve a key for an authroized user that has
///    `public_key`
/// * `DELETE /biome/keys/{public_key}` - delete a  key for an authorized user that has
///    `public key`
/// * `POST /biome/login` - Login enpoint for getting access tokens and refresh tokens
/// * `PATCH /biome/logout` - Login endpoint for removing refresh tokens
/// * `POST /biome/register - Creates credentials for a user
/// * `POST /biome/token` - Creates a new access token for the authorized user
/// * `POST /biome/verify` - Verify a users password
/// * `POST /biome/users` - Create new user
/// * `GET /biome/user` - Get a list of all users in biome
/// * `PUT /biome/user/{id}` - Update user with specified ID
/// * `GET /biome/user/{id}` - Retrieve user with specified ID
/// * `DELETE /biome/user/{id}` - Remove user with specified ID
pub struct BiomeRestResourceManager {
    #[cfg(feature = "biome-key-management")]
    key_store: Arc<dyn KeyStore>,
    #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
    rest_config: Arc<BiomeRestConfig>,
    #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
    token_secret_manager: Arc<dyn SecretManager>,
    #[cfg(feature = "biome-credentials")]
    refresh_token_secret_manager: Arc<dyn SecretManager>,
    #[cfg(feature = "biome-credentials")]
    refresh_token_store: Arc<dyn RefreshTokenStore>,
    #[cfg(feature = "biome-credentials")]
    credentials_store: Arc<dyn CredentialsStore>,
}

impl BiomeRestResourceManager {
    /// Creates a new Biome user identity provider for the Splinter REST API
    #[cfg(all(feature = "auth", feature = "biome-credentials"))]
    pub fn get_identity_provider(&self) -> BiomeUserIdentityProvider {
        BiomeUserIdentityProvider::new(
            self.token_secret_manager.clone(),
            default_validation(&self.rest_config.issuer()),
        )
    }

    /// Creates a new Biome authorization mapping for Users
    #[cfg(all(feature = "auth", feature = "biome-credentials"))]
    pub fn get_authorization_mapping(&self) -> GetUserByBiomeAuthorization {
        GetUserByBiomeAuthorization::new(
            self.rest_config.clone(),
            self.token_secret_manager.clone(),
        )
    }
}

impl RestResourceProvider for BiomeRestResourceManager {
    fn resources(&self) -> Vec<Resource> {
        // This needs to be mutable if biome-credentials feature is enable
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        #[cfg(all(
            feature = "biome-credentials",
            feature = "biome-key-management",
            feature = "rest-api-actix",
        ))]
        {
            resources.push(make_user_routes(
                self.rest_config.clone(),
                self.token_secret_manager.clone(),
                self.credentials_store.clone(),
                self.key_store.clone(),
            ));
        }

        #[cfg(all(feature = "biome-credentials", feature = "rest-api-actix",))]
        {
            resources.push(make_list_route(self.credentials_store.clone()));
            resources.push(make_verify_route(
                self.credentials_store.clone(),
                self.rest_config.clone(),
                self.token_secret_manager.clone(),
            ));
            resources.push(make_login_route(
                self.credentials_store.clone(),
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
            resources.push(make_logout_route(
                self.refresh_token_store.clone(),
                self.token_secret_manager.clone(),
                self.rest_config.clone(),
            ));

            resources.push(make_register_route(
                self.credentials_store.clone(),
                self.rest_config.clone(),
            ));
        }

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
#[derive(Default)]
pub struct BiomeRestResourceManagerBuilder {
    #[cfg(feature = "biome-key-management")]
    key_store: Option<Arc<dyn KeyStore>>,
    rest_config: Option<BiomeRestConfig>,
    token_secret_manager: Option<Arc<dyn SecretManager>>,
    #[cfg(feature = "biome-credentials")]
    refresh_token_secret_manager: Option<Arc<dyn SecretManager>>,
    #[cfg(feature = "biome-credentials")]
    refresh_token_store: Option<Arc<dyn RefreshTokenStore>>,
    #[cfg(feature = "biome-credentials")]
    credentials_store: Option<Arc<dyn CredentialsStore>>,
}

impl BiomeRestResourceManagerBuilder {
    /// Sets a KeyStore for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `pool`: ConnectionPool to database that will serve as backend for KeyStore
    #[cfg(feature = "biome-key-management")]
    pub fn with_key_store(
        mut self,
        store: impl KeyStore + 'static,
    ) -> BiomeRestResourceManagerBuilder {
        self.key_store = Some(Arc::new(store));
        self
    }

    /// Sets a BiomeRestConfig for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `config`: the BiomeRestConfig that will be used to configure the Biome resources
    pub fn with_rest_config(mut self, config: BiomeRestConfig) -> BiomeRestResourceManagerBuilder {
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
        store: impl CredentialsStore + 'static,
    ) -> BiomeRestResourceManagerBuilder {
        self.credentials_store = Some(Arc::new(store));
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
    ) -> BiomeRestResourceManagerBuilder {
        self.token_secret_manager = Some(Arc::new(secret_manager));
        self
    }

    /// Sets a SecretManager for the refresh tokens for the BiomeRestResourceManager
    ///
    /// # Arguments
    ///
    /// * `secret_manager`: the SecretManager to be used for fetching and generating secrets to
    ///   sign and verify JWT tokens
    #[cfg(feature = "biome-credentials")]
    pub fn with_refresh_token_secret_manager(
        mut self,
        secret_manager: impl SecretManager + 'static,
    ) -> BiomeRestResourceManagerBuilder {
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
    #[cfg(feature = "biome-credentials")]
    pub fn with_refresh_token_store(
        mut self,
        store: impl RefreshTokenStore + 'static,
    ) -> BiomeRestResourceManagerBuilder {
        self.refresh_token_store = Some(Arc::new(store));
        self
    }

    /// Consumes the builder and returns a BiomeRestResourceManager
    pub fn build(self) -> Result<BiomeRestResourceManager, BiomeRestResourceManagerBuilderError> {
        #[cfg(feature = "biome-key-management")]
        let key_store = self.key_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing key store".to_string(),
            )
        })?;
        #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
        let rest_config = match self.rest_config {
            Some(config) => config,
            None => {
                debug!("Building BiomeRestResourceManager with default config.");
                BiomeRestConfigBuilder::default().build()?
            }
        };

        #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
        let token_secret_manager = self.token_secret_manager.unwrap_or_else(|| {
            debug!("Building BiomeRestResourceManager with default SecretManager.");
            Arc::new(AutoSecretManager::default())
        });

        #[cfg(feature = "biome-credentials")]
        let refresh_token_secret_manager = self.refresh_token_secret_manager.unwrap_or_else(|| {
            debug!("Building BiomeRestResourceManager with default token SecretManager.");
            Arc::new(AutoSecretManager::default())
        });

        #[cfg(feature = "biome-credentials")]
        let refresh_token_store = self.refresh_token_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing refresh token store".to_string(),
            )
        })?;

        #[cfg(feature = "biome-credentials")]
        #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
        let credentials_store = self.credentials_store.ok_or_else(|| {
            BiomeRestResourceManagerBuilderError::MissingRequiredField(
                "Missing credentials store".to_string(),
            )
        })?;

        Ok(BiomeRestResourceManager {
            #[cfg(feature = "biome-key-management")]
            key_store,
            #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
            rest_config: Arc::new(rest_config),
            #[cfg(any(feature = "biome-key-management", feature = "biome-credentials",))]
            token_secret_manager,
            #[cfg(feature = "biome-credentials")]
            refresh_token_secret_manager,
            #[cfg(feature = "biome-credentials")]
            refresh_token_store,
            #[cfg(feature = "biome-credentials")]
            credentials_store,
        })
    }
}

#[cfg(test)]
#[cfg(all(feature = "biome-key-management", feature = "biome-credentials"))]
mod tests {
    use super::*;

    use std::{panic, thread};

    use reqwest::blocking::Client;

    use crate::biome::{MemoryCredentialsStore, MemoryKeyStore, MemoryRefreshTokenStore};
    #[cfg(feature = "auth")]
    use crate::rest_api::AuthConfig;
    use crate::rest_api::{RestApiBuilder, RestApiShutdownHandle};

    #[derive(Serialize)]
    struct UsernamePassword {
        pub username: String,
        pub hashed_password: String,
    }

    #[derive(Deserialize, Serialize)]
    struct RegistrationUser {
        pub user_id: String,
        pub username: String,
    }

    #[derive(Deserialize)]
    struct RegistrationResponse {
        pub message: String,
        pub data: RegistrationUser,
    }

    #[derive(Deserialize)]
    struct LoginResponse {
        pub message: String,
        pub user_id: String,
        pub token: String,
        pub refresh_token: String,
    }

    #[derive(Deserialize)]
    struct GetUserResponse {
        pub user_id: String,
        pub username: String,
    }

    #[derive(Serialize)]
    struct PutUser {
        pub username: String,
        pub hashed_password: String,
        pub new_password: Option<String>,
        pub new_key_pairs: Vec<PostKey>,
    }

    #[derive(Serialize)]
    struct PostKey {
        pub public_key: String,
        pub encrypted_private_key: String,
        pub display_name: String,
    }

    #[derive(Deserialize)]
    struct Key {
        pub public_key: String,
        pub user_id: String,
        pub display_name: String,
        pub encrypted_private_key: String,
    }

    #[derive(Deserialize)]
    struct PostKeyResponse {
        pub message: String,
        pub data: Key,
    }

    #[derive(Deserialize)]
    struct GetKeyResponse {
        pub data: Key,
    }

    #[derive(Deserialize)]
    struct GetKeysResponse {
        pub data: Vec<Key>,
    }

    #[derive(Deserialize, Serialize)]
    struct PatchKey {
        pub public_key: String,
        pub new_display_name: String,
    }

    #[derive(Deserialize, Serialize)]
    struct PostVerify {
        username: String,
        hashed_password: String,
    }

    #[derive(Deserialize)]
    struct PostVerifyResponse {
        pub message: String,
        pub user_id: String,
    }

    #[derive(Deserialize, Serialize)]
    struct PostToken {
        token: String,
    }

    fn start_biome_rest_api() -> (RestApiShutdownHandle, thread::JoinHandle<()>) {
        let refresh_token_store = MemoryRefreshTokenStore::new();
        let cred_store = MemoryCredentialsStore::new();
        let key_store = MemoryKeyStore::new(cred_store.clone());
        let config = BiomeRestConfigBuilder::default()
            .with_password_encryption_cost("low")
            .build()
            .unwrap();

        let resource_manager = BiomeRestResourceManagerBuilder::default()
            .with_refresh_token_store(refresh_token_store)
            .with_credentials_store(cred_store)
            .with_key_store(key_store)
            .with_rest_config(config)
            .build()
            .unwrap();

        let mut rest_api_builder = RestApiBuilder::new();

        rest_api_builder = rest_api_builder
            .with_bind("127.0.0.1:0")
            .add_resources(resource_manager.resources());

        #[cfg(feature = "auth")]
        {
            rest_api_builder = rest_api_builder
                .with_authorization_mapping(resource_manager.get_authorization_mapping());

            rest_api_builder = rest_api_builder.with_auth_configs(vec![AuthConfig::Biome {
                biome_resource_manager: resource_manager,
            }]);
        }

        rest_api_builder.build().unwrap().run().unwrap()
    }

    fn create_and_authorize_user(
        url: &str,
        client: &Client,
        username: &str,
        password: &str,
    ) -> LoginResponse {
        let registration_response = client
            .post(&format!("{}/biome/register", url))
            .json(&UsernamePassword {
                username: username.to_string(),
                hashed_password: password.to_string(),
            })
            .send()
            .unwrap();
        assert!(registration_response.status().is_success());

        let login_response = client
            .post(&format!("{}/biome/login", url))
            .json(&UsernamePassword {
                username: username.to_string(),
                hashed_password: password.to_string(),
            })
            .send()
            .unwrap();
        assert!(login_response.status().is_success());

        login_response.json::<LoginResponse>().unwrap()
    }

    fn run_test<F>(f: F)
    where
        F: FnOnce(&str, Client) -> () + panic::UnwindSafe,
    {
        let (handle, join_handle) = start_biome_rest_api();

        let port_no = handle.port_numbers()[0];

        let result = panic::catch_unwind(move || {
            let client = Client::new();
            f(&format!("http://127.0.0.1:{}", port_no), client)
        });

        handle.shutdown().unwrap();

        join_handle.join().unwrap();

        assert!(result.is_ok());
    }

    /// Happy path test for POST /biome/register
    ///
    /// Verify that POST /biome/register creates a user
    /// and returns a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Register user via POST /biome/register
    /// 2) Verify that the request was successful
    /// 3) Verify that the username of the user created matches
    #[test]
    fn test_register() {
        run_test(|url, client| {
            let response = client
                .post(&format!("{}/biome/register", url))
                .json(&UsernamePassword {
                    username: "test_register@gmail.com".to_string(),
                    hashed_password: "Admin2193!".to_string(),
                })
                .send()
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);

            let new_user = response.json::<RegistrationResponse>().unwrap();

            assert_eq!("test_register@gmail.com", new_user.data.username);
        })
    }

    /// Happy path test for POST /biome/login
    ///
    /// Verify that POST /biome/login authorizes a user and returns
    /// a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a user
    /// 2) Attempt login as the created user
    /// 3) Verify that the request was successful
    #[test]
    fn test_login() {
        run_test(|url, client| {
            let registration_response = client
                .post(&format!("{}/biome/register", url))
                .json(&UsernamePassword {
                    username: "test_login@gmail.com".to_string(),
                    hashed_password: "Admin2193!".to_string(),
                })
                .send()
                .unwrap();
            assert_eq!(registration_response.status().as_u16(), 200);

            let login_response = client
                .post(&format!("{}/biome/login", url))
                .json(&UsernamePassword {
                    username: "test_login@gmail.com".to_string(),
                    hashed_password: "Admin2193!".to_string(),
                })
                .send()
                .unwrap();
            assert_eq!(login_response.status().as_u16(), 200);
        })
    }

    /// Happy path test for GET /biome/users/{id}
    ///
    /// Verify that GET /biome/users/{id} returns the correct user
    /// and a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create user and login as that user
    /// 2) Query for that user via GET /biome/users/{id}
    /// 3) Verify that the user_id used to query for the user and
    ///    the username given to the user match the user information
    ///    that is returned
    #[test]
    fn test_get_user() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_get_user@gmail.com", "Admin2193!");

            let user_response = client
                .get(&format!("{}/biome/users/{}", url, login.user_id))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(user_response.status().as_u16(), 200);

            let user = user_response.json::<GetUserResponse>().unwrap();

            assert_eq!(login.user_id, user.user_id);
            assert_eq!("test_get_user@gmail.com", user.username);
        })
    }

    /// Happy path test for GET /biome/users
    ///
    /// Verify that GET /biome/users returns a list of
    /// users, and a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create user and login as that user
    /// 2) Query for all users via GET /biome/users
    /// 3) Verify that the user_id and username of the created
    ///    user matches one of the users returned
    #[test]
    fn test_get_users() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_get_users@gmail.com", "Admin2193!");

            let users_response = client
                .get(&format!("{}/biome/users", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(users_response.status().as_u16(), 200);

            let users = users_response.json::<Vec<GetUserResponse>>().unwrap();

            assert!(users.iter().any(|user| {
                login.user_id == user.user_id && "test_get_users@gmail.com" == user.username
            }))
        })
    }

    /// Happy path test for PUT /biome/users/{id}
    ///
    /// Verify that PUT /biome/users/{id} correctly updates the user
    /// resource specified by {id}, and returns a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create user and login as that user
    /// 2) Change the created user's password via PUT /biome/users/{id}
    /// 3) Attempt to login as the user to verify the password was changed
    #[test]
    fn test_put_user() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_put_user@gmail.com", "Admin2193!");

            let put_user_response = client
                .put(&format!("{}/biome/users/{}", url, login.user_id))
                .header("Authorization", format!("Bearer {}", login.token))
                .json(&PutUser {
                    username: "test_put_user@gmail.com".to_string(),
                    hashed_password: "Admin2193!".to_string(),
                    new_password: Some("new_password2193!".to_string()),
                    new_key_pairs: Vec::new(),
                })
                .send()
                .unwrap();

            assert_eq!(put_user_response.status().as_u16(), 200);

            let login_response = client
                .post(&format!("{}/biome/login", url))
                .json(&UsernamePassword {
                    username: "test_put_user@gmail.com".to_string(),
                    hashed_password: "new_password2193!".to_string(),
                })
                .send()
                .unwrap();

            assert_eq!(login_response.status().as_u16(), 200);
        })
    }

    /// Happy path test for DELETE /biome/users/{id}
    ///
    /// Verify DELETE /biome/users/{id} removes the user
    /// specified by {id}. This means the user is no longer
    /// accessible via GET /biome/user/{id}, which should return
    /// a status code of 404.
    ///
    /// Procedure
    ///
    /// 1) Create user and login as that user
    /// 2) Create a second user
    /// 3) Verify that the second user exists
    /// 4) Delete the second user via DELETE /biome/users/{id}
    /// 5) Verify that the user was deleted using GET /biome/users/{id}
    #[test]
    fn test_delete_user() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_delete_user@gmail.com", "Admin2193!");

            let user_to_delete = create_and_authorize_user(
                url,
                &client,
                "test_delete_user_2@gmail.com",
                "Admin2193!",
            );

            let get_user_response = client
                .get(&format!("{}/biome/users/{}", url, user_to_delete.user_id))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(get_user_response.status().as_u16(), 200);

            assert_eq!(
                client
                    .delete(&format!("{}/biome/users/{}", url, user_to_delete.user_id))
                    .header("Authorization", format!("Bearer {}", user_to_delete.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .get(&format!("{}/biome/users/{}", url, user_to_delete.user_id))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                404
            );
        })
    }

    /// Test happy path for POST /biome/keys
    ///
    /// Verify that POST /biome/keys creates a new key resource, and
    /// returns a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Verify the public_key, encrypted_private_key, and display_name
    ///    returned are correct
    #[test]
    fn test_post_key() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_post_key@gmail.com", "Admin2193!");

            let expected_key = PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "test_post_key@gmail.com".to_string(),
            };

            let key = client
                .post(&format!("{}/biome/keys", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .json(&expected_key)
                .send()
                .unwrap()
                .json::<PostKeyResponse>()
                .unwrap();

            assert_eq!(expected_key.public_key, key.data.public_key);
            assert_eq!(
                expected_key.encrypted_private_key,
                key.data.encrypted_private_key
            );
            assert_eq!(expected_key.display_name, key.data.display_name);
        })
    }

    /// Test happy path for GET /biome/keys/{public_key}
    ///
    /// Verify GET /biome/keys/{public_key} retrieves the
    /// correct keys resource, and returns a status code
    /// of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Verify that key exists via GET /biome/keys/{public_key}
    #[test]
    fn test_get_keys_pub_key() {
        run_test(|url, client| {
            let login = create_and_authorize_user(
                url,
                &client,
                "test_get_keys_pub_key@gmail.com",
                "Admin2193!",
            );

            let expected_key = PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "test_get_keys_pub@gmail.com".to_string(),
            };

            let created_key_response = client
                .post(&format!("{}/biome/keys", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .json(&expected_key)
                .send()
                .unwrap();

            assert_eq!(created_key_response.status().as_u16(), 200);

            let created_key = created_key_response.json::<PostKeyResponse>().unwrap();

            let get_key_response = client
                .get(&format!(
                    "{}/biome/keys/{}",
                    url, created_key.data.public_key
                ))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(get_key_response.status().as_u16(), 200);

            let actual_key = get_key_response.json::<GetKeyResponse>().unwrap();

            assert_eq!(expected_key.public_key, actual_key.data.public_key);
            assert_eq!(
                expected_key.encrypted_private_key,
                actual_key.data.encrypted_private_key
            );
            assert_eq!(expected_key.display_name, actual_key.data.display_name);
        })
    }

    /// Test happy path for GET /biome/keys
    ///
    /// Verify that GET /biome/keys retrieves a list of keys
    /// and a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Retrieve a list of user keys via GET /biome/keys
    /// 4) Verify that the created key exists in the list of user keys
    #[test]
    fn test_get_keys() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_get_keys@gmail.com", "Admin2193!");

            let expected_key = PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "test_get_keys_pub@gmail.com".to_string(),
            };

            assert_eq!(
                client
                    .post(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&expected_key)
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            let get_keys_response = client
                .get(&format!("{}/biome/keys", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(get_keys_response.status().as_u16(), 200);

            let actual_keys = get_keys_response.json::<GetKeysResponse>().unwrap();

            assert!(actual_keys.data.iter().any(|key| {
                expected_key.public_key == key.public_key
                    && expected_key.encrypted_private_key == key.encrypted_private_key
                    && expected_key.display_name == key.display_name
            }));
        })
    }

    /// Test happy path for PATCH /biome/keys
    ///
    /// Verify PATCH /biome/keys updates the keys owned by
    /// the authorized user, and returns a status of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Update public_key via PATCH /biome/keys
    /// 4) Retrieve key via GET /biome/keys/{public_key}
    /// 5) Verify the key has been updated
    #[test]
    fn test_patch_keys() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_patch_keys@gmail.com", "Admin2193!");

            assert_eq!(
                client
                    .post(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&PostKey {
                        public_key: "<public_key>".to_string(),
                        encrypted_private_key: "<private_key>".to_string(),
                        display_name: "test_patch_keys@gmail.com".to_string(),
                    })
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .patch(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&PatchKey {
                        public_key: "<public_key>".to_string(),
                        new_display_name: "new_test_patch_keys@gmail.com".to_string(),
                    })
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            let expected_key = PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "new_test_patch_keys@gmail.com".to_string(),
            };

            let get_key_response = client
                .get(&format!("{}/biome/keys/{}", url, expected_key.public_key))
                .header("Authorization", format!("Bearer {}", login.token))
                .send()
                .unwrap();

            assert_eq!(get_key_response.status().as_u16(), 200);

            let actual_key = get_key_response.json::<GetKeyResponse>().unwrap();

            assert_eq!(expected_key.public_key, actual_key.data.public_key);
            assert_eq!(
                expected_key.encrypted_private_key,
                actual_key.data.encrypted_private_key
            );
            assert_eq!(expected_key.display_name, actual_key.data.display_name);
        })
    }

    /// Happy path test for `DELETE /biome/keys/{public_key}`
    ///
    /// Verify that DELETE /biome/keys/{public_key} removes the keys
    /// resource specified by {public_key}. This means that the resource
    /// is no longer available via GET /biome/keys/{public_keys} which
    /// returns a 404.
    ///
    /// Procedure
    ///
    /// 1) Create a user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Verify that the created key exists via GET /biome/keys/{public_key}
    /// 3) Delete public_key via DELETE /biome/keys/{public_key}
    /// 4) Attempt to retrieve the key via GET /biome/keys/{public_key}
    /// 5) Verify the key has been deleted
    #[test]
    fn test_delete_key() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_delete_key@gmail.com", "Admin2193!");

            let new_key = PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "test_delete_key@gmail.com".to_string(),
            };

            assert_eq!(
                client
                    .post(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&new_key)
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .get(&format!("{}/biome/keys/{}", url, new_key.public_key))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .delete(&format!("{}/biome/keys/{}", url, new_key.public_key))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .get(&format!("{}/biome/keys/{}", url, new_key.public_key))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                404
            );
        });
    }

    /// Test happy path for PATCH /biome/logout
    ///
    /// Verify PATCH /biome/logout deletes refresh token
    /// held by authorized user, and verify that
    /// POST /biome/token returns a 403 afterward.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) logout as that user via PATCH /biome/logout
    /// 3) Verify correct status code
    /// 4) Verify token has been deleted via POST /biome/token
    #[test]
    fn test_logout() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_logout@gmail.com", "Admin2193!");

            assert_eq!(
                client
                    .patch(&format!("{}/biome/logout", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            assert_eq!(
                client
                    .post(&format!("{}/biome/token", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&PostToken {
                        token: login.refresh_token
                    })
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                403
            );
        });
    }

    /// Test Happy path for POST /biome/verify
    ///
    /// Verify that POST /biome/verify returns a status code
    /// of 200 when submitting a valid user ID.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Verify the user's password via POST /biome/verify
    /// 3) Verify the correct status code and user id was
    ///    returned
    #[test]
    fn test_password_verify() {
        run_test(|url, client| {
            let login = create_and_authorize_user(
                url,
                &client,
                "test_password_verify@gmail.com",
                "Admin2193!",
            );

            let verify_response = client
                .post(&format!("{}/biome/verify", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .json(&PostVerify {
                    username: "test_password_verify@gmail.com".to_string(),
                    hashed_password: "Admin2193!".to_string(),
                })
                .send()
                .unwrap();

            assert_eq!(verify_response.status().as_u16(), 200);

            let verify_user_id = verify_response
                .json::<PostVerifyResponse>()
                .unwrap()
                .user_id;

            assert_eq!(login.user_id, verify_user_id);
        });
    }

    /// Test Happy path for POST /biome/token
    ///
    /// Verify that POST /biome/token returns a new
    /// access token and a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Retrieve a new access token via POST /biome/token
    /// 3) Verify correct payload was returned
    #[test]
    fn test_post_token() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_post_token@gmail.com", "Admin2193!");

            let token_response = client
                .post(&format!("{}/biome/token", url))
                .header("Authorization", format!("Bearer {}", login.token))
                .json(&PostToken {
                    token: login.refresh_token,
                })
                .send()
                .unwrap();

            assert_eq!(token_response.status().as_u16(), 200);

            token_response.json::<PostToken>().unwrap();
        });
    }
}
