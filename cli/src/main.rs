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

#[macro_use]
extern crate log;
#[cfg(feature = "database")]
extern crate diesel;

mod action;
mod error;

use crate::error::CliError;
use action::{admin, certs, Action, SubcommandActions};

use clap::clap_app;
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(any(feature = "health", feature = "nodes"))]
const DEFAULT_SPLINTER_NODE_URL: &str = "http://localhost:8085";

// log format for cli that will only show the log message
pub fn log_format(
    w: &mut dyn std::io::Write,
    _now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(w, "{}", record.args(),)
}

fn run() -> Result<(), CliError> {
    // ignore unused_mut while there are experimental features
    #[allow(unused_mut)]
    let mut app = clap_app!(myapp =>
        (name: APP_NAME)
        (version: VERSION)
        (author: "Cargill")
        (about: "Command line for Splinter")
        (@arg verbose: -v +multiple "Log verbosely")
        (@setting SubcommandRequiredElseHelp)
        (@subcommand admin =>
            (about: "Administrative commands")
            (@subcommand keygen =>
                (about: "Generates secp256k1 keys to use when signing circuit proposals")
                (@arg key_name: +takes_value "Name of the key to create; defaults to \"splinter\"")
                (@arg key_dir: -d --("key-dir") +takes_value
                 "Name of the directory in which to create the keys; defaults to current working directory")
                (@arg force: --force "Overwrite files if they exist")
                (@arg quiet: -q --quiet "Do not display output")
            )
            (@subcommand keyregistry =>
                (about: "Generates a key registry yaml file and keys, based on a registry \
                 specification")
                (@arg target_dir: -d --("target-dir") +takes_value
                 "Name of the directory in which to create the registry file and keys; \
                 defaults to /var/lib/splinter or the value of SPLINTER_STATE_DIR environment \
                 variable")
                (@arg registry_file: -o --("registry-file") +takes_value
                 "Name of the target registry file (in the target directory); \
                 defaults to \"keys.yaml\"")
                (@arg registry_spec_path: -i --("input-registry-spec") +takes_value
                 "Name of the input key registry specification; \
                 defaults to \"./key_registry_spec.yaml\"")
                (@arg force: --force "Overwrite files if they exist")
                (@arg quiet: -q --quiet "Do not display output")
            )
        )
        (@subcommand cert =>
            (about: "Generate certificates that can be used for development")
            (@subcommand generate =>
                (about: "Generate certificates and keys for the ca, server and client")
                (@arg common_name: --("common-name") +takes_value
                  "The common name that should be used in the generated cert, default localhost")
                (@arg cert_dir: -d --("cert-dir") +takes_value
                  "Name of the directory in which to create the certificates")
                (@arg force: --force  conflicts_with[skip] "Overwrite files if they exist")
                (@arg skip: --skip conflicts_with[force] "Check if files exists, generate if missing")
                (@arg quiet: -q --quiet "Do not display output")
            )
        )
    );

    #[cfg(feature = "health")]
    {
        use clap::{Arg, SubCommand};

        app = app.subcommand(
            SubCommand::with_name("health")
                .about("Displays information about network health")
                .subcommand(
                    SubCommand::with_name("status")
                        .about(
                            "Displays a node's version, endpoint, node id, and a list\n\
                             of endpoints of its connected peers",
                        )
                        .arg(
                            Arg::with_name("url")
                                .short("U")
                                .takes_value(true)
                                .default_value(DEFAULT_SPLINTER_NODE_URL)
                                .help("URL of node"),
                        ),
                ),
        );
    }

    #[cfg(feature = "database")]
    {
        use clap::{Arg, SubCommand};

        app = app.subcommand(
            SubCommand::with_name("database")
                .about("Database commands")
                .subcommand(
                    SubCommand::with_name("migrate")
                        .about("Runs database migrations for the enabled Splinter features")
                        .arg(
                            Arg::with_name("connect")
                                .short("C")
                                .takes_value(true)
                                .help("Database connection URI"),
                        ),
                ),
        )
    }

    #[cfg(feature = "nodes")]
    {
        use action::nodes;
        use clap::{Arg, SubCommand};

        app = app.subcommand(
            SubCommand::with_name("nodes")
                .about("Interact with a node registry")
                .arg(
                    Arg::with_name(nodes::URL_ARG)
                        .long("url")
                        .short("U")
                        .global(true)
                        .takes_value(true)
                        .default_value(DEFAULT_SPLINTER_NODE_URL)
                        .help("Splinter node's endpoint"),
                )
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List all nodes in the registry")
                        .alias("ls")
                        .arg(
                            Arg::with_name(nodes::FORMAT_ARG)
                                .long("format")
                                .takes_value(true)
                                .default_value(nodes::YAML)
                                .possible_values(nodes::SUPPORTED_FORMATS)
                                .help("Format nodes will be displayed in"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("show")
                        .about("Show a single node from the registry")
                        .arg(
                            Arg::with_name("identity")
                                .takes_value(true)
                                .required(true)
                                .help("Identity of the node to show"),
                        )
                        .arg(
                            Arg::with_name(nodes::FORMAT_ARG)
                                .long("format")
                                .takes_value(true)
                                .default_value(nodes::YAML)
                                .possible_values(nodes::SUPPORTED_FORMATS)
                                .help("Format nodes will be displayed in"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("add")
                        .about("Add nodes to the registry, read from a JSON or YAML file")
                        .arg(
                            Arg::with_name(nodes::FILE_ARG)
                                .takes_value(true)
                                .required(true)
                                .help("Path to JSON or YAML file with node definition(s)"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("update")
                        .about(
                            "Update one or more nodes in the registry with definitions from a \
                            JSON or YAML file",
                        )
                        .arg(Arg::with_name(nodes::REPLACE_ALL_ARG).long("replace-all").help(
                            "If specified, the entire node registry will be cleared and replaced \
                            by the nodes in the file",
                        ))
                        .arg(
                            Arg::with_name(nodes::FILE_ARG)
                                .takes_value(true)
                                .required(true)
                                .help("Path to JSON or YAML file with node definition(s)"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("Remove one or more nodes from the registry")
                        .alias("rm")
                        .arg(
                            Arg::with_name(nodes::IDENTITIES_ARG)
                                .takes_value(true)
                                .multiple(true)
                                .required(true)
                                .help("Identities of the nodes to remove"),
                        ),
                ),
        );
    }

    let matches = app.get_matches();

    // set default to info
    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let mut log_spec_builder = LogSpecBuilder::new();
    log_spec_builder.default(log_level);

    let mut logger_handle = Logger::with(log_spec_builder.build())
        .format(log_format)
        .start()
        .expect("Failed to create logger");

    let mut subcommands = SubcommandActions::new()
        .with_command(
            "admin",
            SubcommandActions::new()
                .with_command("keygen", admin::KeyGenAction)
                .with_command("keyregistry", admin::KeyRegistryGenerationAction),
        )
        .with_command(
            "cert",
            SubcommandActions::new().with_command("generate", certs::CertGenAction),
        );

    #[cfg(feature = "health")]
    {
        use action::health;
        subcommands = subcommands.with_command(
            "health",
            SubcommandActions::new().with_command("status", health::StatusAction),
        );
    }

    #[cfg(feature = "database")]
    {
        use action::database;
        subcommands = subcommands.with_command(
            "database",
            SubcommandActions::new().with_command("migrate", database::MigrateAction),
        )
    }

    #[cfg(feature = "nodes")]
    {
        use action::nodes;
        subcommands = subcommands.with_command(
            "nodes",
            SubcommandActions::new()
                .with_command("list", nodes::ListAction)
                .with_command("show", nodes::ShowAction)
                .with_command("add", nodes::AddAction)
                .with_command("update", nodes::UpdateAction)
                .with_command("remove", nodes::RemoveAction),
        );
    }

    subcommands.run(Some(&matches), &mut logger_handle)
}

fn main() {
    if let Err(e) = run() {
        error!("ERROR: {}", e);
        std::process::exit(1);
    }
}
