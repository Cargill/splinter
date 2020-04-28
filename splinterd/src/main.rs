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
#[cfg(feature = "config-command-line")]
#[macro_use]
extern crate clap;

mod config;
mod daemon;
mod error;
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
use transport::build_transport;

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
        (@arg display_name: --("display-name") +takes_value
          "Human-readable name for the node")
        (@arg storage: --("storage") +takes_value
          "Storage type used for the node; defaults to yaml")
        (@arg network_endpoints: -n --("network-endpoint") +takes_value +multiple
          "Endpoints to connect to the network, protocol-prefix://ip:port")
        (@arg advertised_endpoints: -a --("advertised-endpoint") +takes_value +multiple
          "Publicly-visible network endpoints")
        (@arg service_endpoint: --("service-endpoint") +takes_value
          "Endpoint that service will connect to, tcp://ip:port")
        (@arg peers: --peer +takes_value +multiple
          "Endpoint that service will connect to, protocol-prefix://ip:port")
        (@arg no_tls:  --("no-tls") "Turn off tls configuration")
        (@arg bind: --("bind") +takes_value
          "Connection endpoint for REST API")
        (@arg registries: --("registry") +takes_value +multiple "Read-only node registries")
        (@arg registry_auto_refresh_interval: --("registry-auto-refresh") +takes_value
            "How often remote node registries should attempt to fetch upstream changes in the \
             background (in seconds); default is 600 (10 minutes), 0 means off")
        (@arg registry_forced_refresh_interval: --("registry-forced-refresh") +takes_value
            "How long before remote node registries should fetch upstream changes when read \
             (in seconds); default is 10, 0 means off")
        (@arg admin_service_coordinator_timeout: --("admin-timeout") +takes_value
            "The coordinator timeout for admin service proposals (in seconds); default is \
             30 seconds")
        (@arg verbose: -v --verbose +multiple
          "Increase output verbosity"));

    let app = app
        .arg(
            Arg::with_name("heartbeat_interval")
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

    let transport = build_transport(&config)?;

    let state_dir = Path::new(config.state_dir());

    let storage_location = match &config.storage() as &str {
        "yaml" => state_dir
            .join("circuits.yaml")
            .to_str()
            .ok_or_else(|| {
                UserError::InvalidArgument("'state_dir' is not a valid UTF-8 string".into())
            })?
            .to_string(),
        "memory" => "memory".to_string(),
        _ => {
            return Err(UserError::InvalidArgument(format!(
                "storage type is not supported: {}",
                config.storage()
            )))
        }
    };

    let key_registry_location = match &config.storage() as &str {
        "yaml" => state_dir
            .join("keys.yaml")
            .to_str()
            .ok_or_else(|| {
                UserError::InvalidArgument("'state_dir' is not a valid UTF-8 string".into())
            })?
            .to_string(),
        "memory" => "memory".to_string(),
        _ => {
            return Err(UserError::InvalidArgument(format!(
                "storage type is not supported: {}",
                config.storage()
            )))
        }
    };

    let node_registry_directory = state_dir
        .to_str()
        .ok_or_else(|| {
            UserError::InvalidArgument("'state_dir' is not a valid UTF-8 string".into())
        })?
        .to_string();

    let rest_api_endpoint = config.bind();

    #[cfg(feature = "database")]
    let db_url = config.database();

    let admin_service_coordinator_timeout = config.admin_service_coordinator_timeout();

    config.log_as_debug();

    let mut daemon_builder = SplinterDaemonBuilder::new();

    daemon_builder = daemon_builder
        .with_storage_location(storage_location)
        .with_key_registry_location(key_registry_location)
        .with_node_registry_directory(node_registry_directory)
        .with_network_endpoints(config.network_endpoints().to_vec())
        .with_advertised_endpoints(config.advertised_endpoints().to_vec())
        .with_service_endpoint(String::from(config.service_endpoint()))
        .with_initial_peers(config.peers().to_vec())
        .with_node_id(String::from(config.node_id()))
        .with_display_name(String::from(config.display_name()))
        .with_rest_api_endpoint(String::from(rest_api_endpoint))
        .with_storage_type(String::from(config.storage()))
        .with_registries(config.registries().to_vec())
        .with_registry_auto_refresh_interval(config.registry_auto_refresh_interval())
        .with_registry_forced_refresh_interval(config.registry_forced_refresh_interval())
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
