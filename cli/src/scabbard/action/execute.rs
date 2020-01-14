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

pub const SUBCMD: &str = "exec";
const ABOUT_STR: &str = "execute a Sabre contract";

const CONTRACT_ARG: &str = "contract";
const CONTRACT_ARG_HELP: &str = "name:version of a Sabre contract";
const CONTRACT_ARG_SHORT: &str = "C";

const PAYLOAD_ARG: &str = "payload";
const PAYLOAD_ARG_HELP: &str = "path to Sabre contract payload";
const PAYLOAD_ARG_SHORT: &str = "p";

const INPUTS_ARG: &str = "inputs";
const INPUT_ARG_HELP: &str = "input addresses used by the contract";

const OUTPUTS_ARG: &str = "outputs";
const OUTPUT_ARG_HELP: &str = "output addresses used by the contract";

pub fn get_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(SUBCMD).about(ABOUT_STR).args(&[
        Arg::with_name(CONTRACT_ARG)
            .help(CONTRACT_ARG_HELP)
            .short(CONTRACT_ARG_SHORT)
            .long(CONTRACT_ARG)
            .required(true)
            .takes_value(true),
        Arg::with_name(PAYLOAD_ARG)
            .help(PAYLOAD_ARG_HELP)
            .short(PAYLOAD_ARG_SHORT)
            .long(PAYLOAD_ARG)
            .required(true)
            .takes_value(true),
        Arg::with_name(INPUTS_ARG)
            .help(INPUT_ARG_HELP)
            .long(INPUTS_ARG)
            .required(true)
            .takes_value(true)
            .multiple(true),
        Arg::with_name(OUTPUTS_ARG)
            .help(OUTPUT_ARG_HELP)
            .long(OUTPUTS_ARG)
            .required(true)
            .takes_value(true)
            .multiple(true),
    ])
}

pub fn execute(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let contract = matches
        .value_of(CONTRACT_ARG)
        .ok_or_else(|| CliError::MissingArgument(CONTRACT_ARG.into()))?;
    let (name, version) = match contract.splitn(2, ':').collect::<Vec<_>>() {
        v if v.len() == 2 => Ok((v[0], v[1])),
        _ => Err(CliError::InvalidArgument(format!(
            "'{}' must be of the form 'name:version'",
            CONTRACT_ARG
        ))),
    }?;

    let inputs = matches
        .values_of(INPUTS_ARG)
        .ok_or_else(|| CliError::MissingArgument(INPUTS_ARG.into()))?
        .map(String::from)
        .collect();

    let outputs = matches
        .values_of(OUTPUTS_ARG)
        .ok_or_else(|| CliError::MissingArgument(OUTPUTS_ARG.into()))?
        .map(String::from)
        .collect();

    let payload_file = matches
        .value_of(PAYLOAD_ARG)
        .ok_or_else(|| CliError::MissingArgument(PAYLOAD_ARG.into()))?;

    client
        .execute_contract_from_file(name, version, inputs, outputs, payload_file)?
        .submit()?;

    Ok(())
}
