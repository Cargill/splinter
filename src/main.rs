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
extern crate crypto;
extern crate futures;
extern crate hyper;
extern crate protobuf;
extern crate sawtooth_sdk;
extern crate tokio_core;
extern crate users;
extern crate yaml_rust;

mod error;
mod execute;
mod key;
mod protos;
mod submit;
mod transaction;
mod upload;

const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn run() -> Result<(), error::CliError> {
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
        )
        (@subcommand exec =>
            (about: "execute a Sabre contract")
            (@arg contract: -C --contract +required +takes_value "Name:Version of a Sabre contract")
            (@arg payload: -p --payload +required +takes_value "Path to Sabre contract payload")
            (@arg key: -k --key +takes_value "Signing key name")
            (@arg url: --url +takes_value "URL to the Sawtooth REST API")
            (@arg inputs: --inputs +takes_value "Input addresses used by the contract")
            (@arg outputs: --outputs +takes_value "Output addresses used by the contract")
        )
    ).get_matches();

    if let Some(upload_matches) = matches.subcommand_matches("upload") {
        let filename = upload_matches.value_of("filename").unwrap();
        let key_name = upload_matches.value_of("key");
        let url = upload_matches.value_of("url").unwrap_or("http://localhost:8008/");

        upload::do_upload(&filename, key_name, &url)?;
    }

    if let Some(exec_matches) = matches.subcommand_matches("exec") {
        let contract = exec_matches.value_of("contract").unwrap();
        let payload = exec_matches.value_of("payload").unwrap();
        let key_name = exec_matches.value_of("key");
        let url = exec_matches.value_of("url").unwrap_or("http://localhost:8008/");

        let inputs = exec_matches.value_of("inputs").unwrap_or("*").split(":").map(|i| i.into()).collect();
        let outputs = exec_matches.value_of("outputs").unwrap_or("*").split(":").map(|o| o.into()).collect();

        let (name, version) = match contract.split(":").collect::<Vec<_>>() {
            ref v if (v.len() == 1 || v.len() == 2) && v[0].len() == 0 => Err(error::CliError::UserError("contract name must be specified".into())),
            ref v if v.len() == 1 || v.len() == 2 && v[1].len() == 0 => Ok((v[0], "latest".into())),
            ref v if v.len() == 2 => Ok((v[0], v[1])),
            _ => Err(error::CliError::UserError("malformed contract argument, may contain at most one ':'".into()))
        }?;

        execute::do_exec(&name, &version, &payload, inputs, outputs, key_name, &url)?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        println!("{}", e);
        std::process::exit(1);
    }
}
