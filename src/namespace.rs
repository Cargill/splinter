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

use protobuf;
use sawtooth_sdk::signing;

use error::CliError;
use key;
use protos::payload::{SabrePayload, SabrePayload_Action};
use protos::payload::CreateNamespaceRegistryAction;
use protos::payload::UpdateNamespaceRegistryOwnersAction;
use protos::payload::DeleteNamespaceRegistryAction;
use protos::payload::CreateNamespaceRegistryPermissionAction;
use protos::payload::DeleteNamespaceRegistryPermissionAction;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};
use submit::submit_batch_list;

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

    let mut action = CreateNamespaceRegistryAction::new();
    action.set_namespace(namespace.into());
    action.set_owners(protobuf::RepeatedField::from_vec(owners));

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::CREATE_NAMESPACE_REGISTRY;
    payload.set_create_namespace_registry(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
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

    let mut action = UpdateNamespaceRegistryOwnersAction::new();
    action.set_namespace(namespace.into());
    action.set_owners(protobuf::RepeatedField::from_vec(owners));

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::UPDATE_NAMESPACE_REGISTRY_OWNERS;
    payload.set_update_namespace_registry_owners(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_ns_delete(key_name: Option<&str>, url: &str, namespace: &str) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = DeleteNamespaceRegistryAction::new();
    action.set_namespace(namespace.into());

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::DELETE_NAMESPACE_REGISTRY;
    payload.set_delete_namespace_registry(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
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

    let mut action = CreateNamespaceRegistryPermissionAction::new();
    action.set_namespace(namespace.into());
    action.set_contract_name(contract.into());
    action.set_read(read);
    action.set_write(write);

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::CREATE_NAMESPACE_REGISTRY_PERMISSION;
    payload.set_create_namespace_registry_permission(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}
pub fn do_perm_delete(key_name: Option<&str>, url: &str, namespace: &str) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = DeleteNamespaceRegistryPermissionAction::new();
    action.set_namespace(namespace.into());

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::DELETE_NAMESPACE_REGISTRY_PERMISSION;
    payload.set_delete_namespace_registry_permission(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}
