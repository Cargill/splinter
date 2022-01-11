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
/// * `GET /biome/users` - Get a list of all users in biome
/// * `PUT /biome/users/{id}` - Update user with specified ID
/// * `GET /biome/users/{id}` - Retrieve user with specified ID
/// * `DELETE /biome/users/{id}` - Remove user with specified ID
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
        vec![
            user::make_list_route(self.credentials_store.clone()),
            verify::make_verify_route(
                self.credentials_store.clone(),
                self.credentials_config.clone(),
                self.token_secret_manager.clone(),
            ),
            login::make_login_route(
                self.credentials_store.clone(),
                self.refresh_token_store.clone(),
                self.credentials_config.clone(),
                Arc::new(AccessTokenIssuer::new(
                    self.token_secret_manager.clone(),
                    self.refresh_token_secret_manager.clone(),
                )),
            ),
            token::make_token_route(
                self.refresh_token_store.clone(),
                self.token_secret_manager.clone(),
                self.refresh_token_secret_manager.clone(),
                Arc::new(AccessTokenIssuer::new(
                    self.token_secret_manager.clone(),
                    self.refresh_token_secret_manager.clone(),
                )),
                self.credentials_config.clone(),
            ),
            logout::make_logout_route(
                self.refresh_token_store.clone(),
                self.token_secret_manager.clone(),
                self.credentials_config.clone(),
            ),
            register::make_register_route(
                self.credentials_store.clone(),
                self.credentials_config.clone(),
            ),
            #[cfg(feature = "biome-key-management")]
            user::make_user_routes(
                self.credentials_config.clone(),
                self.credentials_store.clone(),
                self.key_store.clone(),
            ),
        ]
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::{panic, thread};

    use reqwest::blocking::Client;

    #[cfg(feature = "biome-key-management")]
    use crate::biome::MemoryKeyStore;
    use crate::biome::{MemoryCredentialsStore, MemoryRefreshTokenStore};
    #[cfg(feature = "authorization")]
    use crate::error::InternalError;
    use crate::rest_api::actix_web_1::{AuthConfig, RestApiBuilder, RestApiShutdownHandle};
    #[cfg(feature = "authorization")]
    use crate::rest_api::auth::{
        authorization::{AuthorizationHandler, AuthorizationHandlerResult},
        identity::Identity,
    };

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

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct RegistrationResponse {
        #[serde(rename = "message")]
        pub _message: String,
        pub data: RegistrationUser,
    }

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct LoginResponse {
        #[serde(rename = "message")]
        pub _message: String,
        pub user_id: String,
        pub token: String,
        pub refresh_token: String,
    }

    #[cfg(feature = "biome-key-management")]
    #[derive(Deserialize)]
    struct GetUserResponse {
        pub user_id: String,
        pub username: String,
    }

    #[cfg(feature = "biome-key-management")]
    #[derive(Serialize)]
    struct PutUser {
        pub username: String,
        pub hashed_password: String,
        pub new_password: Option<String>,
        pub new_key_pairs: Vec<PostKey>,
    }

    #[cfg(feature = "biome-key-management")]
    #[derive(Serialize)]
    struct PostKey {
        pub public_key: String,
        pub encrypted_private_key: String,
        pub display_name: String,
    }

    #[derive(Deserialize, Serialize)]
    struct PostVerify {
        username: String,
        hashed_password: String,
    }

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct PostVerifyResponse {
        #[serde(rename = "message")]
        pub _message: String,
        pub user_id: String,
    }

    #[derive(Deserialize, Serialize)]
    struct PostToken {
        token: String,
    }

    fn start_biome_rest_api() -> (RestApiShutdownHandle, thread::JoinHandle<()>) {
        let refresh_token_store = MemoryRefreshTokenStore::new();
        let cred_store = MemoryCredentialsStore::new();
        #[cfg(feature = "biome-key-management")]
        let key_store = MemoryKeyStore::new(cred_store.clone());
        let config = BiomeCredentialsRestConfigBuilder::default()
            .with_password_encryption_cost("low")
            .build()
            .unwrap();

        let mut biome_credentials_resource_provider_builder =
            BiomeCredentialsRestResourceProviderBuilder::default();

        biome_credentials_resource_provider_builder = biome_credentials_resource_provider_builder
            .with_refresh_token_store(refresh_token_store)
            .with_credentials_store(cred_store)
            .with_credentials_config(config);

        #[cfg(feature = "biome-key-management")]
        {
            biome_credentials_resource_provider_builder =
                biome_credentials_resource_provider_builder.with_key_store(key_store);
        }

        let biome_credentials_resource_provider =
            biome_credentials_resource_provider_builder.build().unwrap();

        let mut rest_api_builder = RestApiBuilder::new();

        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

        rest_api_builder =
            rest_api_builder
                .with_bind(bind)
                .with_auth_configs(vec![AuthConfig::Biome {
                    biome_credentials_resource_provider,
                }]);

        #[cfg(feature = "authorization")]
        {
            rest_api_builder = rest_api_builder
                .with_authorization_handlers(vec![Box::new(AlwaysAllowAuthorizationHandler)]);
        }

        rest_api_builder.build().unwrap().run().unwrap()
    }

    /// An authorization handler that always returns `Ok(AuthorizationHandlerResult::Allow)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysAllowAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysAllowAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Allow)
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
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
    #[cfg(feature = "biome-key-management")]
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
    #[cfg(feature = "biome-key-management")]
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
    #[cfg(feature = "biome-key-management")]
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
    /// accessible via GET /biome/users/{id}, which should return
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
    #[cfg(feature = "biome-key-management")]
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
