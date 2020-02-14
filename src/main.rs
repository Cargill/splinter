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

#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;

mod error;
mod key;
mod submit;
mod upload;

use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use std::time::Instant;

use sabre_sdk::protocol::payload::{
    CreateContractRegistryActionBuilder, CreateNamespaceRegistryActionBuilder,
    CreateNamespaceRegistryPermissionActionBuilder, CreateSmartPermissionActionBuilder,
    DeleteContractRegistryActionBuilder, DeleteNamespaceRegistryActionBuilder,
    DeleteNamespaceRegistryPermissionActionBuilder, DeleteSmartPermissionActionBuilder,
    ExecuteContractActionBuilder, UpdateContractRegistryOwnersActionBuilder,
    UpdateNamespaceRegistryOwnersActionBuilder, UpdateSmartPermissionActionBuilder,
};

use error::CliError;
use key::new_signer;
use submit::submit_batches;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn run() -> Result<(), CliError> {
    // Below, unwrap() is used on required arguments, since they will always
    // contain a value (and lack of value is should cause a panic). unwrap()
    // is also used on get_matches() because SubcommandRequiredElseHelp will
    // ensure a value.

    let matches = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (about: "Sawtooth Sabre CLI")
        (@setting SubcommandRequiredElseHelp)
        (@subcommand upload =>
            (about: "upload a Sabre contract")
            (@arg filename: -f --filename +required +takes_value "Path to Sabre contract definition (*.yaml)")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: --url +takes_value "URL to the Sawtooth REST API")
            (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
            (@arg wasm: -w --wasm +takes_value "Path to compiled smart contract (*.wasm)")
        )
        (@subcommand exec =>
            (about: "execute a Sabre contract")
            (@arg contract: -C --contract +required +takes_value "Name:Version of a Sabre contract")
            (@arg payload: -p --payload +required +takes_value "Path to Sabre contract payload")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: --url +takes_value "URL to the Sawtooth REST API")
            (@arg inputs: --inputs +takes_value +multiple "Input addresses used by the contract")
            (@arg outputs: --outputs +takes_value +multiple "Output addresses used by the contract")
            (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
        )
        (@subcommand ns =>
            (about: "create, update, or delete a Sabre namespace")
            (@group action =>
                (@arg create: -c --create "Create the namespace")
                (@arg update: -u --update "Update the namespace")
                (@arg delete: -d --delete "Delete the namespace")
            )
            (@arg namespace: +required "A global state address prefix (namespace)")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: -U --url +takes_value "URL to the Sawtooth REST API")
            (@arg owner: -O --owner +takes_value +multiple "Owner of this namespace")
            (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
        )
        (@subcommand perm =>
            (about: "set or delete a Sabre namespace permission")
            (@arg namespace: +required "A global state address prefix (namespace)")
            (@arg contract: +required "Name of the contract")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: -U --url +takes_value "URL to the Sawtooth REST API")
            (@arg delete: -d --delete "Remove all permissions")
            (@arg read: -r --read conflicts_with[delete] "Set read permission")
            (@arg write: -w --write conflicts_with[delete] "Set write permission")
            (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
        )
        (@subcommand cr =>
            (about: "create, update, or delete a Sabre contract registry")
            (@group action =>
                (@arg create: -c --create "Create the contract registry")
                (@arg update: -u --update "Update the contract registry")
                (@arg delete: -d --delete "Delete the contract registry")
            )
            (@arg name: +required "Name of the contracts in the registry")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: -U --url +takes_value "URL to the Sawtooth REST API")
            (@arg owner: -O --owner +takes_value +multiple "Owner of this contract registry")
            (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
        )
        (@subcommand sp =>
          (about: "Create, update or delete smart permissions")
          (@arg url: -U --url +takes_value "URL to the Sawtooth REST API")
          (@arg wait: --wait +takes_value "A time in seconds to wait for batches to be committed")
          (@subcommand create =>
                (@arg org_id: +required "Organization ID ")
                (@arg name: +required "Name of the Smart Permission")
                (@arg filename: -f --filename +required +takes_value "Path to smart_permission")
                (@arg key: -k --key +takes_value "Signing key name")
            )
            (@subcommand update =>
                (@arg org_id: +required "Organization IDs")
                (@arg name: +required "Name of the Smart Permission")
                (@arg filename: -f --filename +required +takes_value "Path to smart_permission")
                (@arg key: -k --key +takes_value "Signing key name")
            )
            (@subcommand delete =>
                (@arg org_id: +required "Organization IDs")
                (@arg name: +required "Name of the Smart Permission")
                (@arg key: -k --key +takes_value "Signing key name")
            )
        )
    ).get_matches();

    let (batch_link, mut wait) = if let Some(upload_matches) = matches.subcommand_matches("upload")
    {
        upload(upload_matches)?
    } else if let Some(exec_matches) = matches.subcommand_matches("exec") {
        execute(exec_matches)?
    } else if let Some(ns_matches) = matches.subcommand_matches("ns") {
        namespace_registry(ns_matches)?
    } else if let Some(perm_matches) = matches.subcommand_matches("perm") {
        namespace_permission(perm_matches)?
    } else if let Some(cr_matches) = matches.subcommand_matches("cr") {
        contract_registry(cr_matches)?
    } else if let Some(sp_matches) = matches.subcommand_matches("sp") {
        smart_permission(sp_matches)?
    } else {
        return Err(CliError::UserError("Subcommand required".into()));
    };

    if wait > 0 {
        let response_body = loop {
            let time = Instant::now();
            let status_response = submit::wait_for_batch(&batch_link, wait)?;

            wait = wait.saturating_sub(time.elapsed().as_secs());

            if wait == 0 || status_response.is_finished() {
                break status_response;
            }
        };

        println!("Response Body:\n{}", response_body);
    }

    Ok(())
}

fn upload(upload_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let filename = upload_matches.value_of("filename").unwrap();
    let key_name = upload_matches.value_of("key");
    let url = upload_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");
    let wasm_name = upload_matches.value_of("wasm");

    let wait = match value_t!(upload_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::UserError("Wait must be an integer".into())),
        },
    };

    let batch_link = upload::do_upload(&filename, key_name, &url, wasm_name)?;
    Ok((batch_link, wait))
}

fn execute(exec_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let contract = exec_matches.value_of("contract").unwrap();
    let payload = exec_matches.value_of("payload").unwrap();
    let key_name = exec_matches.value_of("key");
    let url = exec_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");

    let wait = match value_t!(exec_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::UserError("Wait must be an integer".into())),
        },
    };

    let inputs = exec_matches
        .values_of("inputs")
        .map(|values| values.map(|v| v.into()).collect())
        .ok_or_else(|| {
            CliError::UserError("exec action requires one or more --inputs arguments".into())
        })?;

    let outputs = exec_matches
        .values_of("outputs")
        .map(|values| values.map(|v| v.into()).collect())
        .ok_or_else(|| {
            CliError::UserError("exec action requires one or more --outputs arguments".into())
        })?;
    let (name, version) = match contract.split(':').collect::<Vec<_>>() {
        ref v if (v.len() == 1 || v.len() == 2) && v[0].is_empty() => Err(CliError::UserError(
            "contract name must be specified".into(),
        )),
        ref v if v.len() == 1 || v.len() == 2 && v[1].is_empty() => Ok((v[0], "latest")),
        ref v if v.len() == 2 => Ok((v[0], v[1])),
        _ => Err(CliError::UserError(
            "malformed contract argument, may contain at most one ':'".into(),
        )),
    }?;

    let contract_payload = load_bytes_from_file(payload)?;
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

    let batch_link = submit_batches(&url, vec![batch])?;

    Ok((batch_link, wait))
}

fn namespace_registry(ns_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let namespace = ns_matches.value_of("namespace").unwrap();

    let key_name = ns_matches.value_of("key");

    let url = ns_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");

    let wait = match value_t!(ns_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::UserError("Wait must be an integer".into())),
        },
    };

    let signer = new_signer(key_name)?;

    let owners = ns_matches
        .values_of("owner")
        .map(|values| values.map(|v| v.into()).collect());

    let batch_link = if ns_matches.is_present("update") {
        let owners = owners.ok_or_else(|| {
            CliError::UserError("update action requires one or more --owner arguments".into())
        })?;

        let batch = UpdateNamespaceRegistryOwnersActionBuilder::new()
            .with_namespace(namespace.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    } else if ns_matches.is_present("delete") {
        if ns_matches.is_present("owner") {
            return Err(CliError::UserError(
                "arguments --delete and --owner conflict".into(),
            ));
        }

        let batch = DeleteNamespaceRegistryActionBuilder::new()
            .with_namespace(namespace.into())
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    } else {
        let owners = owners.ok_or_else(|| {
            CliError::UserError("create action requires one or more --owner arguments".into())
        })?;

        let batch = CreateNamespaceRegistryActionBuilder::new()
            .with_namespace(namespace.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    };

    Ok((batch_link, wait))
}

fn namespace_permission(perm_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let namespace = perm_matches.value_of("namespace").unwrap();
    let contract = perm_matches.value_of("contract").unwrap();
    let key_name = perm_matches.value_of("key");
    let url = perm_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");

    let wait = match value_t!(perm_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::UserError("Wait must be an integer".into())),
        },
    };

    let signer = new_signer(key_name)?;

    let batch_link = if perm_matches.is_present("delete") {
        let batch = DeleteNamespaceRegistryPermissionActionBuilder::new()
            .with_namespace(namespace.into())
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    } else {
        let read = perm_matches.is_present("read");
        let write = perm_matches.is_present("write");

        if !(read || write) {
            return Err(CliError::UserError("no permissions provided".into()));
        }

        let batch = CreateNamespaceRegistryPermissionActionBuilder::new()
            .with_namespace(namespace.into())
            .with_contract_name(contract.into())
            .with_read(read)
            .with_write(write)
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    };

    Ok((batch_link, wait))
}

fn contract_registry(cr_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let name = cr_matches.value_of("name").unwrap();

    let key_name = cr_matches.value_of("key");

    let url = cr_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");

    let wait = value_t!(cr_matches, "wait", u64).unwrap_or(0);

    let signer = new_signer(key_name)?;

    let owners = cr_matches
        .values_of("owner")
        .map(|values| values.map(|v| v.into()).collect());

    let batch_link = if cr_matches.is_present("update") {
        let owners = owners.ok_or_else(|| {
            CliError::UserError("update action requires one or more --owner arguments".into())
        })?;

        let batch = UpdateContractRegistryOwnersActionBuilder::new()
            .with_name(name.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    } else if cr_matches.is_present("delete") {
        if cr_matches.is_present("owner") {
            return Err(CliError::UserError(
                "arguments --delete and --owner conflict".into(),
            ));
        }

        let batch = DeleteContractRegistryActionBuilder::new()
            .with_name(name.into())
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    } else {
        let owners = owners.ok_or_else(|| {
            CliError::UserError("create action requires one or more --owner arguments".into())
        })?;

        let batch = CreateContractRegistryActionBuilder::new()
            .with_name(name.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder(&signer)?
            .into_batch_builder(&signer)?
            .build(&signer)?;

        submit_batches(&url, vec![batch])?
    };
    Ok((batch_link, wait))
}

fn smart_permission(sp_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let url = sp_matches
        .value_of("url")
        .unwrap_or("http://localhost:8008/");

    let wait = match value_t!(sp_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::UserError("Wait must be an integer".into())),
        },
    };

    let batch_link = match sp_matches.subcommand() {
        ("create", Some(m)) => {
            let org_id = m.value_of("org_id").unwrap();
            let name = m.value_of("name").unwrap();
            let filename = m.value_of("filename").unwrap();
            let key = m.value_of("key");

            let function = load_bytes_from_file(filename)?;

            let signer = new_signer(key)?;
            let batch = CreateSmartPermissionActionBuilder::new()
                .with_name(name.into())
                .with_org_id(org_id.into())
                .with_function(function)
                .into_payload_builder()?
                .into_transaction_builder(&signer)?
                .into_batch_builder(&signer)?
                .build(&signer)?;

            submit_batches(&url, vec![batch])?
        }
        ("update", Some(m)) => {
            let org_id = m.value_of("org_id").unwrap();
            let name = m.value_of("name").unwrap();
            let filename = m.value_of("filename").unwrap();
            let key = m.value_of("key");

            let function = load_bytes_from_file(filename)?;

            let signer = new_signer(key)?;
            let batch = UpdateSmartPermissionActionBuilder::new()
                .with_name(name.to_string())
                .with_org_id(org_id.to_string())
                .with_function(function)
                .into_payload_builder()?
                .into_transaction_builder(&signer)?
                .into_batch_builder(&signer)?
                .build(&signer)?;

            submit_batches(&url, vec![batch])?
        }
        ("delete", Some(m)) => {
            let org_id = m.value_of("org_id").unwrap();
            let name = m.value_of("name").unwrap();
            let key = m.value_of("key");

            let signer = new_signer(key)?;
            let batch = DeleteSmartPermissionActionBuilder::new()
                .with_name(name.to_string())
                .with_org_id(org_id.to_string())
                .into_payload_builder()?
                .into_transaction_builder(&signer)?
                .into_batch_builder(&signer)?
                .build(&signer)?;

            submit_batches(&url, vec![batch])?
        }
        _ => {
            return Err(CliError::UserError(
                "Unrecognized smart permission subcommand".into(),
            ));
        }
    };

    Ok((batch_link, wait))
}

fn load_bytes_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, CliError> {
    let file = File::open(&path).map_err(|e| {
        CliError::UserError(format!(
            "Failed to open file \"{}\": {}",
            path.as_ref().display(),
            e
        ))
    })?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader.read_to_end(&mut contents).map_err(|e| {
        CliError::UserError(format!(
            "IoError while reading file \"{}\": {}",
            path.as_ref().display(),
            e
        ))
    })?;

    Ok(contents)
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}
