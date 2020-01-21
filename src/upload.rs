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

use sabre_sdk::protocol::payload::CreateContractActionBuilder;
use sawtooth_sdk::signing;
use yaml_rust::YamlLoader;

use crate::error::CliError;
use crate::key;
use crate::submit::submit_batch_list;
use crate::transaction::{create_batch, create_batch_list_from_one, create_transaction};

pub fn do_upload(
    filename: &str,
    key_name: Option<&str>,
    url: &str,
    wasm_name: Option<&str>,
) -> Result<String, CliError> {
    let private_key = key::load_signing_key(key_name)?;
    let context = signing::create_context("secp256k1")?;
    let public_key = context.get_public_key(&private_key)?.as_hex();
    let factory = signing::CryptoFactory::new(&*context);
    let signer = factory.new_signer(&private_key);

    let definition = ContractDefinition::load(filename)?;

    // Load the contract file relative to the directory containing the
    // definition YAML
    let mut contract_path_buf = PathBuf::new();
    if let Some(path) = wasm_name {
        contract_path_buf.push(path);
    } else if let Some(wasm) = definition.wasm {
        contract_path_buf.push(filename);
        contract_path_buf.pop();
        contract_path_buf.push(wasm);
    } else {
        return Err(CliError::UserError(format!(
            "Malformed contract definition file \"{}\": missing string field \"wasm\" and/or missing --wasm flag",
            filename
        )));
    }

    let contract = load_contract_file(contract_path_buf.as_path())?;

    let payload = CreateContractActionBuilder::new()
        .with_name(definition.name)
        .with_version(definition.version)
        .with_inputs(definition.inputs)
        .with_outputs(definition.outputs)
        .with_contract(contract)
        .into_payload_builder()?
        .build()?;

    let txn = create_transaction(payload, &signer, &public_key)?;
    let batch = create_batch(txn, &signer, &public_key)?;
    let batch_list = create_batch_list_from_one(batch);

    submit_batch_list(url, &batch_list)
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
    wasm: Option<String>,
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

        let name = doc["name"].as_str().ok_or_else(|| {
            CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing string field \"name\"",
                filename
            ))
        })?;

        let version = doc["version"].as_str().ok_or_else(|| {
            CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing string field \"version\"",
                filename
            ))
        })?;

        let wasm = doc["wasm"].as_str().map(ToString::to_string);

        let inputs = doc["inputs"]
            .as_vec()
            .ok_or_else(|| CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing array \"inputs\"",
                filename
            )))?
            .iter()
            .map(|y| {
                y.as_str()
                 .ok_or_else(|| CliError::UserError(format!(
                     "Malformed contract definition file: \"{}\": inputs array contains non-string values",
                     filename)))
                 .map(String::from)
            })
            .collect::<Result<Vec<_>, CliError>>()?;

        let outputs = doc["outputs"]
            .as_vec()
            .ok_or_else(|| CliError::UserError(format!(
                "Malformed contract definition file \"{}\": missing array \"outputs\"",
                filename
            )))?
            .iter()
            .map(|y| {
                y.as_str()
                 .ok_or_else(|| CliError::UserError(format!(
                     "Malformed contract definition file: \"{}\": outputs array contains non-string values",
                     filename)))
                 .map(String::from)
            })
            .collect::<Result<Vec<_>, CliError>>()?;

        Ok(ContractDefinition {
            name: String::from(name),
            version: String::from(version),
            inputs,
            outputs,
            wasm,
        })
    }
}
