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

pub const SUBCMD: &str = "upload";
const ABOUT_STR: &str = "upload a Sabre contract";

const SCAR_ARG: &str = "scar";
const SCAR_ARG_HELP: &str = "the .scar to upload (either a file path or the name of an archive in \
                             SCAR_PATH)";

pub fn get_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(SUBCMD)
        .about(ABOUT_STR)
        .arg(Arg::with_name(SCAR_ARG).help(SCAR_ARG_HELP).required(true))
}

pub fn upload(matches: &ArgMatches, client: ScabbardClient) -> Result<(), CliError> {
    let scar = matches
        .value_of(SCAR_ARG)
        .ok_or_else(|| CliError::MissingArgument(SCAR_ARG.into()))?;
    client.upload_contract_from_scar(scar)?.submit()?;
    Ok(())
}
