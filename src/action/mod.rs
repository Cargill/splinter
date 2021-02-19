// Copyright 2018-2021 Cargill Incorporated
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

#[cfg(feature = "playlist")]
pub mod playlist;
#[cfg(feature = "workload")]
pub mod workload;

use std::collections::HashMap;
#[cfg(any(feature = "workload", feature = "playlist"))]
use std::path::Path;

use clap::ArgMatches;
#[cfg(any(feature = "workload", feature = "playlist"))]
use cylinder::{
    current_user_search_path, jwt::JsonWebTokenBuilder, load_key, load_key_from_path,
    secp256k1::Secp256k1Context, Context, Signer,
};

use super::error::CliError;

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

    #[cfg(any(feature = "playlist", feature = "workload"))]
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

#[cfg(any(feature = "playlist", feature = "workload"))]
// build a signed json web token using the private key
fn create_cylinder_jwt_auth_signer_key(
    key_name: &str,
) -> Result<(String, Box<dyn Signer>), CliError> {
    let private_key = if key_name.contains('/') {
        load_key_from_path(Path::new(key_name))
            .map_err(|err| CliError::ActionError(err.to_string()))?
    } else {
        let path = &current_user_search_path();
        load_key(key_name, path)
            .map_err(|err| CliError::ActionError(err.to_string()))?
            .ok_or_else(|| {
                CliError::ActionError({
                    format!(
                        "No signing key found in {}. Specify the --key argument",
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

    Ok((format!("Bearer Cylinder:{}", encoded_token), signer))
}

#[cfg(feature = "playlist")]
// load signing key from key file
fn load_cylinder_signer_key(key_name: &str) -> Result<Box<dyn Signer>, CliError> {
    let private_key = if key_name.contains('/') {
        load_key_from_path(Path::new(key_name))
            .map_err(|err| CliError::ActionError(err.to_string()))?
    } else {
        let path = &current_user_search_path();
        load_key(key_name, path)
            .map_err(|err| CliError::ActionError(err.to_string()))?
            .ok_or_else(|| {
                CliError::ActionError({
                    format!(
                        "No signing key found in {}. Specify the --key argument",
                        path.iter()
                            .map(|path| path.as_path().display().to_string())
                            .collect::<Vec<String>>()
                            .join(":")
                    )
                })
            })?
    };

    let context = Secp256k1Context::new();
    Ok(context.new_signer(private_key))
}
