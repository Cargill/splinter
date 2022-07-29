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

mod endpoints;
mod resources;

use std::sync::Arc;

use splinter::biome::key_management::store::KeyStore;

use crate::framework::{Resource, RestResourceProvider};

/// Provides the following REST API endpoints for Biome key management:
///
/// * `GET /biome/keys` - Get all keys for the authorized user
/// * `POST /biome/keys` - Add a new key for the authorized user
/// * `PUT /biome/keys` - Replace keys for the authorized user
/// * `PATCH /biome/keys` - Update the display name associated with a key for the authorized user
/// * `GET /biome/keys/{public_key}` - Retrieve the authorized user's key that corresponds to
///   `public_key`
/// * `DELETE /biome/keys/{public_key}` - Delete the authorized user's key that corresponds to
///   `public key`
pub struct BiomeKeyManagementRestResourceProvider {
    key_store: Arc<dyn KeyStore>,
}

impl BiomeKeyManagementRestResourceProvider {
    pub fn new(key_store: Arc<dyn KeyStore>) -> Self {
        Self { key_store }
    }
}

impl RestResourceProvider for BiomeKeyManagementRestResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        vec![
            endpoints::make_key_management_route(self.key_store.clone()),
            endpoints::make_key_management_route_with_public_key(self.key_store.clone()),
        ]
    }
}

#[cfg(feature = "biome-credentials")]
#[cfg(test)]
mod tests {
    use super::*;

    use std::{panic, thread};

    use reqwest::blocking::Client;

    use splinter::biome::{MemoryCredentialsStore, MemoryKeyStore, MemoryRefreshTokenStore};
    #[cfg(feature = "authorization")]
    use splinter::error::InternalError;
    #[cfg(feature = "authorization")]
    use splinter_rest_api_common::auth::{
        AuthorizationHandler, AuthorizationHandlerResult, Identity,
    };

    use crate::biome::credentials::{
        BiomeCredentialsRestConfigBuilder, BiomeCredentialsRestResourceProviderBuilder,
    };
    use crate::framework::{AuthConfig, RestApiBuilder, RestApiShutdownHandle};

    #[derive(Serialize)]
    struct UsernamePassword {
        pub username: String,
        pub hashed_password: String,
    }

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct LoginResponse {
        #[serde(rename = "message")]
        pub _message: String,
        #[serde(rename = "user_id")]
        pub _user_id: String,
        pub token: String,
        #[serde(rename = "refresh_token")]
        pub _refresh_token: String,
    }

    #[derive(Serialize, Debug, PartialEq, Eq)]
    struct PostKey {
        pub public_key: String,
        pub encrypted_private_key: String,
        pub display_name: String,
    }

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct Key {
        pub public_key: String,
        #[serde(rename = "user_id")]
        pub _user_id: String,
        pub display_name: String,
        pub encrypted_private_key: String,
    }

    // ignored fields test that the server provides the field, but its not important to test the
    // contents
    #[derive(Deserialize)]
    struct PostKeyResponse {
        #[serde(rename = "message")]
        pub _message: String,
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

    fn start_biome_rest_api() -> (RestApiShutdownHandle, thread::JoinHandle<()>) {
        let refresh_token_store = MemoryRefreshTokenStore::new();
        let cred_store = MemoryCredentialsStore::new();
        let key_store = MemoryKeyStore::new(cred_store.clone());
        let config = BiomeCredentialsRestConfigBuilder::default()
            .with_password_encryption_cost("low")
            .build()
            .unwrap();

        let biome_credentials_resource_provider =
            BiomeCredentialsRestResourceProviderBuilder::default()
                .with_refresh_token_store(refresh_token_store)
                .with_credentials_store(cred_store)
                .with_credentials_config(config)
                .with_key_store(key_store.clone())
                .build()
                .unwrap();

        let biome_key_management_resource_provider =
            BiomeKeyManagementRestResourceProvider::new(Arc::new(key_store));

        let mut rest_api_builder = RestApiBuilder::new();

        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());

        rest_api_builder = rest_api_builder
            .with_bind(bind)
            .with_auth_configs(vec![AuthConfig::Biome {
                biome_credentials_resource_provider,
            }])
            .add_resources(biome_key_management_resource_provider.resources());

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

    /// Test happy path for PUT /biome/keys
    ///
    /// Verify that PUT /biome/keys replaces all key resources, and
    /// returns a status code of 200.
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Create a new key via POST /biome/keys
    /// 3) Verify that added key exists
    /// 4) Replace old key with new keys via PUT /biome/keys
    /// 5) Verify new keys are the only keys via GET /biome/keys
    #[test]
    fn test_put_key() {
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

            // The keys we are posting and using as a basis of comparison later
            // These keys must be in ascending order by public_key
            let expected_keys: Vec<PostKey> = vec![
                PostKey {
                    public_key: "<public_key2>".to_string(),
                    encrypted_private_key: "<private_key2>".to_string(),
                    display_name: "test_post_key2@gmail.com".to_string(),
                },
                PostKey {
                    public_key: "<public_key3>".to_string(),
                    encrypted_private_key: "<private_key3>".to_string(),
                    display_name: "test_post_key3@gmail.com".to_string(),
                },
                PostKey {
                    public_key: "<public_key4>".to_string(),
                    encrypted_private_key: "<private_key4>".to_string(),
                    display_name: "test_post_key4@gmail.com".to_string(),
                },
            ];

            assert_eq!(
                client
                    .put(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .json(&expected_keys)
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

            // Coerce keys into a comparable object
            let mut actual_keys = get_keys_response
                .json::<GetKeysResponse>()
                .unwrap()
                .data
                .into_iter()
                .map(|key| PostKey {
                    public_key: key.public_key,
                    encrypted_private_key: key.encrypted_private_key,
                    display_name: key.display_name,
                })
                .collect::<Vec<PostKey>>();

            actual_keys.sort_by(|a, b| a.public_key.partial_cmp(&b.public_key).unwrap());

            // Ensure all keys match up exactly
            assert_eq!(actual_keys, expected_keys);
        })
    }

    /// Test PUT to /biome/keys version number
    ///
    /// Verify that PUT /biome/keys correctly identifies and rejects
    /// invalid version numbers
    ///
    /// Procedure
    ///
    /// 1) Create a new user and log in as that user
    /// 2) Validate that PUT /biome/keys with a correct version returns 200
    /// 3) Validate that PUT /biome/keys with an incorrect returns 400
    #[test]
    fn test_put_key_version() {
        run_test(|url, client| {
            let login =
                create_and_authorize_user(url, &client, "test_post_key@gmail.com", "Admin2193!");

            let test_keys: Vec<PostKey> = vec![PostKey {
                public_key: "<public_key>".to_string(),
                encrypted_private_key: "<private_key>".to_string(),
                display_name: "test_post_key@gmail.com".to_string(),
            }];

            // Verify that a correct version works
            assert_eq!(
                client
                    .put(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .header("SplinterProtocolVersion", "2")
                    .json(&test_keys)
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                200
            );

            // Verify that an incorrect version does not work
            assert_eq!(
                client
                    .put(&format!("{}/biome/keys", url))
                    .header("Authorization", format!("Bearer {}", login.token))
                    .header("SplinterProtocolVersion", "1")
                    .json(&test_keys)
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                400
            );
        });
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
}
