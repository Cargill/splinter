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

pub const SUBCMD: &str = "sp";
const ABOUT_STR: &str = "create, update or delete smart permissions";

const ORG_ID_ARG: &str = "org_id";
const ORG_ID_ARG_HELP: &str = "organization ID";

const NAME_ARG: &str = "name";
const NAME_ARG_HELP: &str = "name of the smart permission";

const FILENAME_ARG: &str = "filename";
const FILENAME_ARG_HELP: &str = "path to smart permission";
const FILENAME_ARG_SHORT: &str = "f";

const CREATE_SUBCMD: &str = "create";
const UPDATE_SUBCMD: &str = "update";
const DELETE_SUBCMD: &str = "delete";

pub fn get_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(SUBCMD)
        .about(ABOUT_STR)
        .subcommand(SubCommand::with_name(CREATE_SUBCMD).args(&generate_create_or_update_args()))
        .subcommand(SubCommand::with_name(UPDATE_SUBCMD).args(&generate_create_or_update_args()))
        .subcommand(SubCommand::with_name(DELETE_SUBCMD).args(&generate_org_id_and_name_args()))
}

fn generate_create_or_update_args<'a, 'b>() -> Vec<Arg<'a, 'b>> {
    let mut args = generate_org_id_and_name_args();
    args.push(
        Arg::with_name(FILENAME_ARG)
            .help(FILENAME_ARG_HELP)
            .short(FILENAME_ARG_SHORT)
            .long(FILENAME_ARG)
            .required(true)
            .takes_value(true),
    );
    args
}

fn generate_org_id_and_name_args<'a, 'b>() -> Vec<Arg<'a, 'b>> {
    vec![
        Arg::with_name(ORG_ID_ARG)
            .help(ORG_ID_ARG_HELP)
            .required(true),
        Arg::with_name(NAME_ARG).help(NAME_ARG_HELP).required(true),
    ]
}

pub fn handle_smart_permission_cmd(
    matches: &ArgMatches,
    client: ScabbardClient,
) -> Result<(), CliError> {
    if let Some(matches) = matches.subcommand_matches(CREATE_SUBCMD) {
        create_smart_permission(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(UPDATE_SUBCMD) {
        update_smart_permission(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(DELETE_SUBCMD) {
        delete_smart_permission(matches, client)
    } else {
        Err(CliError::InvalidSubcommand)
    }
}

fn create_smart_permission(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let (org_id, name, sp_filename) = get_create_or_update_args(matches)?;
    client
        .create_smart_permission_from_file(org_id, name, sp_filename)?
        .submit()?;
    Ok(())
}

fn update_smart_permission(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let (org_id, name, sp_filename) = get_create_or_update_args(matches)?;
    client
        .update_smart_permission_from_file(org_id, name, sp_filename)?
        .submit()?;
    Ok(())
}

fn delete_smart_permission(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let (org_id, name) = get_org_id_and_name_args(matches)?;
    client.delete_smart_permission(org_id, name)?.submit()?;
    Ok(())
}

fn get_create_or_update_args<'a>(
    matches: &'a ArgMatches,
) -> Result<(&'a str, &'a str, &'a str), CliError> {
    let (org_id, name) = get_org_id_and_name_args(matches)?;
    let sp_filename = matches
        .value_of(FILENAME_ARG)
        .ok_or_else(|| CliError::MissingArgument(FILENAME_ARG.into()))?;

    Ok((org_id, name, sp_filename))
}

fn get_org_id_and_name_args<'a>(matches: &'a ArgMatches) -> Result<(&'a str, &'a str), CliError> {
    let org_id = matches
        .value_of(ORG_ID_ARG)
        .ok_or_else(|| CliError::MissingArgument(ORG_ID_ARG.into()))?;
    let name = matches
        .value_of(NAME_ARG)
        .ok_or_else(|| CliError::MissingArgument(NAME_ARG.into()))?;
    Ok((org_id, name))
}
