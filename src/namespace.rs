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

use sabre_sdk::protocol::payload::{
    Action, CreateNamespaceRegistryActionBuilder, CreateNamespaceRegistryPermissionActionBuilder,
    DeleteNamespaceRegistryActionBuilder, DeleteNamespaceRegistryPermissionActionBuilder,
    SabrePayloadBuilder, UpdateNamespaceRegistryOwnersActionBuilder,
};
use sawtooth_sdk::signing;

use error::CliError;
use key;
use submit::submit_batch_list;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};

pub fn do_ns_create(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = CreateNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateNamespaceRegistry(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_ns_update(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = UpdateNamespaceRegistryOwnersActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::UpdateNamespaceRegistryOwners(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_ns_delete(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = DeleteNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteNamespaceRegistry(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_perm_create(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    contract: &str,
    read: bool,
    write: bool,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = CreateNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .with_contract_name(contract.into())
        .with_read(read)
        .with_write(write)
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateNamespaceRegistryPermission(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}
pub fn do_perm_delete(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = DeleteNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteNamespaceRegistryPermission(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}
