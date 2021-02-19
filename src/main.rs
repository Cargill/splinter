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

use clap::clap_app;
#[cfg(any(feature = "workload", feature = "playlist"))]
use clap::{Arg, SubCommand};
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;
use std::ffi::OsString;

#[cfg(feature = "playlist")]
use crate::action::playlist;
#[cfg(feature = "workload")]
use crate::action::workload;
use crate::action::{Action, SubcommandActions};
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
    // Allowing unused_mut because app must be mutable if feature `workload` is
    // enabled
    #[allow(unused_mut)]
    let mut app = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (author: "Cargill")
        (about: "Command line for transact")
        (@arg verbose: -v +multiple +global "Log verbosely")
        (@arg quiet: -q --quiet +global "Do not display output")
        (@setting SubcommandRequiredElseHelp));

    #[cfg(feature = "workload")]
    {
        app = app.subcommand(
            SubCommand::with_name("workload")
                .about("Run a continuous workload against a set of targets")
                .arg(
                    Arg::with_name("targets")
                        .long("targets")
                        .takes_value(true)
                        .multiple(true)
                        .required(true)
                        .help("Node URLS to submit batches to, combine groups with ;"),
                )
                .arg(
                    Arg::with_name("target_rate")
                        .long("target-rate")
                        .takes_value(true)
                        .required(true)
                        .long_help(
                            "How many batches to submit per second, either provide a number or \
                     a range with the min and max separated by '-' ex: 5-15, default to 1",
                        ),
                )
                .arg(
                    Arg::with_name("key")
                        .value_name("private-key-file")
                        .short("k")
                        .long("key")
                        .takes_value(true)
                        .help("Path to private key file"),
                )
                .arg(
                    Arg::with_name("workload")
                        .long("workload")
                        .takes_value(true)
                        .required(true)
                        .possible_values(&["smallbank"])
                        .help("The workload to be submitted"),
                )
                .arg(
                    Arg::with_name("update")
                        .long("update")
                        .short("u")
                        .takes_value(true)
                        .help("The time in seconds between updates, defaults to 30 seconds"),
                )
                .arg(
                    Arg::with_name("smallbank_num_accounts")
                        .long("smallbank-num-accounts")
                        .value_name("ACCOUNTS")
                        .help("The number of smallbank accounts to make. Defaults to 100"),
                )
                .arg(
                    Arg::with_name("smallbank_seed")
                        .long("smallbank-seed")
                        .value_name("SEED")
                        .long_help(
                            "An integer to use as a seed to make the smallbank workload \
                        reproducible",
                        ),
                ),
        );
    }
    #[cfg(feature = "playlist")]
    {
        app = app.subcommand(
            SubCommand::with_name("playlist")
                .about("Create and process playlists of pregenerated payloads")
                .subcommand(
                    SubCommand::with_name("create")
                        .about(
                            "Generates a workload transaction playlist. \
                         A playlist is a series of transactions, described in \
                         YAML.  This command generates a playlist and writes it \
                         to file or standard out.",
                        )
                        .arg(
                            Arg::with_name("workload")
                                .long("workload")
                                .takes_value(true)
                                .required(true)
                                .possible_values(&["smallbank"])
                                .help("The workload type to create a playlist for"),
                        )
                        .arg(
                            Arg::with_name("output")
                                .short("o")
                                .long("output")
                                .value_name("FILE")
                                .help("The target for the generated playlist"),
                        )
                        .arg(
                            Arg::with_name("smallbank_num_accounts")
                                .long("smallbank-num-accounts")
                                .value_name("ACCOUNTS")
                                .help("The number of smallbank accounts to make. Defaults to 10"),
                        )
                        .arg(
                            Arg::with_name("smallbank_seed")
                                .long("smallbank-seed")
                                .value_name("SEED")
                                .long_help(
                                    "An integer to use as a seed generate the same smallbank \
                                    playlist",
                                ),
                        )
                        .arg(
                            Arg::with_name("transactions")
                                .short("n")
                                .long("transactions")
                                .value_name("NUMBER")
                                .required(true)
                                .help("The number of transactions to generate. Defaults to 10"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("process")
                        .about(
                            "Processes a transaction playlist. \
                     A playlist is a series of transactions, described in \
                     YAML.  This command processes a playlist, converting it into \
                     transactions and writes it to file or standard out.",
                        )
                        .arg(
                            Arg::with_name("input")
                                .short("i")
                                .long("input")
                                .value_name("FILE")
                                .required(true)
                                .help("The source of the input playlist yaml"),
                        )
                        .arg(
                            Arg::with_name("key")
                                .short("k")
                                .long("key")
                                .value_name("FILE")
                                .required(true)
                                .help("The signing key for the transactions"),
                        )
                        .arg(
                            Arg::with_name("output")
                                .short("o")
                                .long("output")
                                .value_name("FILE")
                                .help("The target for the generated transactions"),
                        )
                        .arg(
                            Arg::with_name("workload")
                                .long("workload")
                                .takes_value(true)
                                .required(true)
                                .possible_values(&["smallbank"])
                                .help("The workload to be submitted"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("batch")
                        .about(
                            "Generates signed batches from transaction input. \
                     The transaction input is expected to be length-delimited protobuf \
                     Transaction messages, which should also be pre-signed for \
                     submission to the validator.",
                        )
                        .arg(
                            Arg::with_name("input")
                                .short("i")
                                .long("input")
                                .value_name("FILE")
                                .required(true)
                                .help("The source of input transactions"),
                        )
                        .arg(
                            Arg::with_name("output")
                                .short("o")
                                .long("output")
                                .value_name("FILE")
                                .required(true)
                                .help("The target for the signed batches"),
                        )
                        .arg(
                            Arg::with_name("key")
                                .short("k")
                                .long("key")
                                .value_name("FILE")
                                .required(true)
                                .help("The signing key for the transactions"),
                        )
                        .arg(
                            Arg::with_name("max-batch-size")
                                .short("n")
                                .long("max-batch-size")
                                .value_name("NUMBER")
                                .help(
                                    "The maximum number of transactions to include in a batch; \
                             Defaults to 1.",
                                ),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("submit")
                        .about(
                            "Submits signed batches to one or more targets from batch input. \
                     The batch input is expected to be length-delimited protobuf \
                     Batch messages, which should also be pre-signed for \
                     submission to the validator.",
                        )
                        .arg(
                            Arg::with_name("target")
                                .long("target")
                                .takes_value(true)
                                .required(true)
                                .help("Node URLS to submit batches to, combine mulitple with ;"),
                        )
                        .arg(
                            Arg::with_name("rate")
                                .short("r")
                                .long("rate")
                                .value_name("RATE")
                                .long_help(
                                    "The number of batches per second to submit to the target, \
                                defaults to 1",
                                ),
                        )
                        .arg(
                            Arg::with_name("key")
                                .value_name("private-key-file")
                                .short("k")
                                .long("key")
                                .takes_value(true)
                                .help("Path to private key file"),
                        )
                        .arg(
                            Arg::with_name("input")
                                .short("i")
                                .long("input")
                                .value_name("FILE")
                                .help("The source of batch transactions"),
                        )
                        .arg(
                            Arg::with_name("update")
                                .long("update")
                                .short("u")
                                .takes_value(true)
                                .help(
                                    "The time in seconds between updates, defaults to 30 seconds",
                                ),
                        ),
                ),
        );
    }

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

    // needs to be mut if workload or playlist is enabled
    #[allow(unused_mut)]
    let mut subcommands = SubcommandActions::new();

    #[cfg(feature = "workload")]
    {
        subcommands = subcommands.with_command("workload", workload::WorkloadAction)
    }

    #[cfg(feature = "playlist")]
    {
        subcommands = subcommands.with_command(
            "playlist",
            SubcommandActions::new()
                .with_command("create", playlist::CreatePlaylistAction)
                .with_command("process", playlist::ProcessPlaylistAction)
                .with_command("submit", playlist::SubmitPlaylistAction)
                .with_command("batch", playlist::BatchPlaylistAction),
        );
    }

    subcommands.run(Some(&matches))?;
    Ok(())
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
