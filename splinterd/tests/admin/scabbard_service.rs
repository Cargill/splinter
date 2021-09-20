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

//! Integration tests for scabbard

use std::env;
use std::time::Duration;

use cylinder::Signer;
use rand::prelude::*;
use sha2::{Digest, Sha512};
use splinter::admin::messages::AuthorizationType;
use splinterd::node::RestApiVariant;
use transact::protocol::batch::Batch;

use crate::admin::circuit_commit::commit_2_party_circuit;
use crate::admin::{
    get_node_service_id,
    payload::{
        make_command_batch, make_create_contract_registry_batch, make_namespace_create_batch,
        make_namespace_permissions_batch, make_upload_contract_batch,
    },
};
use crate::framework::network::Network;

const COMMAND_NAME: &str = "command";
const COMMAND_VERSION: &str = "1.0";
const COMMAND_PREFIX: &str = "06abbc";

/// Test that the batches to create a contract registry, upload a smart contract,
/// create a namespace and grant namespace permissions can be successfully submitted
/// to scabbard.
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
#[test]
pub fn test_scabbard_upload_smart_contract() {
    let path = env::current_dir().expect("couldn't get current dir");
    println!("The current directory is {}", path.display());

    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "GHIJK-67890";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    for batch in get_command_contract_setup_batches(&*node_a.admin_signer()) {
        assert!(client
            .submit(&service_id_a, vec![batch], Some(Duration::from_secs(10)))
            .is_ok());
    }

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `set-state`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `set-state` command and check that the
///    batch is submitted successfully.
/// 6. Retrieve the value that was set for the address in the set-state command and
///    check that it is the expected value.
#[test]
pub fn test_scabbard_set_state() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let batch = make_command_batch("set-state", address.clone(), &*node_a.admin_signer())
        .expect("failed to make set-state batch");

    assert!(client
        .submit(&service_id_a, vec![batch], Some(Duration::from_secs(10)),)
        .is_ok());

    assert_eq!(
        "state_value",
        String::from_utf8(
            client
                .get_state_at_address(&service_id_a, &address)
                .expect("failed to get state")
                .unwrap()
        )
        .expect("can't convert state value to string")
    );

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `get-state`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `set-state` command and check that the
///    batch is submitted successfully.
/// 6. Retrieve the value that was set for the address in the set-state command and
///    check that it is the expected value.
/// 7. Submit a batch with a command family `get-state` command and check that the
///    batch is submitted successfully.
#[test]
pub fn test_scabbard_get_state() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "BCDEF-12345";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let set_state_batch = make_command_batch("set-state", address.clone(), &*node_a.admin_signer())
        .expect("failed to make set-state batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![set_state_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    assert_eq!(
        "state_value",
        String::from_utf8(
            client
                .get_state_at_address(&service_id_a, &address)
                .expect("failed to get state")
                .unwrap()
        )
        .expect("can't convert state value to string")
    );

    let get_state_batch = make_command_batch("get-state", address.clone(), &*node_a.admin_signer())
        .expect("failed to make get-state batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![get_state_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `add-event`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `set-state` command and check that the
///    batch is submitted successfully.
/// 6. Retrieve the value that was set for the address in the set-state command and
///    check that it is the expected value.
/// 7. Submit a batch with a command family `add-event` command and check that the
///    batch is submitted successfully.
#[test]
pub fn test_scabbard_add_event() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "CDEFG-23456";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let set_state_batch = make_command_batch("set-state", address.clone(), &*node_a.admin_signer())
        .expect("failed to make set-state batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![set_state_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    assert_eq!(
        "state_value",
        String::from_utf8(
            client
                .get_state_at_address(&service_id_a, &address)
                .expect("failed to get state")
                .unwrap()
        )
        .expect("can't convert state value to string")
    );

    let add_event_batch = make_command_batch("add-event", address.clone(), &*node_a.admin_signer())
        .expect("failed to make add-event batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![add_event_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `delete-state`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `set-state` command and check that the
///    batch is submitted successfully.
/// 6. Retrieve the value that was set for the address in the set-state command and
///    check that it is the expected value.
/// 7. Submit a batch with a command family `delete-state` command that attempts to
///    delete the state that was previously set.
/// 8. Check that the batch is submitted successfully.
/// 9. Check that the state at the address given in the delete state command can no
///    longer be retrieved.
#[test]
pub fn test_scabbard_delete_state() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "DEFGH-34567";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let set_state_batch = make_command_batch("set-state", address.clone(), &*node_a.admin_signer())
        .expect("failed to make set-state batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![set_state_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    assert_eq!(
        "state_value",
        String::from_utf8(
            client
                .get_state_at_address(&service_id_a, &address)
                .expect("failed to get state")
                .unwrap()
        )
        .expect("can't convert state value to string")
    );

    let delete_state_batch =
        make_command_batch("delete-state", address.clone(), &*node_a.admin_signer())
            .expect("failed to make delete-state batch");

    assert!(client
        .submit(
            &service_id_a,
            vec![delete_state_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    assert!(client
        .get_state_at_address(&service_id_a, &address)
        .expect("failed to get state")
        .is_none());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `return-invalid`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `return-invalid` command.
/// 6. Check that the 'invalid' error is returned when the batch is submitted.
#[test]
pub fn test_scabbard_return_invalid() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "EFGHI-45678";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let return_invalid_batch =
        make_command_batch("return-invalid", address.clone(), &*node_a.admin_signer())
            .expect("failed to make return-invalid batch");

    assert!(!client
        .submit(
            &service_id_a,
            vec![return_invalid_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that scabbard can handle a batch containing a command family `return-internal-error`
/// transaction
///
/// 1. Start a network with 2 nodes
/// 2. Create a 2 party circuit
/// 3. Create the 4 batches needed to upload the command smart contract
///    and grant necessary permissions.
/// 4. Submit the upload contract batches to scabbard and check that they are
///    submitted successfully.
/// 5. Submit a batch with a command family `return-internal-error` command.
/// 6. Check that the 'return-internal-error' error is returned when the batch is submitted.
#[test]
pub fn test_scabbard_return_internal_error() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "EFGHI-45678";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(1);

    let address = &address_vec[0];

    let return_internal_error_batch = make_command_batch(
        "return-internal-error",
        address.clone(),
        &*node_a.admin_signer(),
    )
    .expect("failed to make return-internal-error batch");

    assert!(!client
        .submit(
            &service_id_a,
            vec![return_internal_error_batch],
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

#[test]
fn test_scabbard_command_workload() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");

    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);

    let client = node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient");

    assert!(client
        .submit(
            &service_id_a,
            get_command_contract_setup_batches(&*node_a.admin_signer()),
            Some(Duration::from_secs(10)),
        )
        .is_ok());

    let address_vec = generate_rand_addresses(10);

    let playlist = create_command_playlist(address_vec, &*node_a.admin_signer());

    for (command_type, batch) in playlist {
        if command_type == "return-internal-error".to_string()
            || command_type == "return-invalid".to_string()
        {
            assert!(!client
                .submit(&service_id_a, vec![batch], Some(Duration::from_secs(10)),)
                .is_ok());
        } else {
            assert!(client
                .submit(&service_id_a, vec![batch], Some(Duration::from_secs(10)),)
                .is_ok());
        }
    }

    shutdown!(network).expect("Unable to shutdown network");
}

// Get the batches that are required to create and upload a smart contract and set
// the necessary permissions
fn get_command_contract_setup_batches(signer: &dyn Signer) -> Vec<Batch> {
    vec![
        make_create_contract_registry_batch(COMMAND_NAME, &*signer)
            .expect("Unable to build `CreateContractRegistryAction`"),
        make_upload_contract_batch(
            COMMAND_NAME,
            COMMAND_VERSION,
            COMMAND_PREFIX,
            "tests/contracts/command/target/wasm32-unknown-unknown/release/command.wasm",
            &*signer,
        )
        .expect("Unable to build `CreateContractAction`"),
        make_namespace_create_batch(COMMAND_PREFIX, &*signer)
            .expect("Unable to build `CreateNamespaceRegistryAction`"),
        make_namespace_permissions_batch(COMMAND_NAME, COMMAND_PREFIX, &*signer)
            .expect("Unable to build `CreateNamespaceRegistryPermissionAction`"),
    ]
}

// Generate a random address that has the command family prefix
fn generate_rand_addresses(num_addresses: usize) -> Vec<String> {
    let mut addresses = Vec::new();
    for i in 0..num_addresses {
        let rand: i32 = rand::random();

        let mut sha = Sha512::new();
        sha.update(format!("address{}{}", i, rand).as_bytes());
        let hash = &mut sha.finalize();

        let hex = hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join("");

        addresses.push(String::from(COMMAND_PREFIX) + &hex[0..64])
    }
    addresses
}

fn create_command_playlist(addresses: Vec<String>, signer: &dyn Signer) -> Vec<(String, Batch)> {
    let mut rng: StdRng = SeedableRng::seed_from_u64(10);
    let mut batches = Vec::new();

    for address in addresses {
        let command: &str = match rng.gen_range(0, 6) {
            0 => "set-state",
            1 => "get-state",
            2 => {
                let b = make_command_batch("set-state", address.clone(), signer)
                    .expect("failed to make return-internal-error batch");
                batches.push(("set-state".to_string(), b));
                "delete-state"
            }
            3 => "add-event",
            4 => "return-invalid",
            5 => "return-internal-error",
            _ => panic!("Should not have generated outside of [0, 5)"),
        };
        let batch = make_command_batch(command, address, signer)
            .expect("failed to make return-internal-error batch");
        batches.push((command.to_string(), batch));
    }
    batches
}
