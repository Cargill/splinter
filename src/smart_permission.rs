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
    CreateSmartPermissionActionBuilder, DeleteSmartPermissionActionBuilder,
    UpdateSmartPermissionActionBuilder,
};

use crate::error::CliError;
use crate::key::new_signer;
use crate::submit::submit_batches;

pub fn do_create(
    url: &str,
    org_id: &str,
    name: &str,
    filename: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let signer = new_signer(key)?;
    let batch = CreateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(function)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}

pub fn do_update(
    url: &str,
    org_id: &str,
    name: &str,
    filename: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let mut smart_permission_path_buf = PathBuf::new();
    smart_permission_path_buf.push(filename);

    let function = load_smart_permission_file(smart_permission_path_buf.as_path())?;

    let signer = new_signer(key)?;
    let batch = UpdateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(function)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}

pub fn do_delete(
    url: &str,
    org_id: &str,
    name: &str,
    key: Option<&str>,
) -> Result<String, CliError> {
    let signer = new_signer(key)?;
    let batch = DeleteSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
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
