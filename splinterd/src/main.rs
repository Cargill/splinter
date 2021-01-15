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
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "config-command-line")]
#[macro_use]
extern crate clap;

mod config;
mod daemon;
mod error;
mod registry_config;
mod routes;
mod transport;

use flexi_logger::{style, DeferredNow, LogSpecBuilder, Logger};
use log::Record;

#[cfg(feature = "config-command-line")]
use crate::config::ClapPartialConfigBuilder;
#[cfg(feature = "config-default")]
use crate::config::DefaultPartialConfigBuilder;
#[cfg(feature = "config-env-var")]
use crate::config::EnvPartialConfigBuilder;
#[cfg(feature = "default")]
use crate::config::PartialConfigBuilder;
#[cfg(feature = "config-toml")]
use crate::config::TomlPartialConfigBuilder;
use crate::config::{Config, ConfigBuilder, ConfigError};
use crate::daemon::SplinterDaemonBuilder;
use clap::{clap_app, crate_version};
use clap::{Arg, ArgMatches};

use std::env;
use std::fs;
use std::path::Path;
use std::thread;

use error::UserError;
use transport::get_transport;

fn create_config(_toml_path: Option<&str>, _matches: ArgMatches) -> Result<Config, UserError> {
    #[cfg(feature = "default")]
    let mut builder = ConfigBuilder::new();
    #[cfg(not(feature = "default"))]
    let builder = ConfigBuilder::new();

    #[cfg(feature = "config-command-line")]
    {
        let clap_config = ClapPartialConfigBuilder::new(_matches).build()?;
        builder = builder.with_partial_config(clap_config);
    }

    #[cfg(feature = "config-toml")]
    {
        if let Some(file) = _toml_path {
            let toml_string = fs::read_to_string(file).map_err(|err| ConfigError::ReadError {
                file: String::from(file),
                err,
            })?;
            let toml_config = TomlPartialConfigBuilder::new(toml_string, String::from(file))
                .map_err(UserError::ConfigError)?
                .build()?;
            builder = builder.with_partial_config(toml_config);
        }
    }

    #[cfg(feature = "config-env-var")]
    {
        let env_config = EnvPartialConfigBuilder::new().build()?;
        builder = builder.with_partial_config(env_config);
    }

    #[cfg(feature = "config-default")]
    {
        let default_config = DefaultPartialConfigBuilder::new().build()?;
        builder = builder.with_partial_config(default_config);
    }
    builder
        .build()
        .map_err(|e| UserError::MissingArgument(e.to_string()))
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
        (@arg storage: --("storage") +takes_value
          "Storage type used for the node; defaults to yaml")
        (@arg transport: --("transport") +takes_value
          "Transport type for sockets, either raw or tls")
        (@arg network_endpoint: -n --("network-endpoint") +takes_value
          "Endpoint to connect to the network, tcp://ip:port")
        (@arg service_endpoint: --("service-endpoint") +takes_value
          "Endpoint that service will connect to, tcp://ip:port")
        (@arg peers: --peer +takes_value +multiple
          "Endpoint that service will connect to, ip:port")
        (@arg ca_file: --("ca-file") +takes_value
          "File path to the trusted CA certificate")
        (@arg cert_dir: --("cert-dir") +takes_value
          "Path to the directory where the certificates and keys are")
        (@arg client_cert: --("client-cert") +takes_value
          "File path to the certificate for the node when connecting to a node")
        (@arg server_cert: --("server-cert") +takes_value
          "File path to the certificate for the node when connecting to a node")
        (@arg server_key:  --("server-key") +takes_value
          "File path to the key for the node when connecting to a node as server")
        (@arg client_key:  --("client-key") +takes_value
          "File path to the key for the node when connecting to a node as client")
        (@arg insecure:  --("insecure")
          "If set to tls, should accept all peer certificates")
        (@arg bind: --("bind") +takes_value
          "Connection endpoint for REST API")
        (@arg registry_backend: --("registry-backend") +takes_value
          "Backend type for the node registry. Possible values: FILE.")
        (@arg registry_file: --("registry-file") +takes_value
          "File path to the node registry file if registry-backend is FILE.")
        (@arg admin_service_coordinator_timeout: --("admin-timeout") +takes_value
            "The coordinator timeout for admin service proposals (in milliseconds); default is \
             30000 (30 seconds)")
        (@arg verbose: -v --verbose +multiple
          "Increase output verbosity"));

    let app = app.arg(
        Arg::with_name("heartbeat_interval")
            .long("heartbeat")
            .long_help(
                "How often heartbeat should be sent, in seconds; defaults to 30 seconds,\
                 0 means off",
            )
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
        Arg::with_name("biome_enabled")
            .long("enable-biome")
            .long_help("Enable the biome subsystem"),
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
        .start()
        .expect("Failed to create logger");

    if let Err(err) = start_daemon(matches) {
        error!("Failed to start daemon, {}", err);
        std::process::exit(1);
    }
}

fn start_daemon(matches: ArgMatches) -> Result<(), UserError> {
    debug!("Loading configuration file");

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

    let transport = get_transport(&config)?;

    let storage_location = match &config.storage() as &str {
        "yaml" => format!("{}{}", config.state_dir(), "circuits.yaml"),
        "memory" => "memory".to_string(),
        _ => {
            return Err(UserError::InvalidArgument(format!(
                "storage type is not supported: {}",
                config.storage()
            )))
        }
    };

    let key_registry_location = match &config.storage() as &str {
        "yaml" => format!("{}{}", config.state_dir(), "keys.yaml"),
        "memory" => "memory".to_string(),
        _ => {
            return Err(UserError::InvalidArgument(format!(
                "storage type is not supported: {}",
                config.storage()
            )))
        }
    };

    let rest_api_endpoint = config.bind();

    #[cfg(feature = "database")]
    let db_url = config.database();

    let registry_backend = config.registry_backend();

    let admin_service_coordinator_timeout = config.admin_service_coordinator_timeout();

    config.log_as_debug();

    let mut daemon_builder = SplinterDaemonBuilder::new()
        .with_storage_location(storage_location)
        .with_key_registry_location(key_registry_location)
        .with_network_endpoint(String::from(config.network_endpoint()))
        .with_service_endpoint(String::from(config.service_endpoint()))
        .with_initial_peers(config.peers().to_vec())
        .with_node_id(String::from(config.node_id()))
        .with_rest_api_endpoint(String::from(rest_api_endpoint))
        .with_storage_type(String::from(config.storage()))
        .with_heartbeat_interval(config.heartbeat_interval())
        .with_admin_service_coordinator_timeout(admin_service_coordinator_timeout);

    #[cfg(feature = "database")]
    {
        daemon_builder = daemon_builder.with_db_url(Some(String::from(db_url)));
    }

    #[cfg(feature = "biome")]
    {
        daemon_builder = daemon_builder.enable_biome(config.biome_enabled());
    }

    if Path::new(&config.registry_file()).is_file() && registry_backend == "FILE" {
        daemon_builder = daemon_builder
            .with_registry_backend(Some(String::from(registry_backend)))
            .with_registry_file(String::from(config.registry_file()));
    } else {
        daemon_builder = daemon_builder.with_registry_backend(None);
    }

    let mut node = daemon_builder.build().map_err(|err| {
        UserError::daemon_err_with_source("unable to build the Splinter daemon", Box::new(err))
    })?;
    node.start(transport)?;
    Ok(())
}
