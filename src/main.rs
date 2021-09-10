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
mod state;
mod submit;
mod upload;

use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::Path;
use std::time::Instant;

use clap::{AppSettings, Arg, SubCommand};
use sabre_sdk::protocol::payload::{
    CreateContractRegistryActionBuilder, CreateNamespaceRegistryActionBuilder,
    CreateNamespaceRegistryPermissionActionBuilder, DeleteContractRegistryActionBuilder,
    DeleteNamespaceRegistryActionBuilder, DeleteNamespaceRegistryPermissionActionBuilder,
    ExecuteContractActionBuilder, UpdateContractRegistryOwnersActionBuilder,
    UpdateNamespaceRegistryOwnersActionBuilder,
};
use sabre_sdk::protocol::{
    compute_contract_address,
    state::{ContractList, ContractRegistryList},
    CONTRACT_REGISTRY_ADDRESS_PREFIX,
};
use sabre_sdk::protos::FromBytes;

use error::CliError;
use key::new_signer;
use submit::submit_batches;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_REST_API_ENDPOINT: &str = "http://localhost:8008/";

fn run() -> Result<(), CliError> {
    // Below, unwrap() is used on required arguments, since they will always
    // contain a value (and lack of value is should cause a panic). unwrap()
    // is also used on get_matches() because SubcommandRequiredElseHelp will
    // ensure a value.

    let app = clap_app!(myapp =>
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
    );

    let app = app.subcommand(
        SubCommand::with_name("contract")
            .about("List or show a Sabre smart contract")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("list")
                    .about("List all registered Sabre smart contracts")
                    .args(&[
                        Arg::with_name("url")
                            .help("URL to the Sawtooth REST API")
                            .short("U")
                            .long("url")
                            .takes_value(true),
                        Arg::with_name("format")
                            .help("Format to display list of smart contracts in")
                            .short("f")
                            .long("format")
                            .takes_value(true)
                            .possible_values(&["human", "csv"])
                            .default_value("human"),
                    ]),
            )
            .subcommand(
                SubCommand::with_name("show")
                    .about("Show details about a registered Sabre smart contract")
                    .args(&[
                        Arg::with_name("url")
                            .help("URL to the Sawtooth REST API")
                            .short("U")
                            .long("url")
                            .takes_value(true),
                        Arg::with_name("contract")
                            .help(
                                "Name and version of the smart contract in the form \
                                     'name:version'",
                            )
                            .takes_value(true)
                            .required(true),
                    ]),
            ),
    );

    let matches = app.get_matches();

    if let Some(contract_matches) = matches.subcommand_matches("contract") {
        contract(contract_matches)?
    } else {
        let (batch_link, mut wait) =
            if let Some(upload_matches) = matches.subcommand_matches("upload") {
                upload(upload_matches)?
            } else if let Some(exec_matches) = matches.subcommand_matches("exec") {
                execute(exec_matches)?
            } else if let Some(ns_matches) = matches.subcommand_matches("ns") {
                namespace_registry(ns_matches)?
            } else if let Some(perm_matches) = matches.subcommand_matches("perm") {
                namespace_permission(perm_matches)?
            } else if let Some(cr_matches) = matches.subcommand_matches("cr") {
                contract_registry(cr_matches)?
            } else {
                return Err(CliError::User("Subcommand required".into()));
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
    }

    Ok(())
}

fn upload(upload_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let filename = upload_matches.value_of("filename").unwrap();
    let key_name = upload_matches.value_of("key");
    let url = upload_matches
        .value_of("url")
        .unwrap_or(DEFAULT_REST_API_ENDPOINT);
    let wasm_name = upload_matches.value_of("wasm");

    let wait = match value_t!(upload_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::User("Wait must be an integer".into())),
        },
    };

    let batch_link = upload::do_upload(filename, key_name, url, wasm_name)?;
    Ok((batch_link, wait))
}

fn execute(exec_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let contract = exec_matches.value_of("contract").unwrap();
    let payload = exec_matches.value_of("payload").unwrap();
    let key_name = exec_matches.value_of("key");
    let url = exec_matches
        .value_of("url")
        .unwrap_or(DEFAULT_REST_API_ENDPOINT);

    let wait = match value_t!(exec_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::User("Wait must be an integer".into())),
        },
    };

    let inputs = exec_matches
        .values_of("inputs")
        .map(|values| values.map(|v| v.into()).collect())
        .ok_or_else(|| {
            CliError::User("exec action requires one or more --inputs arguments".into())
        })?;

    let outputs = exec_matches
        .values_of("outputs")
        .map(|values| values.map(|v| v.into()).collect())
        .ok_or_else(|| {
            CliError::User("exec action requires one or more --outputs arguments".into())
        })?;
    let (name, version) = match contract.split(':').collect::<Vec<_>>() {
        ref v if (v.len() == 1 || v.len() == 2) && v[0].is_empty() => {
            Err(CliError::User("contract name must be specified".into()))
        }
        ref v if v.len() == 1 || v.len() == 2 && v[1].is_empty() => Ok((v[0], "latest")),
        ref v if v.len() == 2 => Ok((v[0], v[1])),
        _ => Err(CliError::User(
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
        .into_transaction_builder()?
        .into_batch_builder(&*signer)?
        .build(&*signer)?;

    let batch_link = submit_batches(url, vec![batch])?;

    Ok((batch_link, wait))
}

fn namespace_registry(ns_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let namespace = ns_matches.value_of("namespace").unwrap();

    let key_name = ns_matches.value_of("key");

    let url = ns_matches
        .value_of("url")
        .unwrap_or(DEFAULT_REST_API_ENDPOINT);

    let wait = match value_t!(ns_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::User("Wait must be an integer".into())),
        },
    };

    let signer = new_signer(key_name)?;

    let owners = ns_matches
        .values_of("owner")
        .map(|values| values.map(|v| v.into()).collect());

    let batch_link = if ns_matches.is_present("update") {
        let owners = owners.ok_or_else(|| {
            CliError::User("update action requires one or more --owner arguments".into())
        })?;

        let batch = UpdateNamespaceRegistryOwnersActionBuilder::new()
            .with_namespace(namespace.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    } else if ns_matches.is_present("delete") {
        if ns_matches.is_present("owner") {
            return Err(CliError::User(
                "arguments --delete and --owner conflict".into(),
            ));
        }

        let batch = DeleteNamespaceRegistryActionBuilder::new()
            .with_namespace(namespace.into())
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    } else {
        let owners = owners.ok_or_else(|| {
            CliError::User("create action requires one or more --owner arguments".into())
        })?;

        let batch = CreateNamespaceRegistryActionBuilder::new()
            .with_namespace(namespace.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    };

    Ok((batch_link, wait))
}

fn namespace_permission(perm_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let namespace = perm_matches.value_of("namespace").unwrap();
    let contract = perm_matches.value_of("contract").unwrap();
    let key_name = perm_matches.value_of("key");
    let url = perm_matches
        .value_of("url")
        .unwrap_or(DEFAULT_REST_API_ENDPOINT);

    let wait = match value_t!(perm_matches, "wait", u64) {
        Ok(wait) => wait,
        Err(err) => match err.kind {
            clap::ErrorKind::ArgumentNotFound => 0,
            _ => return Err(CliError::User("Wait must be an integer".into())),
        },
    };

    let signer = new_signer(key_name)?;

    let batch_link = if perm_matches.is_present("delete") {
        let batch = DeleteNamespaceRegistryPermissionActionBuilder::new()
            .with_namespace(namespace.into())
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    } else {
        let read = perm_matches.is_present("read");
        let write = perm_matches.is_present("write");

        if !(read || write) {
            return Err(CliError::User("no permissions provided".into()));
        }

        let batch = CreateNamespaceRegistryPermissionActionBuilder::new()
            .with_namespace(namespace.into())
            .with_contract_name(contract.into())
            .with_read(read)
            .with_write(write)
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    };

    Ok((batch_link, wait))
}

fn contract_registry(cr_matches: &clap::ArgMatches) -> Result<(String, u64), CliError> {
    let name = cr_matches.value_of("name").unwrap();

    let key_name = cr_matches.value_of("key");

    let url = cr_matches
        .value_of("url")
        .unwrap_or(DEFAULT_REST_API_ENDPOINT);

    let wait = value_t!(cr_matches, "wait", u64).unwrap_or(0);

    let signer = new_signer(key_name)?;

    let owners = cr_matches
        .values_of("owner")
        .map(|values| values.map(|v| v.into()).collect());

    let batch_link = if cr_matches.is_present("update") {
        let owners = owners.ok_or_else(|| {
            CliError::User("update action requires one or more --owner arguments".into())
        })?;

        let batch = UpdateContractRegistryOwnersActionBuilder::new()
            .with_name(name.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    } else if cr_matches.is_present("delete") {
        if cr_matches.is_present("owner") {
            return Err(CliError::User(
                "arguments --delete and --owner conflict".into(),
            ));
        }

        let batch = DeleteContractRegistryActionBuilder::new()
            .with_name(name.into())
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    } else {
        let owners = owners.ok_or_else(|| {
            CliError::User("create action requires one or more --owner arguments".into())
        })?;

        let batch = CreateContractRegistryActionBuilder::new()
            .with_name(name.into())
            .with_owners(owners)
            .into_payload_builder()?
            .into_transaction_builder()?
            .into_batch_builder(&*signer)?
            .build(&*signer)?;

        submit_batches(url, vec![batch])?
    };
    Ok((batch_link, wait))
}

fn contract(contract_matches: &clap::ArgMatches) -> Result<(), CliError> {
    match contract_matches.subcommand() {
        ("list", Some(matches)) => {
            let url = matches.value_of("url").unwrap_or(DEFAULT_REST_API_ENDPOINT);

            let format = matches
                .value_of("format")
                .expect("default not set for --format");

            let registries = state::get_state_with_prefix(url, CONTRACT_REGISTRY_ADDRESS_PREFIX)?
                .into_iter()
                .map(|entry| {
                    base64::decode(entry.data)
                        .map_err(|_| CliError::User("Unable to decode state".into()))
                        .and_then(|bytes| {
                            ContractRegistryList::from_bytes(&bytes)
                                .map_err(CliError::ProtoConversion)
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let mut data = vec![
                // Headers
                vec![
                    "NAME".to_string(),
                    "VERSIONS".to_string(),
                    "OWNERS".to_string(),
                ],
            ];
            for registry_list in registries {
                for registry in registry_list.registries() {
                    let name = registry.name().to_string();
                    let versions = registry
                        .versions()
                        .iter()
                        .map(|version| version.version().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let owners = registry.owners().join(", ");

                    data.push(vec![name, versions, owners]);
                }
            }

            if format == "csv" {
                for row in data {
                    println!("{}", row.join(","))
                }
            } else {
                print_table(data);
            }

            Ok(())
        }
        ("show", Some(matches)) => {
            let url = matches.value_of("url").unwrap_or(DEFAULT_REST_API_ENDPOINT);

            let contract = matches
                .value_of("contract")
                .ok_or_else(|| CliError::User("Missing contract".into()))?;

            let (name, version) = parse_name_version(contract).ok_or_else(|| {
                CliError::User("--contract must be of the form 'name:version'".into())
            })?;

            let address = to_hex(&compute_contract_address(name, version).map_err(|err| {
                CliError::User(format!("Unable to get contract address: {}", err))
            })?);

            let contract_bytes = state::get_state_with_prefix(url, &address)?
                .get(0)
                .cloned()
                .ok_or_else(|| CliError::User(format!("contract '{}' not found", contract)))?;

            let contract_list = ContractList::from_bytes(
                &base64::decode(contract_bytes.data)
                    .map_err(|_| CliError::User("Unable to decode state".into()))?,
            )?;
            let contract = contract_list
                .contracts()
                .get(0)
                .ok_or_else(|| CliError::User("contract list is empty".into()))?;

            println!("{} {}", contract.name(), contract.version());
            println!("  inputs:");
            for input in contract.inputs() {
                println!("  - {}", input);
            }
            println!("  outputs:");
            for output in contract.outputs() {
                println!("  - {}", output);
            }
            println!("  creator: {}", contract.creator());

            Ok(())
        }
        _ => Err(CliError::User("Invalid Subcommand".into())),
    }
}

// Takes a vec of vecs of strings. The first vec should include the title of the columns.
// The max length of each column is calculated and is used as the column with when printing the
// table.
fn print_table(table: Vec<Vec<String>>) {
    let mut max_lengths = Vec::new();

    // find the max lengths of the columns
    for row in table.iter() {
        for (i, col) in row.iter().enumerate() {
            if let Some(length) = max_lengths.get_mut(i) {
                if col.len() > *length {
                    *length = col.len()
                }
            } else {
                max_lengths.push(col.len())
            }
        }
    }

    // print each row with correct column size
    for row in table.iter() {
        let mut col_string = String::from("");
        for (i, len) in max_lengths.iter().enumerate() {
            if let Some(value) = row.get(i) {
                col_string += &format!("{}{} ", value, " ".repeat(*len - value.len()),);
            } else {
                col_string += &" ".repeat(*len);
            }
        }
        println!("{}", col_string);
    }
}

/// Attempts to parse the given string as "name:version" and return the two values.
fn parse_name_version(name_version_string: &str) -> Option<(&str, &str)> {
    match name_version_string.splitn(2, ':').collect::<Vec<_>>() {
        v if v.len() == 2 => Some((v[0], v[1])),
        _ => None,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

fn load_bytes_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, CliError> {
    let file = File::open(&path).map_err(|e| {
        CliError::User(format!(
            "Failed to open file \"{}\": {}",
            path.as_ref().display(),
            e
        ))
    })?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader.read_to_end(&mut contents).map_err(|e| {
        CliError::User(format!(
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
