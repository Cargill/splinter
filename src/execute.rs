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

use sabre_sdk::protocol::payload::ExecuteContractActionBuilder;

use crate::error::CliError;
use crate::key::new_signer;
use crate::submit::submit_batches;

pub fn do_exec(
    name: &str,
    version: &str,
    payload_file: &str,
    inputs: Vec<String>,
    outputs: Vec<String>,
    key_name: Option<&str>,
    url: &str,
) -> Result<String, CliError> {
    let contract_payload = load_contract_payload_file(payload_file)?;
    let signer = new_signer(key_name)?;
    let batch = ExecuteContractActionBuilder::new()
        .with_name(name.into())
        .with_version(version.into())
        .with_inputs(inputs)
        .with_outputs(outputs)
        .with_payload(contract_payload)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
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
