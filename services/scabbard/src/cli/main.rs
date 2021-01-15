// Copyright 2018-2021 Cargill Incorporated
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
extern crate log;

mod error;
mod key;
mod transaction;

use std::fs::File;
use std::io::{BufReader, Read};

#[cfg(any(
    feature = "contract",
    feature = "execute",
    feature = "namespace",
    feature = "namespace-permission",
    feature = "contract-registry",
    feature = "smart-permissions"
))]
use clap::SubCommand;
use clap::{App, AppSettings, Arg};
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;
use sabre_sdk::{
    protocol::{
        payload::{
            Action, CreateContractActionBuilder, CreateContractRegistryActionBuilder,
            CreateNamespaceRegistryActionBuilder, CreateNamespaceRegistryPermissionActionBuilder,
            CreateSmartPermissionActionBuilder, DeleteContractRegistryActionBuilder,
            DeleteNamespaceRegistryActionBuilder, DeleteNamespaceRegistryPermissionActionBuilder,
            DeleteSmartPermissionActionBuilder, ExecuteContractActionBuilder, SabrePayloadBuilder,
            UpdateContractRegistryOwnersActionBuilder, UpdateNamespaceRegistryOwnersActionBuilder,
            UpdateSmartPermissionActionBuilder,
        },
        state::{ContractList, ContractRegistryList},
    },
    protos::FromBytes,
};
use sawtooth_sdk::signing::secp256k1::Secp256k1Context;
use splinter::{
    service::scabbard::client::{SabreSmartContractDefinition, ScabbardClient, ServiceId},
    signing::sawtooth::SawtoothSecp256k1RefSigner,
};

use error::CliError;

fn main() {
    if let Err(e) = run() {
        error!("ERROR: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let mut app = App::new("scabbard");

    app = app
        .version(env!("CARGO_PKG_VERSION"))
        .author("Cargill")
        .about("Command line for scabbard")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("verbose")
                .help("Log verbosely")
                .short("v")
                .global(true)
                .multiple(true),
        );

    #[cfg(feature = "contract")]
    {
        app = app.subcommand(
            SubCommand::with_name("contract")
                .about("List, show, or upload a Sabre smart contract")
                .subcommand(
                    SubCommand::with_name("upload")
                        .about("Upload a Sabre contract")
                        .args(&[
                            Arg::with_name("scar")
                                .long_help(
                                    "The .scar to upload (either a file path or the name of a \
                                     .scar in SCAR_PATH)",
                                )
                                .required(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List all registered Sabre smart contracts")
                        .args(&[
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
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
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("contract")
                                .help(
                                    "Name and version of the smart contract in the form \
                                     'name:verion'",
                                )
                                .takes_value(true)
                                .required(true),
                        ]),
                ),
        );
    }

    #[cfg(feature = "execute")]
    {
        app = app.subcommand(
            SubCommand::with_name("exec")
                .about("Execute a Sabre contract")
                .args(&[
                    Arg::with_name("contract")
                        .help("Name:version of a Sabre contract")
                        .short("C")
                        .long("contract")
                        .required(true)
                        .takes_value(true),
                    Arg::with_name("payload")
                        .help("Path to Sabre contract payload")
                        .short("p")
                        .long("payload")
                        .required(true)
                        .takes_value(true),
                    Arg::with_name("inputs")
                        .help("Input addresses used by the contract")
                        .long("inputs")
                        .required(true)
                        .takes_value(true)
                        .multiple(true),
                    Arg::with_name("outputs")
                        .help("Output addresses used by the contract")
                        .long("outputs")
                        .required(true)
                        .takes_value(true)
                        .multiple(true),
                    Arg::with_name("key")
                        .long_help(
                            "Key for signing transactions (either a file path or the name of a \
                             .priv file in $HOME/.splinter/keys)",
                        )
                        .short("k")
                        .long("key")
                        .required(true)
                        .takes_value(true),
                    Arg::with_name("url")
                        .help("URL to the scabbard REST API")
                        .short("U")
                        .long("url")
                        .takes_value(true)
                        .default_value("http://localhost:8008"),
                    Arg::with_name("service-id")
                        .long_help(
                            "Fully-qualified service ID of the scabbard service (must be of the \
                             form 'circuit_id::service_id')",
                        )
                        .long("service-id")
                        .takes_value(true)
                        .required(true),
                    Arg::with_name("wait")
                        .help("Time (in seconds) to wait for batches to be committed")
                        .long("wait")
                        .takes_value(true)
                        .default_value("300"),
                ]),
        );
    }

    #[cfg(feature = "namespace")]
    {
        app = app.subcommand(
            SubCommand::with_name("ns")
                .about("Create, update, or delete a Sabre namespace")
                .subcommand(
                    SubCommand::with_name("create")
                        .about("Create a Sabre namespace")
                        .args(&[
                            Arg::with_name("namespace")
                                .help("A global state address prefix (namespace)")
                                .required(true),
                            Arg::with_name("owner")
                                .help("Owner of this namespace")
                                .short("O")
                                .long("owner")
                                .required(true)
                                .takes_value(true)
                                .multiple(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("update")
                        .about("Update an existing Sabre namespace")
                        .args(&[
                            Arg::with_name("namespace")
                                .help("A global state address prefix (namespace)")
                                .required(true),
                            Arg::with_name("owner")
                                .help("Owner of this namespace")
                                .short("O")
                                .long("owner")
                                .required(true)
                                .takes_value(true)
                                .multiple(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("delete")
                        .about("Delete a Sabre namespace")
                        .args(&[
                            Arg::with_name("namespace")
                                .help("A global state address prefix (namespace)")
                                .required(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                ),
        );
    }

    #[cfg(feature = "namespace-permission")]
    {
        app = app.subcommand(
            SubCommand::with_name("perm")
                .about("Set or delete a Sabre namespace permission")
                .args(&[
                    Arg::with_name("namespace")
                        .help("A global state address prefix (namespace)")
                        .required(true),
                    Arg::with_name("contract")
                        .help("Name of the contract")
                        .required(true)
                        .conflicts_with("delete"),
                    Arg::with_name("read")
                        .help("Set read permission")
                        .short("r")
                        .long("read")
                        .conflicts_with("delete"),
                    Arg::with_name("write")
                        .help("Set write permission")
                        .short("w")
                        .long("write")
                        .conflicts_with("delete"),
                    Arg::with_name("delete")
                        .help("Remove all permissions")
                        .short("d")
                        .long("delete"),
                    Arg::with_name("key")
                        .long_help(
                            "Key for signing transactions (either a file path or the name of a \
                             .priv file in $HOME/.splinter/keys)",
                        )
                        .short("k")
                        .long("key")
                        .required(true)
                        .takes_value(true),
                    Arg::with_name("url")
                        .help("URL to the scabbard REST API")
                        .short("U")
                        .long("url")
                        .takes_value(true)
                        .default_value("http://localhost:8008"),
                    Arg::with_name("service-id")
                        .long_help(
                            "Fully-qualified service ID of the scabbard service (must be of the \
                             form 'circuit_id::service_id')",
                        )
                        .long("service-id")
                        .takes_value(true)
                        .required(true),
                    Arg::with_name("wait")
                        .help("Time (in seconds) to wait for batches to be committed")
                        .long("wait")
                        .takes_value(true)
                        .default_value("300"),
                ]),
        );
    }

    #[cfg(feature = "contract-registry")]
    {
        app = app.subcommand(
            SubCommand::with_name("cr")
                .about("Create, update, or delete a Sabre contract registry")
                .subcommand(
                    SubCommand::with_name("create")
                        .about("Create a Sabre contract registry")
                        .args(&[
                            Arg::with_name("name")
                                .help("Name of the contracts in the registry")
                                .required(true),
                            Arg::with_name("owner")
                                .help("Owner of this contract registry")
                                .short("O")
                                .long("owner")
                                .required(true)
                                .takes_value(true)
                                .multiple(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("update")
                        .about("Update an existing Sabre contract registry")
                        .args(&[
                            Arg::with_name("name")
                                .help("Name of the contracts in the registry")
                                .required(true),
                            Arg::with_name("owner")
                                .help("Owner of this contract registry")
                                .short("O")
                                .long("owner")
                                .required(true)
                                .takes_value(true)
                                .multiple(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("delete")
                        .about("Delete a Sabre contract registry")
                        .args(&[
                            Arg::with_name("name")
                                .help("name of the contracts in the registry")
                                .required(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                ),
        );
    }

    #[cfg(feature = "smart-permissions")]
    {
        app = app.subcommand(
            SubCommand::with_name("sp")
                .about("Create, update or delete smart permissions")
                .subcommand(
                    SubCommand::with_name("create")
                        .about("Create a smart permission")
                        .args(&[
                            Arg::with_name("org_id")
                                .help("Organization ID")
                                .required(true),
                            Arg::with_name("name")
                                .help("Name of the smart permission")
                                .required(true),
                            Arg::with_name("filename")
                                .help("Path to smart permission")
                                .short("f")
                                .long("filename")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("update")
                        .about("Update an existing a smart permission")
                        .args(&[
                            Arg::with_name("org_id")
                                .help("Organization ID")
                                .required(true),
                            Arg::with_name("name")
                                .help("Name of the smart permission")
                                .required(true),
                            Arg::with_name("filename")
                                .help("Path to smart permission")
                                .short("f")
                                .long("filename")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                )
                .subcommand(
                    SubCommand::with_name("delete")
                        .about("Delete a smart permission")
                        .args(&[
                            Arg::with_name("org_id")
                                .help("Organization ID")
                                .required(true),
                            Arg::with_name("name")
                                .help("Name of the smart permission")
                                .required(true),
                            Arg::with_name("key")
                                .long_help(
                                    "Key for signing transactions (either a file path or the name \
                                     of a .priv file in $HOME/.splinter/keys)",
                                )
                                .short("k")
                                .long("key")
                                .required(true)
                                .takes_value(true),
                            Arg::with_name("url")
                                .help("URL to the scabbard REST API")
                                .short("U")
                                .long("url")
                                .takes_value(true)
                                .default_value("http://localhost:8008"),
                            Arg::with_name("service-id")
                                .long_help(
                                    "Fully-qualified service ID of the scabbard service (must be \
                                     of the  form 'circuit_id::service_id')",
                                )
                                .long("service-id")
                                .takes_value(true)
                                .required(true),
                            Arg::with_name("wait")
                                .help("Time (in seconds) to wait for batches to be committed")
                                .long("wait")
                                .takes_value(true)
                                .default_value("300"),
                        ]),
                ),
        );
    }

    let matches = app.get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    setup_logging(log_level)?;

    match matches.subcommand() {
        ("contract", Some(matches)) => match matches.subcommand() {
            ("upload", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let scar = matches
                    .value_of("scar")
                    .ok_or_else(|| CliError::MissingArgument("scar".into()))?;
                let sc_definition = SabreSmartContractDefinition::new_from_scar(scar)?;

                let action = CreateContractActionBuilder::new()
                    .with_name(sc_definition.metadata.name)
                    .with_version(sc_definition.metadata.version)
                    .with_inputs(sc_definition.metadata.inputs)
                    .with_outputs(sc_definition.metadata.outputs)
                    .with_contract(sc_definition.contract)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::CreateContract(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("list", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let format = matches
                    .value_of("format")
                    .expect("default not set for --format");

                let registries = client
                    .get_state_with_prefix(&service_id, Some("00ec01"))?
                    .iter()
                    .map(|entry| ContractRegistryList::from_bytes(entry.value()))
                    .collect::<Result<Vec<_>, _>>()?;

                let mut data = vec![];
                data.push(vec![
                    "NAME".to_string(),
                    "VERSIONS".to_string(),
                    "OWNERS".to_string(),
                ]);
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
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let contract = matches
                    .value_of("contract")
                    .ok_or_else(|| CliError::MissingArgument("contract".into()))?;
                let name_version = contract.splitn(2, ':').collect::<Vec<_>>();
                let name = name_version.get(0).ok_or_else(|| {
                    CliError::InvalidArgument("contract invalid: cannot be empty".into())
                })?;
                let version = name_version.get(1).ok_or_else(|| {
                    CliError::InvalidArgument(
                        "contract invalid: must be of the form 'name:version'".into(),
                    )
                })?;

                let address = transaction::compute_contract_address(name, version)?;
                let contract_bytes = client
                    .get_state_at_address(&service_id, &address)?
                    .ok_or_else(|| {
                        CliError::action_error(&format!("contract '{}' not found", contract))
                    })?;
                let contract_list = ContractList::from_bytes(&contract_bytes)?;
                let contract = contract_list
                    .contracts()
                    .get(0)
                    .ok_or_else(|| CliError::action_error("contract list is empty"))?;

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
            _ => Err(CliError::InvalidSubcommand),
        },
        ("exec", Some(matches)) => {
            let url = matches.value_of("url").expect("default not set for --url");
            let client = ScabbardClient::new(url);

            let full_service_id = matches
                .value_of("service-id")
                .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
            let service_id = ServiceId::new(full_service_id)?;

            let wait = matches
                .value_of("wait")
                .expect("default not set for --wait")
                .parse::<u64>()
                .map_err(|_| {
                    CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                })?;

            let key = matches
                .value_of("key")
                .ok_or_else(|| CliError::MissingArgument("key".into()))?;
            let signing_key = key::load_signing_key(key)?;
            let context = Secp256k1Context::new();
            let signer = SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                CliError::action_error_with_source("failed to create signer", err.into())
            })?;

            let contract = matches
                .value_of("contract")
                .ok_or_else(|| CliError::MissingArgument("contract".into()))?;
            let (name, version) = match contract.splitn(2, ':').collect::<Vec<_>>() {
                v if v.len() == 2 => Ok((v[0], v[1])),
                _ => Err(CliError::InvalidArgument(
                    "--contract must be of the form 'name:version'".into(),
                )),
            }?;

            let inputs = matches
                .values_of("inputs")
                .ok_or_else(|| CliError::MissingArgument("inputs".into()))?
                .map(String::from)
                .collect();

            let outputs = matches
                .values_of("outputs")
                .ok_or_else(|| CliError::MissingArgument("outputs".into()))?
                .map(String::from)
                .collect();

            let payload_file = matches
                .value_of("payload")
                .ok_or_else(|| CliError::MissingArgument("payload".into()))?;
            let contract_payload = load_file_into_bytes(payload_file)?;

            let action = ExecuteContractActionBuilder::new()
                .with_name(name.into())
                .with_version(version.into())
                .with_inputs(inputs)
                .with_outputs(outputs)
                .with_payload(contract_payload)
                .build()?;
            let payload = SabrePayloadBuilder::new()
                .with_action(Action::ExecuteContract(action))
                .build()?;

            let txn = transaction::create_transaction(payload, &signer)?;
            let batch = transaction::create_batch(vec![txn], &signer)?;
            let batch_list = transaction::create_batch_list_from_one(batch);

            Ok(client.submit(&service_id, batch_list, Some(wait))?)
        }
        ("ns", Some(matches)) => match matches.subcommand() {
            ("create", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let namespace = matches
                    .value_of("namespace")
                    .ok_or_else(|| CliError::MissingArgument("namespace".into()))?;
                let owners = matches
                    .values_of("owner")
                    .ok_or_else(|| CliError::MissingArgument("owner".into()))?
                    .map(String::from)
                    .collect();

                let action = CreateNamespaceRegistryActionBuilder::new()
                    .with_namespace(namespace.into())
                    .with_owners(owners)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::CreateNamespaceRegistry(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("update", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let namespace = matches
                    .value_of("namespace")
                    .ok_or_else(|| CliError::MissingArgument("namespace".into()))?;
                let owners = matches
                    .values_of("owner")
                    .ok_or_else(|| CliError::MissingArgument("owner".into()))?
                    .map(String::from)
                    .collect();

                let action = UpdateNamespaceRegistryOwnersActionBuilder::new()
                    .with_namespace(namespace.into())
                    .with_owners(owners)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::UpdateNamespaceRegistryOwners(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("delete", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let namespace = matches
                    .value_of("namespace")
                    .ok_or_else(|| CliError::MissingArgument("namespace".into()))?;

                let action = DeleteNamespaceRegistryActionBuilder::new()
                    .with_namespace(namespace.into())
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::DeleteNamespaceRegistry(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            _ => Err(CliError::InvalidSubcommand),
        },
        ("perm", Some(matches)) => {
            let url = matches.value_of("url").expect("default not set for --url");
            let client = ScabbardClient::new(url);

            let full_service_id = matches
                .value_of("service-id")
                .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
            let service_id = ServiceId::new(full_service_id)?;

            let wait = matches
                .value_of("wait")
                .expect("default not set for --wait")
                .parse::<u64>()
                .map_err(|_| {
                    CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                })?;

            let key = matches
                .value_of("key")
                .ok_or_else(|| CliError::MissingArgument("key".into()))?;
            let signing_key = key::load_signing_key(key)?;
            let context = Secp256k1Context::new();
            let signer = SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                CliError::action_error_with_source("failed to create signer", err.into())
            })?;

            let namespace = matches
                .value_of("namespace")
                .ok_or_else(|| CliError::MissingArgument("namespace".into()))?;

            let payload = if matches.is_present("delete") {
                let action = DeleteNamespaceRegistryPermissionActionBuilder::new()
                    .with_namespace(namespace.into())
                    .build()?;
                SabrePayloadBuilder::new()
                    .with_action(Action::DeleteNamespaceRegistryPermission(action))
                    .build()?
            } else {
                let contract = matches
                    .value_of("contract")
                    .ok_or_else(|| CliError::MissingArgument("contract".into()))?;
                let read = matches.is_present("read");
                let write = matches.is_present("write");

                let action = CreateNamespaceRegistryPermissionActionBuilder::new()
                    .with_namespace(namespace.into())
                    .with_contract_name(contract.into())
                    .with_read(read)
                    .with_write(write)
                    .build()?;
                SabrePayloadBuilder::new()
                    .with_action(Action::CreateNamespaceRegistryPermission(action))
                    .build()?
            };

            let txn = transaction::create_transaction(payload, &signer)?;
            let batch = transaction::create_batch(vec![txn], &signer)?;
            let batch_list = transaction::create_batch_list_from_one(batch);

            Ok(client.submit(&service_id, batch_list, Some(wait))?)
        }
        ("cr", Some(matches)) => match matches.subcommand() {
            ("create", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;
                let owners = matches
                    .values_of("owner")
                    .ok_or_else(|| CliError::MissingArgument("owner".into()))?
                    .map(String::from)
                    .collect();

                let action = CreateContractRegistryActionBuilder::new()
                    .with_name(name.into())
                    .with_owners(owners)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::CreateContractRegistry(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("update", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;
                let owners = matches
                    .values_of("owner")
                    .ok_or_else(|| CliError::MissingArgument("owner".into()))?
                    .map(String::from)
                    .collect();

                let action = UpdateContractRegistryOwnersActionBuilder::new()
                    .with_name(name.into())
                    .with_owners(owners)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::UpdateContractRegistryOwners(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("delete", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;

                let action = DeleteContractRegistryActionBuilder::new()
                    .with_name(name.into())
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::DeleteContractRegistry(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            _ => Err(CliError::InvalidSubcommand),
        },
        ("sp", Some(matches)) => match matches.subcommand() {
            ("create", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let org_id = matches
                    .value_of("org_id")
                    .ok_or_else(|| CliError::MissingArgument("org_id".into()))?;
                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;
                let sp_filename = matches
                    .value_of("filename")
                    .ok_or_else(|| CliError::MissingArgument("filename".into()))?;
                let function = load_file_into_bytes(sp_filename)?;

                let action = CreateSmartPermissionActionBuilder::new()
                    .with_name(name.to_string())
                    .with_org_id(org_id.to_string())
                    .with_function(function)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::CreateSmartPermission(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("update", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let org_id = matches
                    .value_of("org_id")
                    .ok_or_else(|| CliError::MissingArgument("org_id".into()))?;
                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;
                let sp_filename = matches
                    .value_of("filename")
                    .ok_or_else(|| CliError::MissingArgument("filename".into()))?;
                let function = load_file_into_bytes(sp_filename)?;

                let action = UpdateSmartPermissionActionBuilder::new()
                    .with_name(name.to_string())
                    .with_org_id(org_id.to_string())
                    .with_function(function)
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::UpdateSmartPermission(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            ("delete", Some(matches)) => {
                let url = matches.value_of("url").expect("default not set for --url");
                let client = ScabbardClient::new(url);

                let full_service_id = matches
                    .value_of("service-id")
                    .ok_or_else(|| CliError::MissingArgument("service-id".into()))?;
                let service_id = ServiceId::new(full_service_id)?;

                let wait = matches
                    .value_of("wait")
                    .expect("default not set for --wait")
                    .parse::<u64>()
                    .map_err(|_| {
                        CliError::InvalidArgument("'wait' argument must be a valid integer".into())
                    })?;

                let key = matches
                    .value_of("key")
                    .ok_or_else(|| CliError::MissingArgument("key".into()))?;
                let signing_key = key::load_signing_key(key)?;
                let context = Secp256k1Context::new();
                let signer =
                    SawtoothSecp256k1RefSigner::new(&context, signing_key).map_err(|err| {
                        CliError::action_error_with_source("failed to create signer", err.into())
                    })?;

                let org_id = matches
                    .value_of("org_id")
                    .ok_or_else(|| CliError::MissingArgument("org_id".into()))?;
                let name = matches
                    .value_of("name")
                    .ok_or_else(|| CliError::MissingArgument("name".into()))?;

                let action = DeleteSmartPermissionActionBuilder::new()
                    .with_name(name.to_string())
                    .with_org_id(org_id.to_string())
                    .build()?;
                let payload = SabrePayloadBuilder::new()
                    .with_action(Action::DeleteSmartPermission(action))
                    .build()?;

                let txn = transaction::create_transaction(payload, &signer)?;
                let batch = transaction::create_batch(vec![txn], &signer)?;
                let batch_list = transaction::create_batch_list_from_one(batch);

                Ok(client.submit(&service_id, batch_list, Some(wait))?)
            }
            _ => Err(CliError::InvalidSubcommand),
        },
        _ => Err(CliError::InvalidSubcommand),
    }
}

fn setup_logging(log_level: log::LevelFilter) -> Result<(), CliError> {
    let mut log_spec_builder = LogSpecBuilder::new();
    log_spec_builder.default(log_level);

    Logger::with(log_spec_builder.build())
        .format(log_format)
        .start()?;

    Ok(())
}

// log format for cli that will only show the log message
fn log_format(
    w: &mut dyn std::io::Write,
    _now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(w, "{}", record.args(),)
}

/// Load the contents of a file into a bytes vector.
fn load_file_into_bytes(payload_file: &str) -> Result<Vec<u8>, CliError> {
    let file = File::open(payload_file)
        .map_err(|err| CliError::action_error_with_source("failed to load file", err.into()))?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader
        .read_to_end(&mut contents)
        .map_err(|err| CliError::action_error_with_source("failed to read file", err.into()))?;
    Ok(contents)
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
