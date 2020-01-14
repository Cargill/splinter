// Copyright 2020 Cargill Incorporated
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

use clap::{App, Arg, ArgMatches, SubCommand};
use splinter::service::scabbard::client::ScabbardClient;

use super::CliError;

pub const SUBCMD: &str = "perm";
const ABOUT_STR: &str = "set or delete a Sabre namespace permission";

const NAMESPACE_ARG: &str = "namespace";
const NAMESPACE_ARG_HELP: &str = "a global state address prefix (namespace)";

const CONTRACT_ARG: &str = "contract";
const CONTRACT_ARG_HELP: &str = "name of the contract";

const READ_ARG: &str = "read";
const READ_ARG_HELP: &str = "set read permission";
const READ_ARG_SHORT: &str = "r";

const WRITE_ARG: &str = "write";
const WRITE_ARG_HELP: &str = "set write permission";
const WRITE_ARG_SHORT: &str = "w";

const DELETE_ARG: &str = "delete";
const DELETE_ARG_HELP: &str = "remove all permissions";
const DELETE_ARG_SHORT: &str = "d";

pub fn get_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(SUBCMD).about(ABOUT_STR).args(&[
        Arg::with_name(NAMESPACE_ARG)
            .help(NAMESPACE_ARG_HELP)
            .required(true),
        Arg::with_name(CONTRACT_ARG)
            .help(CONTRACT_ARG_HELP)
            .required(true)
            .conflicts_with(DELETE_ARG),
        Arg::with_name(READ_ARG)
            .help(READ_ARG_HELP)
            .short(READ_ARG_SHORT)
            .long(READ_ARG)
            .conflicts_with(DELETE_ARG),
        Arg::with_name(WRITE_ARG)
            .help(WRITE_ARG_HELP)
            .short(WRITE_ARG_SHORT)
            .long(WRITE_ARG)
            .conflicts_with(DELETE_ARG),
        Arg::with_name(DELETE_ARG)
            .help(DELETE_ARG_HELP)
            .short(DELETE_ARG_SHORT)
            .long(DELETE_ARG),
    ])
}

pub fn handle_permission_cmd(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    if matches.is_present(DELETE_ARG) {
        delete_namespace_registry_permission(matches, client)
    } else {
        create_namespace_registry_permission(matches, client)
    }
}

fn delete_namespace_registry_permission(
    matches: &ArgMatches,
    client: ScabbardClient,
) -> Result<(), CliError> {
    let namespace = matches
        .value_of(NAMESPACE_ARG)
        .ok_or_else(|| CliError::MissingArgument(NAMESPACE_ARG.into()))?;

    client
        .delete_namespace_registry_permission(namespace)?
        .submit()?;

    Ok(())
}

fn create_namespace_registry_permission(
    matches: &ArgMatches,
    client: ScabbardClient,
) -> Result<(), CliError> {
    let namespace = matches
        .value_of(NAMESPACE_ARG)
        .ok_or_else(|| CliError::MissingArgument(NAMESPACE_ARG.into()))?;
    let contract = matches
        .value_of(CONTRACT_ARG)
        .ok_or_else(|| CliError::MissingArgument(CONTRACT_ARG.into()))?;
    let read = matches.is_present(READ_ARG);
    let write = matches.is_present(WRITE_ARG);

    client
        .create_namespace_registry_permission(namespace, contract, read, write)?
        .submit()?;

    Ok(())
}
