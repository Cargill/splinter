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
use protos::payload::CreateContractRegistryAction;
use protos::payload::UpdateContractRegistryOwnersAction;
use protos::payload::DeleteContractRegistryAction;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};
use submit::submit_batch_list;

pub fn do_cr_create(
    key_name: Option<&str>,
    url: &str,
    name: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = CreateContractRegistryAction::new();
    action.set_name(name.into());
    action.set_owners(protobuf::RepeatedField::from_vec(owners));

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::CREATE_CONTRACT_REGISTRY;
    payload.set_create_contract_registry(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_cr_update(
    key_name: Option<&str>,
    url: &str,
    name: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = UpdateContractRegistryOwnersAction::new();
    action.set_name(name.into());
    action.set_owners(protobuf::RepeatedField::from_vec(owners));

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::UPDATE_CONTRACT_REGISTRY_OWNERS;
    payload.set_update_contract_registry_owners(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_cr_delete(key_name: Option<&str>, url: &str, name: &str) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = DeleteContractRegistryAction::new();
    action.set_name(name.into());

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::DELETE_CONTRACT_REGISTRY;
    payload.set_delete_contract_registry(action);

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}
