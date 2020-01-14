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

pub const SUBCMD: &str = "ns";
const ABOUT_STR: &str = "create, update, or delete a Sabre namespace";

const NAMESPACE_ARG: &str = "namespace";
const NAMESPACE_ARG_HELP: &str = "a global state address prefix (namespace)";

const OWNER_ARG: &str = "owner";
const OWNER_ARG_HELP: &str = "owner of this namespace";
const OWNER_ARG_SHORT: &str = "O";

const CREATE_SUBCMD: &str = "create";
const UPDATE_SUBCMD: &str = "update";
const DELETE_SUBCMD: &str = "delete";

pub fn get_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(SUBCMD)
        .about(ABOUT_STR)
        .subcommand(SubCommand::with_name(CREATE_SUBCMD).args(&generate_create_or_update_args()))
        .subcommand(SubCommand::with_name(UPDATE_SUBCMD).args(&generate_create_or_update_args()))
        .subcommand(SubCommand::with_name(DELETE_SUBCMD).arg(generate_namespace_arg()))
}

fn generate_create_or_update_args<'a, 'b>() -> Vec<Arg<'a, 'b>> {
    vec![
        generate_namespace_arg(),
        Arg::with_name(OWNER_ARG)
            .help(OWNER_ARG_HELP)
            .short(OWNER_ARG_SHORT)
            .long(OWNER_ARG)
            .required(true)
            .takes_value(true)
            .multiple(true),
    ]
}

fn generate_namespace_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name(NAMESPACE_ARG)
        .help(NAMESPACE_ARG_HELP)
        .required(true)
}

pub fn handle_namespace_cmd(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    if let Some(matches) = matches.subcommand_matches(CREATE_SUBCMD) {
        create_namespace_registry(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(UPDATE_SUBCMD) {
        update_namespace_registry(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(DELETE_SUBCMD) {
        delete_namespace_registry(matches, client)
    } else {
        Err(CliError::InvalidSubcommand)
    }
}

fn create_namespace_registry(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let (namespace, owners) = get_create_or_update_args(matches)?;
    client
        .create_namespace_registry(namespace, owners)?
        .submit()?;
    Ok(())
}

fn update_namespace_registry(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let (namespace, owners) = get_create_or_update_args(matches)?;
    client
        .update_namespace_registry(namespace, owners)?
        .submit()?;
    Ok(())
}

fn delete_namespace_registry(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let namespace = get_namespace_arg(matches)?;
    client.delete_namespace_registry(namespace)?.submit()?;
    Ok(())
}

fn get_create_or_update_args<'a>(
    matches: &'a ArgMatches,
) -> Result<(&'a str, Vec<String>), CliError> {
    let namespace = get_namespace_arg(matches)?;
    let owners = matches
        .values_of(OWNER_ARG)
        .ok_or_else(|| CliError::MissingArgument(OWNER_ARG.into()))?
        .map(String::from)
        .collect();

    Ok((namespace, owners))
}

fn get_namespace_arg<'a>(matches: &'a ArgMatches) -> Result<&'a str, CliError> {
    matches
        .value_of(NAMESPACE_ARG)
        .ok_or_else(|| CliError::MissingArgument(NAMESPACE_ARG.into()))
}
