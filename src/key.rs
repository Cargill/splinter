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

//! Contains functions which assist with signing key management

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

// use sawtooth_sdk::signing::{
//     create_context, secp256k1::Secp256k1PrivateKey,
// };
use cylinder::{secp256k1::Secp256k1Context, Context, PrivateKey, Signer};
use users::get_current_username;

use crate::error::CliError;

/// Return a `TransactSigner`, loading the signing key from the user's environment.
pub fn new_signer(key_name: Option<&str>) -> Result<Box<dyn Signer>, CliError> {
    let context = Secp256k1Context::new();
    let private_key = load_signing_key(key_name)?;
    Ok(context.new_signer(private_key))
}

/// Return a signing key loaded from the user's environment
///
/// This method attempts to load the user's key from a file.
/// The input parameter ```key_param (K)``` works as follows.
/// The parameter `key_param` is optional
///
/// If the parameter is Some:
///   1) The short-name translates to the ```keyfile``` at
///      ${HOME}/.sawtooth/(K).priv
///   2) If a ```keyfile``` as in point (1) is not found then a file
///      ${HOME}/.sawtooth/{K} is searched for.
///   3) If a ```keyfile``` as in point (2) also fails then a path
///      {K} is searched for.
///
/// If the parameter is None:
/// The USER environment variable is used as a key file identifier.
/// The filename is constructed by appending ".priv" to the
/// constructed key's name from the USER environment variable.
/// The directory containing the keys is determined using the HOME
/// environment variable:
///
///   $HOME/.sawtooth/keys/
///
/// # Arguments
///
/// * `key_param` - The signing key parameter to be loaded
///
/// # Errors
///
/// If a signing error occurs, a CliError::SigningError is returned.
///
/// If a HOME or USER environment variable is required but cannot be
/// retrieved from the environment, a CliError::VarError is returned.
fn load_signing_key(key_param: Option<&str>) -> Result<PrivateKey, CliError> {
    let derived_keyfile: String = key_param
        .map(String::from)
        .ok_or_else(|| env::var("USER"))
        .or_else(|_| get_current_username().ok_or(0))
        .map_err(|_| {
            CliError::User(String::from(
                "Could not load signing key: unable to determine username",
            ))
        })?;

    // For the case Some(scenario 3)
    let mut private_key_filename: PathBuf = PathBuf::from(&derived_keyfile);

    // For the case Some(scenario 2)
    let keyfile_identifier = dirs::home_dir()
        .ok_or_else(|| {
            CliError::User(String::from(
                "Could not load signing key: unable to determine home directory",
            ))
        })
        .map(|mut p| {
            p.push(".sawtooth");
            p.push("keys");
            p.push(&derived_keyfile);
            p
        })?;
    if keyfile_identifier.as_path().exists() {
        private_key_filename = keyfile_identifier;
    }

    // For the case Some(scenario 1) and None
    let key_identifier = dirs::home_dir()
        .ok_or_else(|| {
            CliError::User(String::from(
                "Could not load signing key: unable to determine home directory",
            ))
        })
        .map(|mut p| {
            p.push(".sawtooth");
            p.push("keys");
            p.push(format!("{}.priv", &derived_keyfile));
            p
        })?;
    if key_identifier.as_path().exists() {
        private_key_filename = key_identifier;
    }

    if !private_key_filename.as_path().exists() {
        return Err(CliError::User(format!(
            "No such key file: {}",
            private_key_filename.display()
        )));
    }

    let mut f = File::open(&private_key_filename)?;

    let mut contents = String::new();
    f.read_to_string(&mut contents)?;

    let key_str = match contents.lines().next() {
        Some(k) => k,
        None => {
            return Err(CliError::User(format!(
                "Empty key file: {}",
                private_key_filename.display()
            )));
        }
    };

    PrivateKey::new_from_hex(&key_str).map_err(|err| {
        CliError::Signing(format!(
            "Unable to parse private key file {}: {} ",
            private_key_filename.display(),
            err
        ))
    })
}
