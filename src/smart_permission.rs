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
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use sabre_sdk::protocol::payload::{
    Action, CreateSmartPermissionActionBuilder, DeleteSmartPermissionActionBuilder,
    SabrePayloadBuilder, UpdateSmartPermissionActionBuilder,
};
use sawtooth_sdk::signing;

use crate::error::CliError;
use crate::key;
use crate::submit::submit_batch_list;
use crate::transaction::{create_batch, create_batch_list_from_one, create_transaction};

pub fn do_create(
    url: &str,
    org_id: &str,
    name: &str,
    filename: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let action = CreateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(function)
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateSmartPermission(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_update(
    url: &str,
    org_id: &str,
    name: &str,
    filename: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let action = UpdateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(function)
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::UpdateSmartPermission(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_delete(
    url: &str,
    org_id: &str,
    name: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let action = DeleteSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .build()?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteSmartPermission(action))
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

fn load_smart_permission_file(path: &Path) -> Result<Vec<u8>, CliError> {
    let file = File::open(path).map_err(|e| {
        CliError::UserError(format!(
            "Could not load smart permission \"{}\": {}",
            path.display(),
            e
        ))
    })?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader.read_to_end(&mut contents).map_err(|e| {
        CliError::UserError(format!(
            "IoError while reading smart permission \"{}\": {}",
            path.display(),
            e
        ))
    })?;

    Ok(contents)
}
