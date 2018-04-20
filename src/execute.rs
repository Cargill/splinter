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
use std::io::BufReader;
use std::io::prelude::*;

use protobuf;
use sawtooth_sdk::signing;

use error::CliError;
use key;
use protos::payload::{SabrePayload, SabrePayload_Action};
use protos::payload::ExecuteContractAction;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};
use submit::submit_batch_list;

pub fn do_exec(
    name: &str,
    version: &str,
    payload_file: &str,
    inputs: Vec<String>,
    outputs: Vec<String>,
    key_name: Option<&str>,
    url: &str,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let contract_payload = load_contract_payload_file(payload_file)?;

    let txn_payload = create_exec_txn_payload(name, version, inputs, outputs, contract_payload);

    let txn = create_transaction(&txn_payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

fn create_exec_txn_payload(
    name: &str,
    version: &str,
    inputs: Vec<String>,
    outputs: Vec<String>,
    contract_payload: Vec<u8>,
) -> SabrePayload {
    let mut exec_contract = ExecuteContractAction::new();
    exec_contract.set_name(name.into());
    exec_contract.set_version(version.into());
    exec_contract.set_inputs(protobuf::RepeatedField::from_vec(inputs));
    exec_contract.set_outputs(protobuf::RepeatedField::from_vec(outputs));
    exec_contract.set_payload(contract_payload);

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::EXECUTE_CONTRACT;
    payload.set_execute_contract(exec_contract);
    payload
}

fn load_contract_payload_file(payload_file: &str) -> Result<Vec<u8>, CliError> {
    let file = File::open(payload_file).map_err(|e| {
        CliError::UserError(format!(
            "Could not load payload \"{}\": {}",
            payload_file, e
        ))
    })?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader.read_to_end(&mut contents).map_err(|e| {
        CliError::UserError(format!(
            "IoError while reading payload \"{}\": {}",
            payload_file, e
        ))
    })?;

    Ok(contents)
}
