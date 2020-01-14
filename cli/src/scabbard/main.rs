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

mod action;
mod error;

use clap::{App, AppSettings, Arg};
use splinter::service::scabbard::client::ScabbardClient;

use error::CliError;

const APP_NAME: &str = "scabbard";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const AUTHOR: &str = "Cargill";
const ABOUT_STR: &str = "Command line for scabbard";

const KEY_ARG: &str = "key";
const KEY_ARG_HELP: &str = "path to signing key";
const KEY_ARG_SHORT: &str = "k";

const URL_ARG: &str = "url";
const URL_ARG_HELP: &str = "URL to the scabbard REST API";
const URL_ARG_SHORT: &str = "U";
const DEFAULT_URL: &str = "http://localhost:8008";

fn run() -> Result<(), CliError> {
    let app = App::new(APP_NAME)
        .version(VERSION)
        .author(AUTHOR)
        .about(ABOUT_STR)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name(KEY_ARG)
                .help(KEY_ARG_HELP)
                .short(KEY_ARG_SHORT)
                .long(KEY_ARG)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name(URL_ARG)
                .help(URL_ARG_HELP)
                .short(URL_ARG_SHORT)
                .long(URL_ARG)
                .takes_value(true)
                .default_value(DEFAULT_URL),
        )
        .subcommands(vec![
            action::upload::get_subcommand(),
            action::execute::get_subcommand(),
            action::namespace::get_subcommand(),
            action::permission::get_subcommand(),
            action::contract_registry::get_subcommand(),
            action::smart_permission::get_subcommand(),
        ]);

    let matches = app.get_matches();

    let key_file = matches
        .value_of(KEY_ARG)
        .ok_or_else(|| CliError::MissingArgument(KEY_ARG.into()))?;
    let url = matches
        .value_of(URL_ARG)
        .expect("default not set for URL_ARG");

    let client = ScabbardClient::new_with_local_signing_key(url, key_file)?;

    if let Some(matches) = matches.subcommand_matches(action::upload::SUBCMD) {
        action::upload::upload(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(action::execute::SUBCMD) {
        action::execute::execute(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(action::namespace::SUBCMD) {
        action::namespace::handle_namespace_cmd(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(action::permission::SUBCMD) {
        action::permission::handle_permission_cmd(matches, client)
    } else if let Some(matches) = matches.subcommand_matches(action::contract_registry::SUBCMD) {
        action::contract_registry::handle_contract_registry_cmd(matches, client)
    } else if let Some(matches) = matches.subcommand_matches("sp") {
        action::smart_permission::handle_smart_permission_cmd(matches, client)
    } else {
        Err(CliError::InvalidSubcommand)
    }
}

fn main() {
    if let Err(e) = run() {
        println!("ERROR: {}", e);
        std::process::exit(1);
    }
}
