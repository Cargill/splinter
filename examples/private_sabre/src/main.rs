// Copyright 2019 Cargill Incorporated
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
#[macro_use]
extern crate log;

mod error;

use crate::error::ServiceError;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), ServiceError> {
    let matches = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (author: "Contributors to Splinter")
        (about: "Private Sabre Service for Splinter")
        (@arg verbose: -v +multiple "Log verbosely")
    )
    .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::Level::Warn,
        1 => log::Level::Info,
        _ => log::Level::Debug,
    };
    simple_logger::init_with_level(log_level)?;

    info!("Started Private Sabre Service");

    Ok(())
}
