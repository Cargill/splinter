// Copyright 2018 Cargill Incorporated
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

//! Contains functions which assist with the creation of Identity Batches and
//! Transactions

use std::time::Instant;

use crypto::digest::Digest;
use crypto::sha2::Sha512;
use crypto::sha2::Sha256;

use protobuf;
use protobuf::Message;

use sawtooth_sdk::messages::batch::Batch;
use sawtooth_sdk::messages::batch::BatchHeader;
use sawtooth_sdk::messages::batch::BatchList;
use sawtooth_sdk::messages::transaction::Transaction;
use sawtooth_sdk::messages::transaction::TransactionHeader;
use sawtooth_sdk::signing::Signer;

use error::CliError;
use protos::payload;
use protos::payload::SabrePayload_Action as Action;

/// The Sawtooth Sabre transaction family name (sabre)
const SABRE_FAMILY_NAME: &'static str = "sabre";

/// The Sawtooth Sabre transaction family version (0.0)
const SABRE_FAMILY_VERSION: &'static str = "0.0";

/// The namespace registry prefix for global state (00ec00)
const NAMESPACE_REGISTRY_PREFIX: &'static str = "00ec00";

/// The contract registry prefix for global state (00ec01)
const CONTRACT_REGISTRY_PREFIX: &'static str = "00ec01";

/// The contract prefix for global state (00ec02)
const CONTRACT_PREFIX: &'static str = "00ec02";

const SETTING_PREFIX: &'static str = "000000";

/// Creates a nonce appropriate for a TransactionHeader
fn create_nonce() -> String {
    let elapsed = Instant::now().elapsed();
    format!("{}{}", elapsed.as_secs(), elapsed.subsec_nanos())
}

/// Returns a hex string representation of the supplied bytes
///
/// # Arguments
///
/// * `b` - input bytes
fn bytes_to_hex_str(b: &[u8]) -> String {
    b.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

/// Returns a state address for a given namespace registry
///
/// # Arguments
///
/// * `namespace` - the address prefix for this namespace
fn compute_namespace_registry_address(namespace: &str) -> Result<String, CliError> {
    let prefix = match namespace.get(..6) {
        Some(x) => x,
        None => {
            return Err(CliError::UserError(format!(
                "Namespace must be at least 6 characters long: {}",
                namespace
            )))
        }
    };

    let hash: &mut [u8] = &mut [0; 64];

    let mut sha = Sha512::new();
    sha.input(prefix.as_bytes());
    sha.result(hash);

    Ok(String::from(NAMESPACE_REGISTRY_PREFIX) + &bytes_to_hex_str(hash)[..64])
}

/// Returns a state address for a given contract registry
///
/// # Arguments
///
/// * `name` - the name of the contract registry
fn compute_contract_registry_address(name: &str) -> String {
    let hash: &mut [u8] = &mut [0; 64];

    let mut sha = Sha512::new();
    sha.input(name.as_bytes());
    sha.result(hash);

    String::from(CONTRACT_REGISTRY_PREFIX) + &bytes_to_hex_str(hash)[..64]
}

/// Returns a state address for a given contract
///
/// # Arguments
///
/// * `name` - the name of the contract
/// * `version` - the version of the contract
fn compute_contract_address(name: &str, version: &str) -> String {
    let hash: &mut [u8] = &mut [0; 64];

    let s = String::from(name) + "," + version;

    let mut sha = Sha512::new();
    sha.input(s.as_bytes());
    sha.result(hash);

    String::from(CONTRACT_PREFIX) + &bytes_to_hex_str(hash)[..64]
}

/// Returns a state address for a the setting sawtooth.swa.administrators
fn compute_setting_admin_address() -> String {
    SETTING_PREFIX.to_string() + &hash_256("sawtooth", 16) + &hash_256("swa", 16)
        + &hash_256("administrators", 16) + &hash_256("", 16)
}

/// Returns a Sha256 hash of the given length
///
/// # Arguments
///
/// * `to_hash` - string to hash
/// * `num` - the length of the string returned
fn hash_256(to_hash: &str, num: usize) -> String {
    let mut sha = Sha256::new();
    sha.input_str(to_hash);
    let temp = sha.result_str().to_string();
    let hash = match temp.get(..num) {
        Some(x) => x,
        None => "",
    };
    hash.to_string()
}

/// Returns a Transaction for the given Payload and Signer
///
/// # Arguments
///
/// * `payload` - a fully populated identity payload
/// * `signer` - the signer to be used to sign the transaction
/// * `public_key` - the public key associated with the signer
///
/// # Errors
///
/// If an error occurs during serialization of the provided payload or
/// internally created `TransactionHeader`, a `CliError::ProtobufError` is
/// returned.
///
/// If a signing error occurs, a `CliError::SigningError` is returned.
pub fn create_transaction(
    payload: &payload::SabrePayload,
    signer: &Signer,
    public_key: &String,
) -> Result<Transaction, CliError> {
    let mut txn = Transaction::new();
    let mut txn_header = TransactionHeader::new();

    txn_header.set_family_name(String::from(SABRE_FAMILY_NAME));
    txn_header.set_family_version(String::from(SABRE_FAMILY_VERSION));
    txn_header.set_nonce(create_nonce());
    txn_header.set_signer_public_key(public_key.clone());
    txn_header.set_batcher_public_key(public_key.clone());

    let (input_addresses, output_addresses) = match payload.get_action() {
        Action::ACTION_UNSET => panic!("payload action was ACTION_UNSET"),
        Action::CREATE_CONTRACT => {
            let name = payload.get_create_contract().get_name();
            let version = payload.get_create_contract().get_version();

            let addresses = vec![
                compute_contract_registry_address(name),
                compute_contract_address(name, version),
            ];

            (addresses.clone(), addresses)
        }
        Action::DELETE_CONTRACT => {
            let name = payload.get_delete_contract().get_name();
            let version = payload.get_delete_contract().get_version();

            let addresses = vec![
                compute_contract_registry_address(name),
                compute_contract_address(name, version),
            ];

            (addresses.clone(), addresses)
        }
        Action::EXECUTE_CONTRACT => {
            let name = payload.get_execute_contract().get_name();
            let version = payload.get_execute_contract().get_version();

            let mut input_addresses = vec![
                compute_contract_registry_address(name),
                compute_contract_address(name, version),
            ];
            for input in payload.get_execute_contract().get_inputs() {
                let namespace = match input.get(..6) {
                    Some(namespace) => namespace,
                    None => {
                        return Err(CliError::UserError(format!(
                            "Input must be at least 6 characters long: {}",
                            input
                        )))
                    }
                };

                input_addresses.push(compute_namespace_registry_address(namespace)?);
            }
            input_addresses.append(&mut payload.get_execute_contract().get_inputs().to_vec());

            let mut output_addresses = vec![
                compute_contract_registry_address(name),
                compute_contract_address(name, version),
            ];

            for output in payload.get_execute_contract().get_outputs() {
                let namespace = match output.get(..6) {
                    Some(namespace) => namespace,
                    None => {
                        return Err(CliError::UserError(format!(
                            "Output must be at least 6 characters long: {}",
                            output
                        )))
                    }
                };

                output_addresses.push(compute_namespace_registry_address(namespace)?);
            }
            output_addresses.append(&mut payload.get_execute_contract().get_outputs().to_vec());

            (input_addresses, output_addresses)
        }
        Action::CREATE_CONTRACT_REGISTRY => {
            let name = payload.get_create_contract_registry().get_name();
            let addresses = vec![compute_contract_registry_address(name)];
            (addresses.clone(), addresses)
        }
        Action::DELETE_CONTRACT_REGISTRY => {
            let name = payload.get_delete_contract_registry().get_name();
            let addresses = vec![compute_contract_registry_address(name)];
            (addresses.clone(), addresses)
        }
        Action::UPDATE_CONTRACT_REGISTRY_OWNERS => {
            let name = payload.get_update_contract_registry_owners().get_name();
            let addresses = vec![compute_contract_registry_address(name)];
            (addresses.clone(), addresses)
        }
        Action::CREATE_NAMESPACE_REGISTRY => {
            let namespace = payload.get_create_namespace_registry().get_namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                compute_setting_admin_address(),
            ];
            (addresses.clone(), addresses)
        }
        Action::DELETE_NAMESPACE_REGISTRY => {
            let namespace = payload.get_delete_namespace_registry().get_namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                compute_setting_admin_address(),
            ];
            (addresses.clone(), addresses)
        }
        Action::UPDATE_NAMESPACE_REGISTRY_OWNERS => {
            let namespace = payload
                .get_update_namespace_registry_owners()
                .get_namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                compute_setting_admin_address(),
            ];
            (addresses.clone(), addresses)
        }
        Action::CREATE_NAMESPACE_REGISTRY_PERMISSION => {
            let namespace = payload
                .get_create_namespace_registry_permission()
                .get_namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                compute_setting_admin_address(),
            ];
            (addresses.clone(), addresses)
        }
        Action::DELETE_NAMESPACE_REGISTRY_PERMISSION => {
            let namespace = payload
                .get_delete_namespace_registry_permission()
                .get_namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                compute_setting_admin_address(),
            ];
            (addresses.clone(), addresses)
        }
    };

    txn_header.set_inputs(protobuf::RepeatedField::from_vec(input_addresses));
    txn_header.set_outputs(protobuf::RepeatedField::from_vec(output_addresses));

    let payload_bytes = payload.write_to_bytes()?;
    let mut sha = Sha512::new();
    sha.input(&payload_bytes);
    let hash: &mut [u8] = &mut [0; 64];
    sha.result(hash);
    txn_header.set_payload_sha512(bytes_to_hex_str(hash));
    txn.set_payload(payload_bytes);

    let txn_header_bytes = txn_header.write_to_bytes()?;
    txn.set_header(txn_header_bytes.clone());

    let b: &[u8] = &txn_header_bytes;
    txn.set_header_signature(signer.sign(b)?);

    Ok(txn)
}

/// Returns a Batch for the given Transaction and Signer
///
/// # Arguments
///
/// * `txn` - a Transaction
/// * `signer` - the signer to be used to sign the transaction
/// * `public_key` - the public key associated with the signer
///
/// # Errors
///
/// If an error occurs during serialization of the provided Transaction or
/// internally created `BatchHeader`, a `CliError::ProtobufError` is
/// returned.
///
/// If a signing error occurs, a `CliError::SigningError` is returned.
pub fn create_batch(
    txn: Transaction,
    signer: &Signer,
    public_key: &String,
) -> Result<Batch, CliError> {
    let mut batch = Batch::new();
    let mut batch_header = BatchHeader::new();

    batch_header.set_transaction_ids(protobuf::RepeatedField::from_vec(vec![
        txn.header_signature.clone(),
    ]));
    batch_header.set_signer_public_key(public_key.clone());
    batch.set_transactions(protobuf::RepeatedField::from_vec(vec![txn]));

    let batch_header_bytes = batch_header.write_to_bytes()?;
    batch.set_header(batch_header_bytes.clone());

    let b: &[u8] = &batch_header_bytes;
    batch.set_header_signature(signer.sign(b)?);

    Ok(batch)
}

/// Returns a BatchList containing the provided Batch
///
/// # Arguments
///
/// * `batch` - a Batch
pub fn create_batch_list_from_one(batch: Batch) -> BatchList {
    let mut batch_list = BatchList::new();
    batch_list.set_batches(protobuf::RepeatedField::from_vec(vec![batch]));
    return batch_list;
}
