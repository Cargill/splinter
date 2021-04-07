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

//! Integration tests using a `Biome` client to validate `Biome`'s management of users,
//! credentials, and keys.

use splinter::biome::client::{Authorization, Credentials, Key, NewKey, UpdateUser};
use splinterd::node::RestApiVariant;

use crate::framework::network::Network;

#[test]
/// This test validates `Biome`'s credentials-related endpoints. The `BiomeClient` is used to
/// register a new user and then initiate an active session for the user. This test validates that
/// `Biome` handles valid and invalid access tokens and credentials as expected.
///
/// 1. Start a single-node network and retrieve the node
/// 2. Register and login a Biome user, verify this returns successfully and store the access token
///    in the response to make calls to authorized `Biome` endpoints
/// 3. Submit the user's credentials to the `verify` endpoint, to verify the access token
///    returned in the previous step and the user's credentials are valid
/// 4. Update the user's password, validate this returns successfully
/// 5. Attempt to verify the user using the outdated password, validate this returns an error
/// 6. Verify the user using the updated password, validate this returns successfully
/// 7. Get a new access token for the user, using the `refresh_token` supplied when the user
///    logged in then validate the new access token is returned successfully
/// 8. Attempt to use the new access token to access a protected `Biome` resource, validate the
///    new access token is validated successfully
/// 9. Logout the `Biome` user, validate this returns successfully
/// 10. Attempt to get a new access token, validate this returns an error as the access token
///     being used is no longer valid
/// 11. Shutdown the network
fn test_biome_credentials() {
    // Start a single-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(1)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the node in the network
    let node = network.node(0).expect("Unable to get node");
    // Register a Biome user.
    assert!(node.biome_client(None).register("user", "password").is_ok());
    // Login to Biome to retrieve the user's authorization details.
    let user_auth: Authorization = node
        .biome_client(None)
        .login("user", "password")
        .expect("Unable to login Biome user");
    let auth_token = format!("Bearer {}", &user_auth.token);

    assert!(node
        .biome_client(Some(&auth_token))
        .verify("user", "password")
        .is_ok());

    // Create the struct used to update the Biome user's password.
    let update_user = UpdateUser {
        username: "user".to_string(),
        hashed_password: "password".to_string(),
        new_password: Some("new_password".to_string()),
        new_key_pairs: vec![],
    };
    let update_result = node
        .biome_client(Some(&auth_token))
        .update_user(&user_auth.user_id, update_user);
    assert!(update_result.is_ok());

    assert!(node
        .biome_client(Some(&auth_token))
        .verify("user", "password")
        .is_err());
    assert!(node
        .biome_client(Some(&auth_token))
        .verify("user", "new_password")
        .is_ok());

    let new_access_token_res = node
        .biome_client(Some(&auth_token))
        .get_new_access_token(&user_auth.refresh_token);
    assert!(new_access_token_res.is_ok());
    let new_token = format!("Bearer {}", new_access_token_res.unwrap());

    assert!(node
        .biome_client(Some(&new_token))
        .verify("user", "new_password")
        .is_ok());

    assert!(node.biome_client(Some(&new_token)).logout().is_ok());

    // Validate we are not able to retrieve a new access token from `Biome` after logging out,
    // invalidating the user's access tokens
    assert!(node
        .biome_client(Some(&new_token))
        .get_new_access_token(&user_auth.refresh_token)
        .is_err());

    shutdown!(network).expect("Unable to shutdown network");
}

#[test]
/// This test validates `Biome`'s key management operations available to users.
///
/// 1. Start a single-node network and retrieve the node
/// 2. Register and login a Biome user, verify this returns successfully and store the access token
///    in the response to make calls to authorized `Biome` endpoints
/// 3. Submit a new key pair to the node, using the Biome client's `add_key` method and verify this
///    returns successfully
/// 4. List the authorized user's key pairs, verify this returns 1 key pair successfully
/// 5. Submit a request to update the display name of the key pair just added, verify this returns
///    successfully
/// 6. Get the updated key, using `get_key`, then validate the key data is updated as expected
/// 7. Submit another new key pair to the node, using the Biome client's `add_key` method and
///    verify this returns successfully
/// 8. List the authorized user's key pairs, verify this returns 2 key pair successfully
/// 9. Delete one of the user's key pairs, verify this returns successfully
/// 10. Attempt to get the key deleted in the previous step, verify this successfully returns
///     a `None` value as the key pair no longer exists
/// 11. Shutdown the network
fn test_biome_key_management() {
    // Start a single-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(1)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the node in the network
    let node = network.node(0).expect("Unable to get node");
    // Register a Biome user.
    assert!(node
        .biome_client(None)
        .register("key_user", "password")
        .is_ok());
    // Login to Biome to retrieve the user's authorization details.
    let user_auth: Authorization = node
        .biome_client(None)
        .login("key_user", "password")
        .expect("Unable to login Biome user");
    let auth_token = format!("Bearer {}", &user_auth.token);

    // Create the new key to be added for the Biome user
    let new_key = NewKey {
        public_key: "01234".to_string(),
        encrypted_private_key: "56789".to_string(),
        display_name: "first_key".to_string(),
    };
    assert!(node
        .biome_client(Some(&auth_token))
        .add_key(&user_auth.user_id, new_key)
        .is_ok());

    // List the user's keys, using the biome user's access token returned at log-in
    let keys = node
        .biome_client(Some(&auth_token))
        .list_user_keys()
        .expect("Unable to list user keys")
        .collect::<Vec<Key>>();
    // Validate only the single key is returned
    assert_eq!(keys.len(), 1);

    // Create the new key to be added for the Biome user
    let new_key = NewKey {
        public_key: "01234".to_string(),
        encrypted_private_key: "56789".to_string(),
        display_name: "first_key_updated".to_string(),
    };
    // Create the struct used to update the Biome user's password.
    let update_user = UpdateUser {
        username: "key_user".to_string(),
        hashed_password: "password".to_string(),
        new_password: None,
        new_key_pairs: vec![new_key],
    };
    // Update the biome user, using the biome user's access token returned at log-in
    let update_result = node
        .biome_client(Some(&auth_token))
        .update_user(&user_auth.user_id, update_user);
    assert!(update_result.is_ok());
    // Attempt to get the key, using the biome user's access token returned at log-in
    if let Some(key) = node
        .biome_client(Some(&auth_token))
        .get_key("01234")
        .expect("Unable to get key from Biome")
    {
        assert_eq!(&key.display_name, "first_key_updated");
        assert_eq!(&key.user_id, &user_auth.user_id);
    } else {
        panic!("Unable to retrieve updated key");
    }

    // Create the new key to be added for the Biome user
    let new_key = NewKey {
        public_key: "43210".to_string(),
        encrypted_private_key: "98765".to_string(),
        display_name: "second_key".to_string(),
    };
    // Attempt to add a key, using the biome user's access token returned at log-in
    assert!(node
        .biome_client(Some(&auth_token))
        .add_key(&user_auth.user_id, new_key)
        .is_ok());

    // List the user's keys, using the biome user's access token returned at log-in
    let keys = node
        .biome_client(Some(&auth_token))
        .list_user_keys()
        .expect("Unable to list Biome user keys")
        .collect::<Vec<Key>>();
    // Validate both keys are returned
    assert_eq!(keys.len(), 2);
    // Attempt to delete the user's key, using the biome user's access token returned at log-in
    if let Some(deleted_key) = node
        .biome_client(Some(&auth_token))
        .delete_key("01234")
        .expect("Unable to delete Biome user key")
    {
        assert_eq!(&deleted_key.display_name, "first_key_updated");
        assert_eq!(&deleted_key.user_id, &user_auth.user_id);
    } else {
        panic!("Unable to delete first Biome key");
    }
    // Attempt to get the deleted key, using the biome user's access token returned at log-in
    assert!(node
        .biome_client(Some(&auth_token))
        .get_key("01234")
        .expect("Unable to retrieve key")
        .is_none());
    // Shutdown the network
    shutdown!(network).expect("Unable to shutdown network");
}

#[test]
/// This test validates the `Biome`'s user management operations available to users.
///
/// 1. Start a single-node network and retrieve the node
/// 2. Register and login a Biome user, verify this returns successfully and store the access token
///    in the response to make calls to authorized `Biome` endpoints
/// 3. List the Biome users, verify this successfully returns 1 user
/// 4. Update the user's password, using the `update_user` method, verify this returns successfully
/// 5. Attempt to `verify` the user using the old password, verify this returns an error
/// 6. Attempt to `verify` the user using the updated password, verify this returns successfully
/// 7. Register and login another Biome user, verify this returns successfully
/// 8. List the Biome users, verify this successfully returns 2 users
/// 9. Delete a Biome user, using the `delete_user` method, verify this returns successfully
/// 10. List the Biome users, verify this successfully returns 1 user
/// 11. Shutdown the network
fn test_biome_user_management() {
    // Start a single-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(1)
        .expect("Unable to start single node ActixWeb1 network");
    // Get the node in the network
    let node = network.node(0).expect("Unable to get node");
    // Register a Biome user
    assert!(node.biome_client(None).register("user", "password").is_ok());
    // Login to Biome to retrieve the user's authorization details
    let user_auth: Authorization = node
        .biome_client(None)
        .login("user", "password")
        .expect("Unable to login Biome user");
    let user_token = format!("Bearer {}", &user_auth.token);
    // List biome users, using the biome user's access token returned at log-in
    let users = node
        .biome_client(Some(&user_token))
        .list_users()
        .expect("Unable to list Biome users")
        .collect::<Vec<Credentials>>();
    // Verify this only returns one set of credentials
    assert_eq!(users.len(), 1);

    // Create the struct used to update the Biome user's password
    let update_user = UpdateUser {
        username: "user".to_string(),
        hashed_password: "password".to_string(),
        new_password: Some("new_password".to_string()),
        new_key_pairs: vec![],
    };
    // Update the biome user, using the biome user's access token returned at log-in
    let update_result = node
        .biome_client(Some(&user_token))
        .update_user(&user_auth.user_id, update_user);
    // Verify this returned successfully
    assert!(update_result.is_ok());
    // Verify the biome user with the original credentials, verify this returns an error
    assert!(node
        .biome_client(Some(&user_token))
        .verify("user", "password")
        .is_err());
    // Verify the biome user with the updated credentials, verify this returns successfully
    assert!(node
        .biome_client(Some(&user_token))
        .verify("user", "new_password")
        .is_ok());

    // Register a second Biome user
    assert!(node
        .biome_client(None)
        .register("user_two", "password")
        .is_ok());
    // Login to Biome to retrieve the user's authorization details
    let user_two_auth: Authorization = node
        .biome_client(None)
        .login("user_two", "password")
        .expect("Unable to login Biome user");
    let user_two_token = format!("Bearer {}", &user_two_auth.token);

    // List the Biome users, using the user's access token retrieved at log-in
    let users = node
        .biome_client(Some(&user_two_token))
        .list_users()
        .expect("Unable to list Biome users")
        .collect::<Vec<Credentials>>();
    // Verify two sets of credentials are returned
    assert_eq!(users.len(), 2);
    // Delete one of the Biome users (uses the user's access token), verify this returns
    // successfully
    assert!(node
        .biome_client(Some(&user_two_token))
        .delete_user(&user_two_auth.user_id)
        .is_ok());
    // List the Biome users, using the user's access token retrieved at log-in
    let users = node
        .biome_client(Some(&user_two_token))
        .list_users()
        .expect("Unable to list Biome users")
        .collect::<Vec<Credentials>>();
    // Verify only one user is returned
    assert_eq!(users.len(), 1);

    shutdown!(network).expect("Unable to shutdown network");
}
