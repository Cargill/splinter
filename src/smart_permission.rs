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
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use error::CliError;
use key;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};
use submit::submit_batch_list;
use protos::payload::{
    SabrePayload,
    SabrePayload_Action,
    CreateSmartPermissionAction,
    UpdateSmartPermissionAction,
    DeleteSmartPermissionAction
};

use protobuf::Message;


pub fn do_create(
    url: &str,
    org_id: &str,
    name: &str,
    filename: &str,
    key: Option<&str>,
    output: Option<&str>) -> Result<String, CliError> {

    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let mut action = CreateSmartPermissionAction::new();
    action.set_name(name.to_string());
    action.set_org_id(org_id.to_string());
    action.set_function(function);

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::CREATE_SMART_PERMISSION;
    payload.set_create_smart_permission(action);

    if let Some(o) = output {
        let mut buffer = File::create(o)?;
        let payload_bytes = payload.write_to_bytes()?;
        buffer.write_all(&payload_bytes).map_err(|err| CliError::IoError(err))?;
    }

    let txn = create_transaction(&payload, &signer, &public_key)?;
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
    output: Option<&str>) -> Result<String, CliError> {

    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let mut action = UpdateSmartPermissionAction::new();
    action.set_name(name.to_string());
    action.set_org_id(org_id.to_string());
    action.set_function(function);

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::UPDATE_SMART_PERMISSION;
    payload.set_update_smart_permission(action);

    if let Some(o) = output {
        let mut buffer = File::create(o)?;
        let payload_bytes = payload.write_to_bytes()?;
        buffer.write_all(&payload_bytes).map_err(|err| CliError::IoError(err))?;
    }

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

pub fn do_delete(
    url: &str,
    org_id: &str,
    name: &str,
    key: Option<&str>,
    output: Option<&str>) -> Result<String, CliError> {

    let private_key = key::load_signing_key(key)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let mut action = DeleteSmartPermissionAction::new();
    action.set_name(name.to_string());
    action.set_org_id(org_id.to_string());

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::UPDATE_SMART_PERMISSION;
    payload.set_delete_smart_permission(action);

    if let Some(o) = output {
        let mut buffer = File::create(o)?;
        let payload_bytes = payload.write_to_bytes()?;
        buffer.write_all(&payload_bytes).map_err(|err| CliError::IoError(err))?;
    }

    let txn = create_transaction(&payload, &signer, &public_key)?;
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
