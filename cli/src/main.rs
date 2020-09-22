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

#[macro_use]
extern crate log;
#[cfg(feature = "database")]
extern crate diesel;

mod action;
mod error;
#[cfg(test)]
mod tests;

#[cfg(feature = "circuit-template")]
mod template;

use std::ffi::OsString;

use clap::{clap_app, AppSettings, Arg, SubCommand};
#[cfg(test)]
use flexi_logger::FlexiLoggerError;
use flexi_logger::{DeferredNow, LogSpecBuilder, Logger};
use log::Record;

use action::{admin, certs, circuit, keygen, registry, Action, SubcommandActions};
use error::CliError;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const CIRCUIT_PROPOSE_AFTER_HELP: &str = r"DETAILS:
    One or more nodes must be specified using the --node and/or --node-file arguments. These
    arguments can be used on their own or together, but at least one of them is required.

    The --node-file argument must be a valid YAML file. A valid YAML file will be a list of nodes,
    where each node has an 'identity' or 'node_id' field, as well as an 'endpoints' field. Example:
        ---
        - identity: 'node-1'
          endpoints:
            - tcps://node-1-endpoint:8044
        - node_id: 'node-2'
          endpoints:
            - tcps://node-2-endpoint:8045

    For the --service-arg, --service-peer-group, and --service-type options, service IDs can be
    wildcarded with '*' to match multiple services. For example, '--service-type *::scabbard' match
    all services, and '--service-type sc*::scabbard' will match all services with IDs that start
    with 'sc'.

    With '--metadata-encoding string' (the default), the --metadata option takes a single string
    value and the --metadata option can be used only once. With '--metadata-encoding json', the
    --metadata option takes a key/value pair in the format '<key>=<value>', where '<value>' is a
    simple string, a JSON array, or a JSON object; the --metadata option can be used multiple times
    with JSON encoding.";

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
        (about: "Command line for Splinter")
        (@arg verbose: -v +multiple +global "Log verbosely")
        (@arg quiet: -q --quiet +global "Do not display output")
        (@setting SubcommandRequiredElseHelp)
        (@subcommand admin =>
            (about: "Administrative commands")
            (@setting SubcommandRequiredElseHelp)
            (@subcommand keygen =>
                (about: "Generates secp256k1 keys to use when signing circuit proposals")
                (@arg key_name: +takes_value "Name of the key to create; defaults to \"splinter\"")
                (@arg key_dir: -d --("key-dir") +takes_value
                 "Name of the directory in which to create the keys; defaults to current working directory")
                (@arg force: --force "Overwrite files if they exist")
            )
        )
    );

    app = app
        .subcommand(
            SubCommand::with_name("cert")
                .about("Generates certificates that can be used for development")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("generate")
                        .long_about(
                            "Generates test certificates and keys for running splinterd with \
                         TLS (in insecure mode)",
                        )
                        .arg(
                            Arg::with_name("common_name")
                                .long("common-name")
                                .takes_value(true)
                                .long_help(
                                    "String that specifies a common name for the generated \
                             certificate (defaults to localhost). Use this option if the \
                             splinterd URL uses a DNS address instead of a numerical IP \
                             address.",
                                ),
                        )
                        .arg(
                            Arg::with_name("cert_dir")
                                .long("cert-dir")
                                .short("d")
                                .takes_value(true)
                                .long_help(
                                    "Path to the directory certificates are created in. \
                             Defaults to /etc/splinter/certs/. This location can also be \
                             changed with the SPLINTER_CERT_DIR environment variable. \
                             This directory must exist.
                        ",
                                ),
                        )
                        .arg(
                            Arg::with_name("force")
                                .long("force")
                                .conflicts_with("skip")
                                .long_help(
                                    "Overwrites files if they exist. If this flag is not \
                            provided and the file exists, an error is returned.
                        ",
                                ),
                        )
                        .arg(
                            Arg::with_name("skip")
                                .long("skip")
                                .conflicts_with("force")
                                .long_help(
                                    "Checks if the files exists and generates the files that \
                             are missing. If this flag is not \
                             provided and the file exists, an error is returned.",
                                ),
                        )
                        .after_help(
                            "DETAILS: \n\n\
                    The files are generated in the location specified by --cert-dir, the \
                    SPLINTER_CERT_DIR environment variable, or in the default location \
                     /etc/splinter/certs/. \n\n\
                    The following files are created: \n    \
                        - client.crt \n    \
                        - client.key \n    \
                        - server.crt \n    \
                        - server.key \n    \
                        - generated_ca.pem \n    \
                        - generated_ca.key
                                    ",
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("keygen")
                .about("Generates secp256k1 keys")
                .arg(
                    Arg::with_name("key-name")
                        .takes_value(true)
                        .help("Name of keys generated; defaults to user name"),
                )
                .arg(
                    Arg::with_name("key_dir")
                        .long("key-dir")
                        .takes_value(true)
                        .conflicts_with("system")
                        .help(
                            "Name of the directory in which to create the keys; defaults to \
                             $HOME/splinter/keys",
                        ),
                )
                .arg(
                    Arg::with_name("force")
                        .short("f")
                        .long("force")
                        .help("Overwrite files if they exist"),
                )
                .arg(
                    Arg::with_name("system")
                        .long("system")
                        .help("Generate system keys in /etc/splinter/keys"),
                ),
        );

    let propose_circuit = SubCommand::with_name("propose")
        .about("Propose that a new circuit is created")
        .arg(
            Arg::with_name("url")
                .short("U")
                .long("url")
                .takes_value(true)
                .help("URL of Splinter Daemon"),
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
            Arg::with_name("node_file")
                .long("node-file")
                .takes_value(true)
                .required_unless("node")
                .help("File system path or HTTP(S) URL to nodes file"),
        )
        .arg(
            Arg::with_name("node")
                .long("node")
                .takes_value(true)
                .required_unless("node_file")
                .multiple(true)
                .help(
                    "Node that is part of a circuit \
                     (<node_id>::<endpoint1>,<endpoint2>)",
                ),
        )
        .arg(
            Arg::with_name("service")
                .long("service")
                .takes_value(true)
                .multiple(true)
                .min_values(2)
                .required_unless("template")
                .help(
                    "Service ID and allowed nodes \
                     (<service-id>::<allowed_nodes>)",
                ),
        )
        .arg(
            Arg::with_name("service_argument")
                .long("service-arg")
                .takes_value(true)
                .multiple(true)
                .help(
                    "Pass arguments to a service \
                     (<service_id>::<key>=<value>)",
                ),
        )
        .arg(
            Arg::with_name("service_peer_group")
                .long("service-peer-group")
                .takes_value(true)
                .multiple(true)
                .help("List of peer services (comma-separated list)"),
        )
        .arg(
            Arg::with_name("management_type")
                .long("management")
                .takes_value(true)
                .help("Management type for the circuit"),
        )
        .arg(
            Arg::with_name("service_type")
                .long("service-type")
                .takes_value(true)
                .multiple(true)
                .help(
                    "Service type \
                     (<service_id>::<service_type>)",
                ),
        )
        .arg(
            Arg::with_name("metadata")
                .long("metadata")
                .value_name("application_metadata")
                .takes_value(true)
                .multiple(true)
                .help("Application metadata of the proposal"),
        )
        .arg(
            Arg::with_name("metadata_encoding")
                .long("metadata-encoding")
                .takes_value(true)
                .possible_values(&["json", "string"])
                .requires("metadata")
                .help(
                    "Set encoding of application metadata \
                       (default: string)",
                ),
        )
        .arg(
            Arg::with_name("comments")
                .long("comments")
                .takes_value(true)
                .help("Add human-readable comments to the proposal"),
        )
        .arg(
            Arg::with_name("dry_run")
                .long("dry-run")
                .short("n")
                .help("Print circuit definition without submitting the proposal"),
        )
        .after_help(CIRCUIT_PROPOSE_AFTER_HELP);

    #[cfg(feature = "circuit-auth-type")]
    let propose_circuit = propose_circuit.arg(
        Arg::with_name("authorization_type")
            .long("auth-type")
            .possible_values(&["trust"])
            .default_value("trust")
            .takes_value(true)
            .help("Authorization type for the circuit"),
    );

    #[cfg(feature = "circuit-template")]
    let propose_circuit = propose_circuit
        .arg(
            Arg::with_name("template")
                .long("template")
                .takes_value(true)
                .required_unless("service")
                .help("Template name to be applied to circuit"),
        )
        .arg(
            Arg::with_name("template_arg")
                .long("template-arg")
                .multiple(true)
                .takes_value(true)
                .requires("template")
                .help(
                    "Arguments for the template argument \
                     (<key>=<value>)",
                ),
        );

    let circuit_command = SubCommand::with_name("circuit")
        .about("Provides circuit management functionality")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(propose_circuit)
        .subcommand(
            SubCommand::with_name("vote")
                .about("Vote on a new circuit proposal")
                .arg(
                    Arg::with_name("url")
                        .short("U")
                        .long("url")
                        .takes_value(true)
                        .help("URL of Splinter Daemon"),
                )
                .arg(
                    Arg::with_name("private_key_file")
                        .value_name("private-key-file")
                        .short("k")
                        .long("key")
                        .takes_value(true)
                        .help("Path to private key file"),
                )
                .arg(
                    Arg::with_name("circuit_id")
                        .value_name("circuit-id")
                        .takes_value(true)
                        .required(true)
                        .help("ID of the proposed circuit"),
                )
                .arg(
                    Arg::with_name("accept")
                        .required(true)
                        .long("accept")
                        .conflicts_with("reject")
                        .help("Accept the proposal"),
                )
                .arg(
                    Arg::with_name("reject")
                        .required(true)
                        .long("reject")
                        .conflicts_with("accept")
                        .help("Reject the proposal"),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List the circuits")
                .arg(
                    Arg::with_name("url")
                        .short("U")
                        .long("url")
                        .help("URL of the Splinter daemon REST API")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("member")
                        .short("m")
                        .long("member")
                        .help("Filter circuits by a node ID in the member list")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("format")
                        .short("f")
                        .long("format")
                        .help("Output format")
                        .possible_values(&["human", "csv"])
                        .default_value("human")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("show")
                .about("Show a specific circuit or proposal")
                .arg(
                    Arg::with_name("url")
                        .short("U")
                        .long("url")
                        .help("URL of the Splinter daemon REST API")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("circuit")
                        .help("ID of the circuit to be shown")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("format")
                        .short("f")
                        .long("format")
                        .help("Output format")
                        .possible_values(&["human", "yaml", "json"])
                        .default_value("human")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("proposals")
                .about("List the circuit proposals")
                .arg(
                    Arg::with_name("url")
                        .short("U")
                        .long("url")
                        .help("URL of the Splinter daemon REST API")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("management_type")
                        .long("management-type")
                        .help(
                            "Filter circuit proposals by circuit \
                             management type",
                        )
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("member")
                        .long("member")
                        .help(
                            "Show proposals with the given node ID in \
                            its member list",
                        )
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("format")
                        .short("f")
                        .long("format")
                        .help("Output format")
                        .possible_values(&["human", "csv"])
                        .default_value("human")
                        .takes_value(true),
                ),
        );

    #[cfg(feature = "circuit-template")]
    let circuit_command = circuit_command.subcommand(
        SubCommand::with_name("template")
            .about("Manage circuit templates")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("list")
                    .about("List available templates")
                    .arg(
                        Arg::with_name("format")
                            .short("F")
                            .long("format")
                            .help("Output format")
                            .possible_values(&["human", "csv"])
                            .default_value("human")
                            .takes_value(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("show").about("Show a template").arg(
                    Arg::with_name("name")
                        .required(true)
                        .takes_value(true)
                        .value_name("name")
                        .help("Name of template"),
                ),
            )
            .subcommand(
                SubCommand::with_name("arguments")
                    .about("List arguments of a template")
                    .arg(
                        Arg::with_name("name")
                            .required(true)
                            .takes_value(true)
                            .value_name("name")
                            .help("Name of template"),
                    ),
            ),
    );

    app = app.subcommand(circuit_command);

    app = app.subcommand(
        SubCommand::with_name("registry")
            .about("Splinter registry commands")
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .subcommand(
                SubCommand::with_name("build")
                    .about("Add a node to a YAML file")
                    .arg(Arg::with_name("file").long("file").takes_value(true).help(
                        "Path of registry file to add node to; defaults to \
                                './nodes.yaml'",
                    ))
                    .arg(
                        Arg::with_name("force")
                            .long("force")
                            .help("Overwrite node if it already exists"),
                    )
                    .arg(
                        Arg::with_name("status_url")
                            .takes_value(true)
                            .help("URL of splinter REST API to query for node data"),
                    )
                    .arg(
                        Arg::with_name("key_files")
                            .long("key-file")
                            .takes_value(true)
                            .multiple(true)
                            .required(true)
                            .help("Path of public key file to include with node"),
                    )
                    .arg(
                        Arg::with_name("metadata")
                            .long("metadata")
                            .takes_value(true)
                            .multiple(true)
                            .help("Metadata to include with node (<key>=<value>)"),
                    ),
            ),
    );

    #[cfg(feature = "health")]
    {
        app = app.subcommand(
            SubCommand::with_name("health")
                .about("Displays information about network health")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("status")
                        .about(
                            "Displays a node's version, endpoint, node id, and a list\n\
                             of endpoints of its connected peers",
                        )
                        .arg(
                            Arg::with_name("url")
                                .short("U")
                                .long("url")
                                .help("URL of the Splinter daemon REST API")
                                .takes_value(true),
                        ),
                ),
        );
    }

    #[cfg(feature = "database")]
    {
        app = app.subcommand(
            SubCommand::with_name("database")
                .about("Database commands")
                .setting(AppSettings::SubcommandRequiredElseHelp)
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
        #[cfg(test)]
        // `FlexiLoggerError::Log` means the logger has already been initialized; this will happen
        // when `run` is called more than once in the tests.
        Err(FlexiLoggerError::Log(_)) => {}
        Err(err) => panic!("Failed to start logger: {}", err),
    }

    let mut subcommands = SubcommandActions::new()
        .with_command(
            "admin",
            SubcommandActions::new().with_command("keygen", admin::AdminKeyGenAction),
        )
        .with_command(
            "cert",
            SubcommandActions::new().with_command("generate", certs::CertGenAction),
        )
        .with_command("keygen", keygen::KeyGenAction);

    let circuit_command = SubcommandActions::new()
        .with_command("propose", circuit::CircuitProposeAction)
        .with_command("vote", circuit::CircuitVoteAction)
        .with_command("list", circuit::CircuitListAction)
        .with_command("show", circuit::CircuitShowAction)
        .with_command("proposals", circuit::CircuitProposalsAction);

    #[cfg(feature = "circuit-template")]
    let circuit_command = circuit_command.with_command(
        "template",
        SubcommandActions::new()
            .with_command("list", circuit::template::ListCircuitTemplates)
            .with_command("show", circuit::template::ShowCircuitTemplate)
            .with_command("arguments", circuit::template::ListCircuitTemplateArguments),
    );

    subcommands = subcommands.with_command("circuit", circuit_command);

    subcommands = subcommands.with_command(
        "registry",
        SubcommandActions::new().with_command("build", registry::RegistryGenerateAction),
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

    subcommands.run(Some(&matches))
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
