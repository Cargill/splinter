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
extern crate serde_json;
#[macro_use]
extern crate log;

mod actions;

use clap::{clap_app, crate_version};
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;

use actions::{add_connection, list_connections, remove_connection};

pub fn log_format(
    w: &mut dyn std::io::Write,
    _now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(w, "{}", record.args(),)
}

fn main() {
    let matches = clap_app!(cmc =>
        (version: crate_version!())
        (about: "Connection manager client (cmc)")
        (@arg url: +takes_value "Rest api address")
        (@arg verbose: -v --verbose +multiple "Increase output verbosity")
        (@setting SubcommandRequiredElseHelp)
        (@subcommand connection =>
            (@setting SubcommandRequiredElseHelp)
            (about: "Add, remove, and list connections")
            (@subcommand add =>
                (about: "Add a new connection")
                (@arg address: +takes_value +required "Node address you want to add as a connection")
            )
            (@subcommand remove =>
                (about: "remove connection")
                (@arg address: +takes_value +required "Node address you want to add as a connection")
            )
            (@subcommand list =>
                (about: "list active connections")
            )
         )
    ).get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let mut log_spec_builder = LogSpecBuilder::new();
    log_spec_builder.default(log_level);
    log_spec_builder.module("hyper", log::LevelFilter::Warn);
    log_spec_builder.module("tokio", log::LevelFilter::Warn);

    Logger::with(log_spec_builder.build())
        .format(log_format)
        .start()
        .expect("Failed to create logger");

    let url = matches.value_of("url").unwrap_or("http://localhost:3030");

    match matches.subcommand() {
        ("connection", Some(m)) => match m.subcommand() {
            ("add", Some(m)) => add_connection(url, m.value_of("address").unwrap()),
            ("remove", Some(m)) => remove_connection(url, m.value_of("address").unwrap()),
            ("list", Some(_)) => list_connections(url),
            _ => panic!("Unknown command"),
        },
        _ => panic!("Unknown command"),
    }
}
