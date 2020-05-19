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
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate clap;

mod config;
mod daemon;
mod error;
mod routes;
mod transport;

use flexi_logger::{style, DeferredNow, LogSpecBuilder, Logger};
use log::Record;
use rand::{thread_rng, Rng};

use crate::config::{
    ClapPartialConfigBuilder, Config, ConfigBuilder, ConfigError, DefaultPartialConfigBuilder,
    EnvPartialConfigBuilder, PartialConfigBuilder, TomlPartialConfigBuilder,
};
use crate::daemon::SplinterDaemonBuilder;
use clap::{clap_app, crate_version};
use clap::{Arg, ArgMatches};

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::thread;

use error::UserError;
use transport::build_transport;

fn create_config(_toml_path: Option<&str>, _matches: ArgMatches) -> Result<Config, UserError> {
    let mut builder = ConfigBuilder::new();

    let clap_config = ClapPartialConfigBuilder::new(_matches).build()?;
    builder = builder.with_partial_config(clap_config);

    if let Some(file) = _toml_path {
        debug!("Loading config toml file: {:?}", fs::canonicalize(file)?);
        let toml_string = fs::read_to_string(file).map_err(|err| ConfigError::ReadError {
            file: String::from(file),
            err,
        })?;
        let toml_config = TomlPartialConfigBuilder::new(toml_string, String::from(file))
            .map_err(UserError::ConfigError)?
            .build()?;
        builder = builder.with_partial_config(toml_config);
    }

    let env_config = EnvPartialConfigBuilder::new().build()?;
    builder = builder.with_partial_config(env_config);

    let default_config = DefaultPartialConfigBuilder::new().build()?;
    builder = builder.with_partial_config(default_config);

    builder
        .build()
        .map_err(|e| UserError::MissingArgument(e.to_string()))
}

// Checks whether there is a saved node_id file. If there is, the config node_id must match
// the node_id in the file, otherwise we will return an error.
fn find_node_id(config: &Config) -> Result<String, UserError> {
    let node_id_path = Path::new(config.state_dir()).join("node_id");

    // Check if node file exists
    if node_id_path.exists() {
        // If the node file exists, read the node_id within the file.
        let mut file_node_id = fs::read_to_string(&node_id_path).map_err(|err| {
            UserError::io_err_with_source("Unable to read node_id file", Box::new(err))
        })?;
        if file_node_id.ends_with('\n') {
            file_node_id.pop();
        }
        match config.node_id() {
            // If the config has a node_id, check if this matches the node_id read from the file.
            Some(config_node_id) => {
                if config_node_id != file_node_id {
                    // If the node_id from the config object and the file do not match,
                    // return an error.
                    Err(UserError::InvalidArgument(format!(
                        "node_id from file {} does not match node_id from config {}",
                        file_node_id, config_node_id
                    )))
                } else {
                    // If the node_id does match, then we return this node_id and continue.
                    Ok(config_node_id.to_string())
                }
            }
            None => {
                // If the config object does not have a node_id, continue with the node_id read
                // from the file.
                Ok(file_node_id)
            }
        }
    } else {
        // If node file does not exist, need to create and save a node_id file.
        // Check if the config obejct has a node_id, otherwise generate a random one.
        let node_id = config
            .node_id()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("n{}", thread_rng().gen::<u16>().to_string()));
        let mut file = File::create(&node_id_path).map_err(|err| {
            UserError::io_err_with_source(
                &format!("Unable to create node_id file {:?}", &node_id_path),
                Box::new(err),
            )
        })?;
        file.write_all(&node_id.as_bytes()).map_err(|err| {
            UserError::io_err_with_source(
                &format!("Unable to write node_id file {:?}", &node_id_path),
                Box::new(err),
            )
        })?;
        // Append newline to file
        writeln!(file).map_err(|err| {
            UserError::io_err_with_source(
                &format!("Unable to write to node_id file {:?}", &node_id_path),
                Box::new(err),
            )
        })?;

        // Continue with node_id
        Ok(node_id)
    }
}

// format for logs
pub fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    let level = record.level();
    write!(
        w,
        "[{}] T[{:?}] {} [{}] {}",
        now.now().format("%Y-%m-%d %H:%M:%S%.3f"),
        thread::current().name().unwrap_or("<unnamed>"),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        style(level, &record.args()),
    )
}

fn main() {
    let app = clap_app!(splinterd =>
        (version: crate_version!())
        (about: "Splinter Daemon")
        (@arg config: -c --config +takes_value)
        (@arg node_id: --("node-id") +takes_value
          "Unique ID for the node ")
        (@arg display_name: --("display-name") +takes_value
          "Human-readable name for the node")
        (@arg storage: --("storage") +takes_value
          "Storage type used for the node; defaults to yaml")
        (@arg no_tls:  --("no-tls") "Turn off tls configuration")
        (@arg registry_auto_refresh: --("registry-auto-refresh") +takes_value
            "How often remote Splinter registries should attempt to fetch upstream changes in the \
             background (in seconds); default is 600 (10 minutes), 0 means off")
        (@arg registry_forced_refresh: --("registry-forced-refresh") +takes_value
            "How long before remote Splinter registries should fetch upstream changes when read \
             (in seconds); default is 10, 0 means off")
        (@arg admin_timeout: --("admin-timeout") +takes_value
            "The coordinator timeout for admin service proposals (in seconds); default is \
             30 seconds")
        (@arg verbose: -v --verbose +multiple
          "Increase output verbosity"));

    let app = app
        .arg(
            Arg::with_name("advertised_endpoints")
                .long("advertised-endpoints")
                .short("a")
                .long_help("Publicly-visible network endpoints")
                .takes_value(true)
                .multiple(true)
                .alias("advertised-endpoint"),
        )
        .arg(
            Arg::with_name("heartbeat")
                .long("heartbeat")
                .long_help(
                    "How often heartbeat should be sent, in seconds; defaults to 30 seconds,\
                 0 means off",
                )
                .takes_value(true),
        )
        .arg(
            Arg::with_name("config_dir")
                .long("config-dir")
                .help("Path to the directory containing configuration files")
                .takes_value(true)
                .alias("config-dir"),
        )
        .arg(
            Arg::with_name("network_endpoints")
                .long("network-endpoints")
                .short("n")
                .long_help("Endpoints to connect to the network, protocol-prefix://ip:port")
                .takes_value(true)
                .multiple(true)
                .alias("network-endpoint"),
        )
        .arg(
            Arg::with_name("service_endpoint")
                .long("service-endpoint")
                .long_help("Endpoint that service will connect to, tcp://ip:port")
                .takes_value(true)
                .hidden(!cfg!(feature = "service-endpoint")),
        )
        .arg(
            Arg::with_name("rest_api_endpoint")
                .long("rest-api-endpoint")
                .help("Connection endpoint for REST API")
                .takes_value(true)
                .alias("bind"),
        )
        .arg(
            Arg::with_name("peers")
                .long("peers")
                .help("Endpoint that service will connect to, protocol-prefix://ip:port")
                .takes_value(true)
                .multiple(true)
                .alias("peer"),
        )
        .arg(
            Arg::with_name("registries")
                .long("registries")
                .help("Read-only Splinter registries")
                .takes_value(true)
                .multiple(true)
                .alias("registry"),
        )
        .arg(
            Arg::with_name("tls_cert_dir")
                .long("tls-cert-dir")
                .help("Path to the directory where the certificates and keys are")
                .takes_value(true)
                .alias("cert-dir"),
        )
        .arg(
            Arg::with_name("tls_ca_file")
                .long("tls-ca-file")
                .help("File path to the trusted CA certificate")
                .takes_value(true)
                .alias("ca-file"),
        )
        .arg(
            Arg::with_name("tls_client_cert")
                .long("tls-client-cert")
                .help("File path to the certificate for the node when connecting to a node")
                .takes_value(true)
                .alias("client-cert"),
        )
        .arg(
            Arg::with_name("tls_client_key")
                .long("tls-client-key")
                .help("File path to the key for the node when connecting to a node as client")
                .takes_value(true)
                .alias("client-key"),
        )
        .arg(
            Arg::with_name("tls_server_cert")
                .long("tls-server-cert")
                .help("File path to the certificate for the node when connecting to a node")
                .takes_value(true)
                .alias("server-cert"),
        )
        .arg(
            Arg::with_name("tls_server_key")
                .long("tls-server-key")
                .help("File path to the key for the node when connecting to a node as server")
                .takes_value(true)
                .alias("server-key"),
        )
        .arg(
            Arg::with_name("tls_insecure")
                .long("tls-insecure")
                .help("If set to tls, should accept all peer certificates")
                .alias("insecure"),
        )
        .arg(
            Arg::with_name("state_dir")
                .long("state-dir")
                .help("Storage directory when storage is YAML")
                .takes_value(true),
        );

    #[cfg(feature = "database")]
    let app = app.arg(
        Arg::with_name("database")
            .long("database")
            .long_help("DB connection URL")
            .takes_value(true),
    );

    #[cfg(feature = "biome")]
    let app = app.arg(
        Arg::with_name("enable_biome")
            .long("enable-biome")
            .long_help("Enable the biome subsystem"),
    );

    #[cfg(feature = "rest-api-cors")]
    let app = app.arg(
        Arg::with_name("whitelist")
            .long("whitelist")
            .multiple(true)
            .required(false)
            .takes_value(true)
            .help("Whitelisted domains"),
    );

    let matches = app.get_matches();

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
        .log_target(flexi_logger::LogTarget::StdOut)
        .start()
        .expect("Failed to create logger");

    if let Err(err) = start_daemon(matches) {
        error!("Failed to start daemon, {}", err);
        std::process::exit(1);
    }
}

fn start_daemon(matches: ArgMatches) -> Result<(), UserError> {
    // get provided config file or search default location
    let config_file = matches
        .value_of("config")
        .unwrap_or("/etc/splinter/splinterd.toml");

    let config_file_path = if Path::new(&config_file).is_file() {
        Some(config_file)
    } else {
        None
    };

    let config = create_config(config_file_path, matches.clone())?;

    if config.no_tls() {
        for network_endpoint in config.network_endpoints() {
            if network_endpoint.starts_with("tcps://") {
                return Err(UserError::InvalidArgument(format!(
                    "TLS is disabled, thus endpoint {} is invalid",
                    network_endpoint,
                )));
            }
        }
    }

    let transport = build_transport(&config)?;

    let rest_api_endpoint = config.rest_api_endpoint();

    #[cfg(feature = "database")]
    let db_url = config.database();

    let admin_timeout = config.admin_timeout();

    config.log_as_debug();

    let node_id = find_node_id(&config)?;
    let display_name = config
        .display_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Node {}", &node_id));

    let mut daemon_builder = SplinterDaemonBuilder::new();

    daemon_builder = daemon_builder
        .with_state_dir(config.state_dir().to_string())
        .with_network_endpoints(config.network_endpoints().to_vec())
        .with_advertised_endpoints(config.advertised_endpoints().to_vec())
        .with_initial_peers(config.peers().to_vec())
        .with_node_id(node_id)
        .with_display_name(display_name)
        .with_rest_api_endpoint(String::from(rest_api_endpoint))
        .with_storage_type(String::from(config.storage()))
        .with_registries(config.registries().to_vec())
        .with_registry_auto_refresh(config.registry_auto_refresh())
        .with_registry_forced_refresh(config.registry_forced_refresh())
        .with_heartbeat(config.heartbeat())
        .with_admin_timeout(admin_timeout);

    #[cfg(feature = "service-endpoint")]
    {
        daemon_builder =
            daemon_builder.with_service_endpoint(String::from(config.service_endpoint()))
    }
    #[cfg(not(feature = "service-endpoint"))]
    {
        if matches.is_present("service_endpoint") {
            warn!(
                "--service-endpoint is an experimental feature.  It is enabled by building \
                splinterd with the features \"service-endpoint\" enabled"
            );
        }
    }

    #[cfg(feature = "database")]
    {
        daemon_builder = daemon_builder.with_db_url(Some(String::from(db_url)));
    }

    #[cfg(feature = "biome")]
    {
        daemon_builder = daemon_builder.enable_biome(config.enable_biome());
    }

    #[cfg(feature = "rest-api-cors")]
    {
        daemon_builder = daemon_builder.with_whitelist(config.whitelist().map(ToOwned::to_owned));
    }

    let mut node = daemon_builder.build().map_err(|err| {
        UserError::daemon_err_with_source("unable to build the Splinter daemon", Box::new(err))
    })?;
    node.start(transport)?;
    Ok(())
}
