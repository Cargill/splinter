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

use clap::ArgMatches;
use reqwest::StatusCode;
use serde_json::Value;

use super::{Action, DEFAULT_SPLINTER_REST_API_URL, SPLINTER_REST_API_URL_ENV};

use crate::error::CliError;

pub struct StatusAction;

const SPLINTERD_MISSING_HEALTH_STATUS: &str = "The health status endpoint was not found. \
                                               The specified splinter daemon has not enabled this \
                                               feature.";

impl Action for StatusAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = arg_matches
            .and_then(|args| args.value_of("url"))
            .map(ToOwned::to_owned)
            .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
            .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

        reqwest::blocking::get(&format!("{}/health/status", url))
            .map_err(|err| match err.status() {
                Some(StatusCode::NOT_FOUND) => {
                    CliError::ActionError(SPLINTERD_MISSING_HEALTH_STATUS.into())
                }
                Some(status_code) => CliError::ActionError(format!(
                    "The server failed to respond({}).",
                    status_code.as_u16()
                )),
                _ => CliError::ActionError(format!("Unable to contact the server at {}", url)),
            })
            .and_then(|res| match res.status() {
                StatusCode::OK => res.json().map_err(|_| {
                    CliError::ActionError("The server failed to send a valid response".into())
                }),
                StatusCode::NOT_FOUND => Err(CliError::ActionError(
                    SPLINTERD_MISSING_HEALTH_STATUS.into(),
                )),
                status_code => Err(CliError::ActionError(format!(
                    "The server failed to respond({}).",
                    status_code.as_u16()
                ))),
            })
            .and_then(|status: Value| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&status).map_err(|_| CliError::ActionError(
                        "Failed to serialize response".into()
                    ))?
                );
                Ok(())
            })
    }
}
