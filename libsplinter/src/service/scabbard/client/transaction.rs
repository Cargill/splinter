// Copyright 2020 Cargill Incorporated
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

//! Contains functions which assist with the creation of Sabre batches and
//! transactions.

use std::iter::FromIterator;
use std::time::Instant;

use protobuf::{self, Message};
use sabre_sdk::{
    protocol::{
        payload::{Action, SabrePayload},
        ADMINISTRATORS_SETTING_ADDRESS,
    },
    protos::IntoBytes,
};
use sawtooth_sdk::{
    messages::{
        batch::{Batch, BatchHeader, BatchList},
        transaction::{Transaction, TransactionHeader},
    },
    signing::{self, secp256k1::Secp256k1PrivateKey, CryptoFactory, Signer},
};

use crate::signing::{hash::HashSigner, Signer as _};

use super::Error;

/// The Sawtooth Sabre transaction family name (sabre)
const SABRE_FAMILY_NAME: &str = "sabre";
/// The Sawtooth Sabre transaction family version (0.4)
const SABRE_FAMILY_VERSION: &str = "0.4";
/// The namespace registry prefix for global state (00ec00)
const NAMESPACE_REGISTRY_PREFIX: &str = "00ec00";
/// The contract registry prefix for global state (00ec01)
const CONTRACT_REGISTRY_PREFIX: &str = "00ec01";
/// The contract prefix for global state (00ec02)
const CONTRACT_PREFIX: &str = "00ec02";
/// The smart permission prefix for global state (00ec03)
const SMART_PERMISSION_PREFIX: &str = "00ec03";

const PIKE_AGENT_PREFIX: &str = "cad11d00";
const PIKE_ORG_PREFIX: &str = "cad11d01";

/// Takes an `Iterator` of `SabrePayload`s, signs the payloads with the private key, and wraps it
/// in a `BatchList` to be submitted to scabbard.
///
/// This is a convenience function that creates the signing context and signer from the private
/// key, then calls the `create_transaction`, `create_batch`, and `create_batch_list_from_one`
/// functions.
pub fn batch_payloads(
    payloads: &mut dyn Iterator<Item = SabrePayload>,
    signing_key: &Secp256k1PrivateKey,
) -> Result<BatchList, Error> {
    let context = signing::create_context("secp256k1")
        .map_err(|err| Error(format!("failed to create secp256k1 context: {}", err)))?;
    let factory = CryptoFactory::new(&*context);
    let signer = factory.new_signer(signing_key);

    let txns = payloads
        .map(|payload| create_transaction(payload, &signer))
        .collect::<Result<Vec<_>, _>>()?;
    let batch = create_batch(txns, &signer)?;
    let batch_list = create_batch_list_from_one(batch);

    Ok(batch_list)
}

/// Returns a Transaction for the given Payload and Signer
///
/// # Arguments
///
/// * `payload` - a fully populated identity payload
/// * `signer` - the signer to be used to sign the transaction
///
/// # Errors
///
/// An error is returned if it occurs during serialization of the provided payload or internally
/// created `TransactionHeader`, or if a signing error occurs.
pub fn create_transaction(payload: SabrePayload, signer: &Signer) -> Result<Transaction, Error> {
    let public_key = signer
        .get_public_key()
        .map_err(|err| Error(format!("failed to get public key from signer: {}", err)))?
        .as_hex();

    let mut txn = Transaction::new();
    let mut txn_header = TransactionHeader::new();

    txn_header.set_family_name(String::from(SABRE_FAMILY_NAME));
    txn_header.set_family_version(String::from(SABRE_FAMILY_VERSION));
    txn_header.set_nonce(create_nonce());
    txn_header.set_signer_public_key(public_key.clone());
    txn_header.set_batcher_public_key(public_key.clone());

    let (input_addresses, output_addresses) = match payload.action() {
        Action::CreateContract(create_contract) => {
            let name = create_contract.name();
            let version = create_contract.version();

            let addresses = vec![
                compute_contract_registry_address(name)?,
                compute_contract_address(name, version)?,
            ];

            (addresses.clone(), addresses)
        }
        Action::DeleteContract(delete_contract) => {
            let name = delete_contract.name();
            let version = delete_contract.version();

            let addresses = vec![
                compute_contract_registry_address(name)?,
                compute_contract_address(name, version)?,
            ];

            (addresses.clone(), addresses)
        }
        Action::ExecuteContract(execute_contract) => {
            let name = execute_contract.name();
            let version = execute_contract.version();

            let mut input_addresses = vec![
                compute_contract_registry_address(name)?,
                compute_contract_address(name, version)?,
            ];
            for input in execute_contract.inputs() {
                let namespace = match input.get(..6) {
                    Some(namespace) => namespace,
                    None => {
                        return Err(Error(format!(
                            "invalid input: '{}' is less than 6 characters long",
                            input,
                        )));
                    }
                };

                input_addresses.push(compute_namespace_registry_address(namespace)?);
            }
            input_addresses.append(&mut execute_contract.inputs().to_vec());

            let mut output_addresses = vec![
                compute_contract_registry_address(name)?,
                compute_contract_address(name, version)?,
            ];

            for output in execute_contract.outputs() {
                let namespace = match output.get(..6) {
                    Some(namespace) => namespace,
                    None => {
                        return Err(Error(format!(
                            "invalid output: '{}' is less than 6 characters long",
                            output
                        )));
                    }
                };

                output_addresses.push(compute_namespace_registry_address(namespace)?);
            }
            output_addresses.append(&mut execute_contract.outputs().to_vec());

            (input_addresses, output_addresses)
        }
        Action::CreateContractRegistry(create_contract_registry) => {
            let name = create_contract_registry.name();
            let addresses = vec![
                compute_contract_registry_address(name)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::DeleteContractRegistry(delete_contract_registry) => {
            let name = delete_contract_registry.name();
            let addresses = vec![
                compute_contract_registry_address(name)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::UpdateContractRegistryOwners(update_contract_registry_owners) => {
            let name = update_contract_registry_owners.name();
            let addresses = vec![
                compute_contract_registry_address(name)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::CreateNamespaceRegistry(create_namespace_registry) => {
            let namespace = create_namespace_registry.namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::DeleteNamespaceRegistry(delete_namespace_registry) => {
            let namespace = delete_namespace_registry.namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::UpdateNamespaceRegistryOwners(update_namespace_registry_owners) => {
            let namespace = update_namespace_registry_owners.namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::CreateNamespaceRegistryPermission(create_namespace_registry_permission) => {
            let namespace = create_namespace_registry_permission.namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::DeleteNamespaceRegistryPermission(delete_namespace_registry_permission) => {
            let namespace = delete_namespace_registry_permission.namespace();
            let addresses = vec![
                compute_namespace_registry_address(namespace)?,
                ADMINISTRATORS_SETTING_ADDRESS.into(),
            ];
            (addresses.clone(), addresses)
        }
        Action::CreateSmartPermission(create_smart_permission) => {
            let org_id = create_smart_permission.org_id();
            let name = create_smart_permission.name();
            let addresses = vec![
                compute_smart_permission_address(org_id, name)?,
                compute_org_address(org_id)?,
                compute_agent_address(&public_key)?,
            ];

            (addresses.clone(), addresses)
        }
        Action::UpdateSmartPermission(update_smart_permission) => {
            let org_id = update_smart_permission.org_id();
            let name = update_smart_permission.name();
            let addresses = vec![
                compute_smart_permission_address(org_id, name)?,
                compute_org_address(org_id)?,
                compute_agent_address(&public_key)?,
            ];

            (addresses.clone(), addresses)
        }
        Action::DeleteSmartPermission(delete_smart_permission) => {
            let org_id = delete_smart_permission.org_id();
            let name = delete_smart_permission.name();
            let addresses = vec![
                compute_smart_permission_address(org_id, name)?,
                compute_org_address(org_id)?,
                compute_agent_address(&public_key)?,
            ];

            (addresses.clone(), addresses)
        }
    };

    txn_header.set_inputs(protobuf::RepeatedField::from_vec(input_addresses));
    txn_header.set_outputs(protobuf::RepeatedField::from_vec(output_addresses));

    let payload_bytes = payload.into_bytes()?;
    let hash = HashSigner
        .sign(&payload_bytes)
        .map_err(|err| Error(format!("failed to hash payload bytes: {}", err)))?;
    txn_header.set_payload_sha512(bytes_to_hex_str(&hash));
    txn.set_payload(payload_bytes);

    let txn_header_bytes = txn_header.write_to_bytes()?;
    txn.set_header(txn_header_bytes.clone());

    let b: &[u8] = &txn_header_bytes;
    txn.set_header_signature(signer.sign(b)?);

    Ok(txn)
}

/// Returns a Batch for the given Transactions and Signer
///
/// # Arguments
///
/// * `txns` - transaction to put in the batch
/// * `signer` - the signer to be used to sign the batch
///
/// # Errors
///
/// An error is returned if it occurs during serialization of the provided transactions or
/// internally created `BatchHeader`, or if a signing error occurs.
pub fn create_batch(txns: Vec<Transaction>, signer: &Signer) -> Result<Batch, Error> {
    let public_key = signer
        .get_public_key()
        .map_err(|err| Error(format!("failed to get public key from signer: {}", err)))?
        .as_hex();

    let mut batch = Batch::new();
    let mut batch_header = BatchHeader::new();

    batch_header.set_transaction_ids(protobuf::RepeatedField::from_iter(
        txns.iter().map(|txn| txn.header_signature.clone()),
    ));
    batch_header.set_signer_public_key(public_key);
    batch.set_transactions(txns.into());

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
    batch_list
}

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
fn compute_namespace_registry_address(namespace: &str) -> Result<String, Error> {
    let prefix = match namespace.get(..6) {
        Some(x) => x,
        None => {
            return Err(Error(format!(
                "invalid namespace: '{}' is less than 6 characters long",
                namespace,
            )));
        }
    };

    let hash = HashSigner.sign(prefix.as_bytes()).map_err(|err| {
        Error(format!(
            "failed to hash namespace registry address: {}",
            err
        ))
    })?;

    Ok(String::from(NAMESPACE_REGISTRY_PREFIX) + &bytes_to_hex_str(&hash)[..64])
}

/// Returns a state address for a given contract registry
///
/// # Arguments
///
/// * `name` - the name of the contract registry
fn compute_contract_registry_address(name: &str) -> Result<String, Error> {
    let hash = HashSigner
        .sign(name.as_bytes())
        .map_err(|err| Error(format!("failed to hash contract registry address: {}", err)))?;

    Ok(String::from(CONTRACT_REGISTRY_PREFIX) + &bytes_to_hex_str(&hash)[..64])
}

/// Returns a state address for a given contract
///
/// # Arguments
///
/// * `name` - the name of the contract
/// * `version` - the version of the contract
fn compute_contract_address(name: &str, version: &str) -> Result<String, Error> {
    let s = String::from(name) + "," + version;

    let hash = HashSigner
        .sign(s.as_bytes())
        .map_err(|err| Error(format!("failed to hash contract address: {}", err)))?;

    Ok(String::from(CONTRACT_PREFIX) + &bytes_to_hex_str(&hash)[..64])
}

/// Returns a state address for a given agent name
///
/// # Arguments
///
/// * `name` - the agent's name
fn compute_agent_address(name: &str) -> Result<String, Error> {
    let hash = HashSigner
        .sign(name.as_bytes())
        .map_err(|err| Error(format!("failed to hash pike agent address: {}", err)))?;

    Ok(String::from(PIKE_AGENT_PREFIX) + &bytes_to_hex_str(&hash)[..62])
}

/// Returns a state address for a given organization id
///
/// # Arguments
///
/// * `id` - the organization's id
fn compute_org_address(id: &str) -> Result<String, Error> {
    let hash = HashSigner
        .sign(id.as_bytes())
        .map_err(|err| Error(format!("failed to hash pike org address: {}", err)))?;

    Ok(String::from(PIKE_ORG_PREFIX) + &bytes_to_hex_str(&hash)[..62])
}

/// Returns a state address for a given smart permission
///
/// # Arguments
///
/// * `org_id` - the organization's id
/// * `name` - smart permission name
fn compute_smart_permission_address(org_id: &str, name: &str) -> Result<String, Error> {
    let org_id_hash = HashSigner
        .sign(org_id.as_bytes())
        .map_err(|err| Error(format!("failed to hash pike org id: {}", err)))?;

    let name_hash = HashSigner
        .sign(name.as_bytes())
        .map_err(|err| Error(format!("failed to hash pike org id: {}", err)))?;

    Ok(String::from(SMART_PERMISSION_PREFIX)
        + &bytes_to_hex_str(&org_id_hash)[..6]
        + &bytes_to_hex_str(&name_hash)[..58])
}

impl From<protobuf::error::ProtobufError> for Error {
    fn from(err: protobuf::error::ProtobufError) -> Self {
        Error(format!("failed to write protobuf to bytes: {}", err))
    }
}

impl From<sabre_sdk::protos::ProtoConversionError> for Error {
    fn from(err: sabre_sdk::protos::ProtoConversionError) -> Self {
        Error(format!("failed to convert protobuf into bytes: {}", err))
    }
}

impl From<sawtooth_sdk::signing::Error> for Error {
    fn from(err: sawtooth_sdk::signing::Error) -> Self {
        Error(format!("signing failed: {}", err))
    }
}
