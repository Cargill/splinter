// Copyright 2018-2020 Cargill Incorporated
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

use std::error::Error;
use std::fmt;

use clap::Error as ClapError;

#[derive(Debug)]
pub enum CliError {
    /// A subcommand requires one or more arguments, but none were provided.
    RequiresArgs,
    /// A non-existent subcommand was specified.
    InvalidSubcommand,
    /// An error was detected by `clap`.
    ClapError(ClapError),
    /// A general error encountered by a subcommand.
    ActionError(String),
    /// The environment is not in the correct state to execute the subcommand as requested.
    EnvironmentError(String),
}

impl Error for CliError {}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CliError::RequiresArgs => write!(
                f,
                "The specified subcommand requires arguments, but none were provided"
            ),
            CliError::InvalidSubcommand => write!(f, "An invalid subcommand was specified"),
            CliError::ClapError(err) => f.write_str(&err.message),
            CliError::ActionError(msg) => write!(f, "Subcommand encountered an error: {}", msg),
            CliError::EnvironmentError(msg) => {
                write!(f, "Environment not valid for subcommand: {}", msg)
            }
        }
    }
}

impl From<ClapError> for CliError {
    fn from(err: ClapError) -> Self {
        Self::ClapError(err)
    }
}
