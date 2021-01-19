// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Intel Corporation
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

pub mod admin;
mod api;
pub mod certs;
pub mod circuit;
#[cfg(feature = "database")]
pub mod database;
#[cfg(feature = "health")]
pub mod health;
pub mod keygen;
#[cfg(feature = "authorization-handler-maintenance")]
pub mod maintenance;
pub mod registry;

use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::{Error as IoError, ErrorKind, Read};
use std::path::Path;

use clap::ArgMatches;
use cylinder::{
    current_user_key_name, current_user_search_path, jwt::JsonWebTokenBuilder, load_key,
    load_key_from_path, secp256k1::Secp256k1Context, Context,
};

use super::error::CliError;

const DEFAULT_SPLINTER_REST_API_URL: &str = "http://127.0.0.1:8080";
const SPLINTER_REST_API_URL_ENV: &str = "SPLINTER_REST_API_URL";

/// A CLI Command Action.
///
/// An Action is a single subcommand for CLI operations.
pub trait Action {
    /// Run a CLI Action with the given args
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError>;
}

/// A collection of Subcommands associated with a single parent command.
#[derive(Default)]
pub struct SubcommandActions<'a> {
    actions: HashMap<String, Box<dyn Action + 'a>>,
}

impl<'a> SubcommandActions<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_command<'action: 'a, A: Action + 'action>(
        mut self,
        command: &str,
        action: A,
    ) -> Self {
        self.actions.insert(command.to_string(), Box::new(action));

        self
    }
}

impl<'s> Action for SubcommandActions<'s> {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let (subcommand, args) = args.subcommand();

        if let Some(action) = self.actions.get_mut(subcommand) {
            action.run(args)
        } else {
            Err(CliError::InvalidSubcommand)
        }
    }
}

fn chown(path: &Path, uid: u32, gid: u32) -> Result<(), CliError> {
    let pathstr = path
        .to_str()
        .ok_or_else(|| CliError::EnvironmentError(format!("Invalid path: {:?}", path)))?;
    let cpath =
        CString::new(pathstr).map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;
    let result = unsafe { libc::chown(cpath.as_ptr(), uid, gid) };
    match result {
        0 => Ok(()),
        code => Err(CliError::EnvironmentError(format!(
            "Error chowning file {}: {}",
            pathstr, code
        ))),
    }
}

/// Reads a private key from the given file name.
fn read_private_key(file_name: &str) -> Result<String, CliError> {
    let mut file = File::open(file_name).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Unable to open key file '{}': {}",
            file_name,
            msg_from_io_error(err)
        ))
    })?;

    let mut buf = String::new();
    file.read_to_string(&mut buf).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Unable to read key file '{}': {}",
            file_name,
            msg_from_io_error(err)
        ))
    })?;
    let key = buf.trim().to_string();

    Ok(key)
}

fn msg_from_io_error(err: IoError) -> String {
    match err.kind() {
        ErrorKind::NotFound => "File not found".into(),
        ErrorKind::PermissionDenied => "Permission denied".into(),
        ErrorKind::InvalidData => "Invalid data".into(),
        _ => "Unknown I/O error".into(),
    }
}

// build a signed json web token using the private key
fn create_cylinder_jwt_auth(key_name: Option<&str>) -> Result<String, CliError> {
    let private_key = if let Some(key_name) = key_name {
        if key_name.contains('/') {
            load_key_from_path(Path::new(key_name))
                .map_err(|err| CliError::ActionError(err.to_string()))?
        } else {
            let path = &current_user_search_path();
            load_key(key_name, path)
                .map_err(|err| CliError::ActionError(err.to_string()))?
                .ok_or_else(|| {
                    CliError::ActionError({
                        format!(
                            "No signing key found in {}. Either specify the --key argument or \
                            generate the default key via splinter keygen",
                            path.iter()
                                .map(|path| path.as_path().display().to_string())
                                .collect::<Vec<String>>()
                                .join(":")
                        )
                    })
                })?
        }
    } else {
        // If the `CYLINDER_PATH` environment variable is not set, add `$HOME/.splinter/keys`
        // to the vector of paths to search. This is for backwards compatibility.
        let path = match env::var("CYLINDER_PATH") {
            Ok(_) => current_user_search_path(),
            Err(_) => {
                let mut splinter_path = match dirs::home_dir() {
                    Some(dir) => dir,
                    None => Path::new(".").to_path_buf(),
                };
                splinter_path.push(".splinter");
                splinter_path.push("keys");
                let mut paths = current_user_search_path();
                paths.push(splinter_path);
                paths
            }
        };
        load_key(&current_user_key_name(), &path)
            .map_err(|err| CliError::ActionError(err.to_string()))?
            .ok_or_else(|| {
                CliError::ActionError({
                    format!(
                        "No signing key found in {}. Either specify the --key argument or \
                        generate the default key via splinter keygen",
                        path.iter()
                            .map(|path| path.as_path().display().to_string())
                            .collect::<Vec<String>>()
                            .join(":")
                    )
                })
            })?
    };

    let context = Secp256k1Context::new();
    let signer = context.new_signer(private_key);

    let encoded_token = JsonWebTokenBuilder::new()
        .build(&*signer)
        .map_err(|err| CliError::ActionError(format!("failed to build json web token: {}", err)))?;

    Ok(format!("Bearer Cylinder:{}", encoded_token))
}
