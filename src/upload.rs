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
use std::path::Path;
use std::path::PathBuf;

use protobuf;
use sawtooth_sdk::signing;
use yaml_rust::YamlLoader;

use error::CliError;
use key;
use protos::payload::{SabrePayload, SabrePayload_Action};
use protos::payload::CreateContractAction;
use transaction::{create_batch, create_batch_list_from_one, create_transaction};
use submit::submit_batch_list;

pub fn do_upload(filename: &str, key_name: Option<&str>, url: &str) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let definition = ContractDefinition::load(filename)?;

    // Load the contract file relative to the directory containing the
    // definition YAML
    let mut contract_path_buf = PathBuf::new();
    contract_path_buf.push(filename);
    contract_path_buf.pop();
    contract_path_buf.push(definition.wasm);

    let contract = load_contract_file(contract_path_buf.as_path())?;

    let payload = create_upload_payload(
        &definition.name,
        &definition.version,
        definition.inputs,
        definition.outputs,
        contract,
    );

    let txn = create_transaction(&payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
}

fn create_upload_payload(
    name: &str,
    version: &str,
    inputs: Vec<String>,
    outputs: Vec<String>,
    contract: Vec<u8>,
) -> SabrePayload {
    let mut create_contract = CreateContractAction::new();
    create_contract.set_name(String::from(name));
    create_contract.set_version(String::from(version));
    create_contract.set_inputs(protobuf::RepeatedField::from_vec(inputs));
    create_contract.set_outputs(protobuf::RepeatedField::from_vec(outputs));
    create_contract.set_contract(contract);

    let mut payload = SabrePayload::new();
    payload.action = SabrePayload_Action::CREATE_CONTRACT;
    payload.set_create_contract(create_contract);
    payload
}

fn load_contract_file(path: &Path) -> Result<Vec<u8>, CliError> {
    let file = File::open(path).map_err(|e| {
        CliError::UserError(format!(
            "Could not load contract \"{}\": {}",
            path.display(),
            e
        ))
    })?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader.read_to_end(&mut contents).map_err(|e| {
        CliError::UserError(format!(
            "IoError while reading contract \"{}\": {}",
            path.display(),
            e
        ))
    })?;

    Ok(contents)
}

struct ContractDefinition {
    name: String,
    version: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
    wasm: String,
}

impl ContractDefinition {
    fn load(filename: &str) -> Result<ContractDefinition, CliError> {
        let file = File::open(filename).map_err(|e| {
            CliError::UserError(format!(
                "Could not load contract definition file \"{}\": {}",
                filename, e
            ))
        })?;
        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).map_err(|e| {
            CliError::UserError(format!(
                "IoError while reading contract definition file \"{}\": {}",
                filename, e
            ))
        })?;

        let docs = YamlLoader::load_from_str(&contents).unwrap();
        if docs.is_empty() {
            return Err(CliError::UserError(format!(
                "Malformed contract definition file \"{}\": no content",
                filename
            )));
        }
        let doc = &docs[0];

        let name = doc["name"].as_str().ok_or(CliError::UserError(format!(
            "Malformed contract definition file \"{}\": missing string field \"name\"",
            filename
        )))?;

        let version = doc["version"].as_str().ok_or(CliError::UserError(format!(
            "Malformed contract definition file \"{}\": missing string field \"version\"",
            filename
        )))?;

        let wasm = doc["wasm"].as_str().ok_or(CliError::UserError(format!(
            "Malformed contract definition file \"{}\": missing string field \"wasm\"",
            filename
        )))?;

        let inputs = doc["inputs"]
            .as_vec()
            .ok_or(CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing array \"inputs\"",
                filename
            )))?
            .into_iter()
            .map(|y| {
                y.as_str()
                 .ok_or(CliError::UserError(format!(
                     "Malformed contract definition file: \"{}\": inputs array contains non-string values",
                     filename)))
                 .map(|s| String::from(s))
            })
            .collect::<Result<Vec<_>, CliError>>()?;

        let outputs = doc["outputs"]
            .as_vec()
            .ok_or(CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing array \"outputs\"",
                filename
            )))?
            .into_iter()
            .map(|y| {
                y.as_str()
                 .ok_or(CliError::UserError(format!(
                     "Malformed contract definition file: \"{}\": outputs array contains non-string values",
                     filename)))
                 .map(|s| String::from(s))
            })
            .collect::<Result<Vec<_>, CliError>>()?;

        Ok(ContractDefinition {
            name: String::from(name),
            version: String::from(version),
            inputs: inputs,
            outputs: outputs,
            wasm: String::from(wasm),
        })
    }
}
