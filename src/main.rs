// Copyright 2018-2021 Cargill Incorporated
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
extern crate log;

mod action;
pub mod error;

use std::ffi::OsString;

use clap::{clap_app, Arg, SubCommand};
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;

use crate::error::CliError;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

// log format for cli that will only show the log message
pub fn log_format(
    w: &mut dyn std::io::Write,
    _now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(w, "{}", record.args(),)
}

fn run<I: IntoIterator<Item = T>, T: Into<OsString> + Clone>(args: I) -> Result<(), CliError> {
    let mut app = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (author: "Cargill")
        (about: "Command line for transact")
        (@arg verbose: -v +multiple +global "Log verbosely")
        (@arg quiet: -q --quiet +global "Do not display output")
        (@setting SubcommandRequiredElseHelp));

    let matches = app.get_matches_from_safe(args)?;

    // set default to info
    let log_level = if matches.is_present("quiet") {
        log::LevelFilter::Error
    } else {
        match matches.occurrences_of("verbose") {
            0 => log::LevelFilter::Info,
            1 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    };

    let mut log_spec_builder = LogSpecBuilder::new();
    log_spec_builder.default(log_level);
    log_spec_builder.module("reqwest", log::LevelFilter::Warn);
    log_spec_builder.module("hyper", log::LevelFilter::Warn);
    log_spec_builder.module("mio", log::LevelFilter::Warn);
    log_spec_builder.module("want", log::LevelFilter::Warn);

    match Logger::with(log_spec_builder.build())
        .format(log_format)
        .log_target(flexi_logger::LogTarget::StdOut)
        .start()
    {
        Ok(_) => {}
        Err(err) => panic!("Failed to start logger: {}", err),
    }
}

fn main() {
    match run(std::env::args_os()) {
        Ok(_) => {}
        Err(CliError::ClapError(err)) => err.exit(),
        Err(e) => {
            error!("ERROR: {}", e);
            std::process::exit(1);
        }
    }
}
