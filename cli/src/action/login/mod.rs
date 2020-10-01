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

#[cfg(feature = "login-oauth2")]
mod oauth;

use clap::ArgMatches;

use crate::error::CliError;

use super::{Action, DEFAULT_SPLINTER_REST_API_URL, SPLINTER_REST_API_URL_ENV};

pub struct LoginAction;

impl Action for LoginAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        if cfg!(feature = "login-oauth2") {
            let args = arg_matches.ok_or_else(|| CliError::RequiresArgs)?;

            let mut splinter_dir = dirs::home_dir().ok_or_else(|| {
                CliError::ActionError(
                    "Unable to determine your home directory; this is required to log in.".into(),
                )
            })?;
            splinter_dir.push(".splinter");

            let url = args
                .value_of("url")
                .map(ToOwned::to_owned)
                .or_else(|| std::env::var(SPLINTER_REST_API_URL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_SPLINTER_REST_API_URL.to_string());

            return oauth::handle_oauth2_login(&url, splinter_dir);
        }

        Ok(())
    }
}
