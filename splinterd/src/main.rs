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
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate clap;

mod certs;
mod config;
mod daemon;
mod node_registry;
mod registry_config;
mod routes;

use flexi_logger::{style, DeferredNow, LogSpecBuilder, Logger};
use log::Record;

use crate::certs::{make_ca_cert, make_ca_signed_cert, write_file, CertError};
use crate::config::{Config, ConfigError};
#[cfg(feature = "config-toml")]
use crate::config::{ConfigBuilder, TomlConfig};
use crate::daemon::SplinterDaemonBuilder;
use clap::{clap_app, crate_version};
use openssl::error::ErrorStack;
use splinter::transport::raw::RawTransport;
use splinter::transport::tls::{TlsInitError, TlsTransport};
use splinter::transport::Transport;
use tempdir::TempDir;

use std::env;
#[cfg(feature = "config-toml")]
use std::fs;
#[cfg(not(feature = "config-toml"))]
use std::fs::File;
use std::io;
use std::path::Path;
use std::thread;

const DEFAULT_STATE_DIR: &str = "/var/lib/splinter/";
const STATE_DIR_ENV: &str = "SPLINTER_STATE_DIR";
const HEARTBEAT_DEFAULT: u64 = 30;

#[cfg(not(feature = "config-toml"))]
fn load_toml_config(config_file_path: &str) -> Config {
    File::open(config_file_path)
        .map_err(ConfigError::from)
        .and_then(Config::from_file)
        .unwrap_or_else(|err| {
            warn!("Unable to load {}: {}", config_file_path, err);
            Config::default()
        })
}

#[cfg(feature = "config-toml")]
fn load_toml_config(config_file_path: &str) -> Config {
    let mut config_builder = ConfigBuilder::new();

    match fs::read_to_string(config_file_path)
        .map_err(ConfigError::from)
        .and_then(TomlConfig::new)
    {
        Ok(toml_config) => {
            config_builder = toml_config.apply_to_builder(config_builder);
        }
        Err(err) => {
            warn!("Unable to load {}: {}", config_file_path, err);
        }
    };

    config_builder.build()
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
    let matches = clap_app!(splinterd =>
        (version: crate_version!())
        (about: "Splinter Daemon")
        (@arg config: -c --config +takes_value)
        (@arg node_id: --("node-id") +takes_value
          "unique id for the node ")
        (@arg storage: --("storage") +takes_value
          "storage type used for node, default yaml")
        (@arg transport: --("transport") +takes_value
          "transport type for sockets, either raw or tls")
        (@arg network_endpoint: -n --("network-endpoint") +takes_value
          "endpoint to connect to the network, tcp://ip:port")
        (@arg service_endpoint: --("service-endpoint") +takes_value
          "endpoint that service will connect to, tcp://ip:port")
        (@arg peers: --peer +takes_value +multiple
          "endpoint that service will connect to, ip:port")
        (@arg ca_file: --("ca-file") +takes_value
          "file path to the trusted ca cert")
        (@arg client_cert: --("client-cert") +takes_value
          "file path the cert for the node when connecting to a node")
        (@arg server_cert: --("server-cert") +takes_value
          "file path the cert for the node when connecting to a node")
        (@arg server_key:  --("server-key") +takes_value
          "file path key for the node when connecting to a node as sever")
        (@arg client_key:  --("client-key") +takes_value
          "file path key for the node when connecting to a node as client")
        (@arg insecure:  --("insecure")
          "if set tls should accept all peer certificates")
        (@arg generate_certs:  --("generate-certs")
          "if set, the certs will be generated and insecure will be false, only use for development")
        (@arg cert_location:  --("cert-location") +takes_value
          "if generating certs, save the generated certs to this location.")
        (@arg common_name: --("common-name") +takes_value
          "the common name that should be used in the generated cert, defaults to localhost")
        (@arg bind: --("bind") +takes_value
            "connection endpoint for REST API")
        (@arg registry_backend: --("registry-backend") +takes_value
            "backend type for the node registry. Possible values: FILE.")
        (@arg registry_file: --("registry-file") +takes_value
            "file path to the node registry file if registry-backend is FILE.")
        (@arg heartbeat_interval: --("heartbeat") +takes_value
            "how often heartbeat should be sent in seconds, defaults to 30 seconds, 0 means off")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity"))
    .get_matches();

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

    debug!("Loading configuration file");

    // get provided config file or search default location
    let config_file_path = matches
        .value_of("config")
        .unwrap_or("/etc/splinter/splinterd.toml");

    let config = load_toml_config(config_file_path);

    // Currently only YamlStorage is supported

    let node_id = matches
        .value_of("node_id")
        .map(String::from)
        .or_else(|| config.node_id())
        .expect("Must provide a unique node id");

    let storage_type = matches
        .value_of("storage")
        .map(String::from)
        .or_else(|| config.storage())
        .or_else(|| Some(String::from("yaml")))
        .expect("No Storage Provided");

    let transport_type = matches
        .value_of("transport")
        .map(String::from)
        .or_else(|| config.transport())
        .or_else(|| Some(String::from("raw")))
        .expect("No Transport Provided");

    let service_endpoint = matches
        .value_of("service_endpoint")
        .map(String::from)
        .or_else(|| config.service_endpoint())
        .or_else(|| Some("127.0.0.1:8043".to_string()))
        .expect("Must provide a valid service endpoint");

    let network_endpoint = matches
        .value_of("network_endpoint")
        .map(String::from)
        .or_else(|| config.network_endpoint())
        .or_else(|| Some("127.0.0.1:8044".to_string()))
        .expect("Must provide a valid network endpoint");

    let initial_peers = matches
        .values_of("peers")
        .map(|values| values.map(String::from).collect::<Vec<String>>())
        .or_else(|| config.peers())
        .unwrap_or_default();

    let heartbeat_interval = value_t!(matches.value_of("heartbeat_interval"), u64)
        .unwrap_or_else(|_| config.heartbeat_interval().unwrap_or(HEARTBEAT_DEFAULT));

    let (transport, transport_log) = match get_transport(&transport_type, &matches, &config) {
        Ok(transport) => transport,
        Err(err) => {
            error!("An error occurred while getting transport {:?}", err);
            std::process::exit(1);
        }
    };

    let location = {
        if let Ok(s) = env::var(STATE_DIR_ENV) {
            s.to_string()
        } else {
            DEFAULT_STATE_DIR.to_string()
        }
    };

    let storage_location = match &storage_type as &str {
        "yaml" => format!("{}{}", location, "circuits.yaml"),
        "memory" => "memory".to_string(),
        _ => panic!("Storage type is not supported: {}", storage_type),
    };

    let key_registry_location = match &storage_type as &str {
        "yaml" => format!("{}{}", location, "keys.yaml"),
        "memory" => "memory".to_string(),
        _ => panic!("Storage type is not supported: {}", storage_type),
    };

    let rest_api_endpoint = matches
        .value_of("bind")
        .map(String::from)
        .or_else(|| config.bind())
        .or_else(|| Some("127.0.0.1:8080".to_string()))
        .expect("Must provide a url for REST API endpoint");

    let registry_backend = matches
        .value_of("registry_backend")
        .map(String::from)
        .or_else(|| config.registry_backend());

    let registry_file = matches
        .value_of("registry_file")
        .map(String::from)
        .or_else(|| config.registry_file());

    debug!(
        "Configuration: {{ storage_type: {}, storage_location: {}, key_registry_location: {}, {}, \
         service_endpoint: {}, network_endpoint: {}, initial_peers: {:?}, node_id: {}, \
         rest_api_endpoint: {}, registry_backend: {:?}, registry_file: {:?}, \
         heartbeat_interval: {} }}",
        storage_type,
        storage_location,
        key_registry_location,
        transport_log,
        service_endpoint,
        network_endpoint,
        initial_peers,
        node_id,
        rest_api_endpoint,
        registry_backend,
        registry_file,
        heartbeat_interval,
    );

    let mut daemon_builder = SplinterDaemonBuilder::new()
        .with_storage_location(storage_location)
        .with_key_registry_location(key_registry_location)
        .with_network_endpoint(network_endpoint)
        .with_service_endpoint(service_endpoint)
        .with_initial_peers(initial_peers)
        .with_node_id(node_id)
        .with_rest_api_endpoint(rest_api_endpoint)
        .with_registry_backend(registry_backend.clone())
        .with_storage_type(storage_type)
        .with_heartbeat_interval(heartbeat_interval);

    if let Some(registry_file) = registry_file {
        daemon_builder = daemon_builder.with_registry_file(registry_file);
    }

    let mut node = match daemon_builder.build() {
        Ok(node) => node,
        Err(err) => {
            error!("An error occurred while creating daemon {:?}", err);
            std::process::exit(1);
        }
    };

    if let Err(err) = node.start(transport) {
        error!("Failed to start daemon {:?}", err);
        std::process::exit(1);
    }
}

fn get_transport(
    transport_type: &str,
    matches: &clap::ArgMatches,
    config: &Config,
) -> Result<(Box<dyn Transport + Send>, String), GetTransportError> {
    match transport_type {
        "tls" => {
            if matches.is_present("generate_certs") {
                info!("Generating Certs for TLS Transport");

                let common_name = matches
                    .value_of("common_name")
                    .map(String::from)
                    .unwrap_or_else(|| String::from("localhost"));

                // Generate Certificate Authority keys and certificate
                let (ca_key, ca_cert) = make_ca_cert()?;

                let temp_dir = TempDir::new("tls-transport")?;
                let dir_path = {
                    match matches.value_of("cert_location") {
                        Some(cert_location) => Path::new(cert_location),
                        None => {
                            // Create temp directory to store generated_certs
                            temp_dir.path()
                        }
                    }
                };

                // Generate client and server keys and certificates
                let (client_key, client_cert) =
                    make_ca_signed_cert(&ca_cert, &ca_key, &common_name)?;
                let (server_key, server_cert) =
                    make_ca_signed_cert(&ca_cert, &ca_key, &common_name)?;

                let client_cert = write_file(
                    dir_path.to_path_buf(),
                    "client.cert",
                    &client_cert.to_pem()?,
                )?;

                let client_key_file = write_file(
                    dir_path.to_path_buf(),
                    "client.key",
                    &client_key.private_key_to_pem_pkcs8()?,
                )?;

                let server_cert = write_file(
                    dir_path.to_path_buf(),
                    "server.cert",
                    &server_cert.to_pem()?,
                )?;

                let server_key_file = write_file(
                    dir_path.to_path_buf(),
                    "server.key",
                    &server_key.private_key_to_pem_pkcs8()?,
                )?;

                warn!("Starting TlsTransport in insecure mode");

                let log_value = "ca_certs: generated, client_cert: generated, client_key: \
                                 generated, server_cert: generated, server_key: generated"
                    .to_string();

                // Start transport in insecure mode, do not verify the certs if auto generated,
                // as the ca will not match
                let transport = TlsTransport::new(
                    None,
                    client_key_file,
                    client_cert,
                    server_key_file,
                    server_cert,
                )?;

                Ok((Box::new(transport), log_value))
            } else {
                let client_cert = matches
                    .value_of("client_cert")
                    .map(String::from)
                    .or_else(|| config.client_cert())
                    .expect("Must provide a valid client certificate");

                let server_cert = matches
                    .value_of("server_cert")
                    .map(String::from)
                    .or_else(|| config.server_cert())
                    .expect("Must provide a valid server certificate");

                let server_key_file = matches
                    .value_of("server_key")
                    .map(String::from)
                    .or_else(|| config.server_key())
                    .expect("Must provide a valid key path");

                let client_key_file = matches
                    .value_of("client_key")
                    .map(String::from)
                    .or_else(|| config.client_key())
                    .expect("Must provide a valid key path");

                let ca_file = {
                    if matches.is_present("insecure") {
                        warn!("Starting TlsTransport in insecure mode");
                        None
                    } else {
                        let ca_file = matches
                            .value_of("ca_file")
                            .map(String::from)
                            .or_else(|| config.ca_certs())
                            .expect("Must provide a valid file containing ca certs");
                        Some(ca_file)
                    }
                };

                let current_path = env::current_dir()?
                    .to_str()
                    .expect("Unable to get current path")
                    .to_string();

                let ca_file_log = {
                    if let Some(ca_file) = &ca_file {
                        format!("{}/{}", current_path, &ca_file)
                    } else {
                        "insecure".to_string()
                    }
                };

                let log_value = format!(
                    "transport_type: tls, ca_certs: {}, client_cert: {}/{}, \
                     client_key: {}/{}, server_cert: {}/{}, server_key: {}/{}",
                    ca_file_log,
                    current_path,
                    client_cert,
                    current_path,
                    client_key_file,
                    current_path,
                    server_cert,
                    current_path,
                    server_key_file,
                );

                let transport = TlsTransport::new(
                    ca_file,
                    client_key_file,
                    client_cert,
                    server_key_file,
                    server_cert,
                )?;

                Ok((Box::new(transport), log_value))
            }
        }
        "raw" => Ok((
            Box::new(RawTransport::default()),
            "transport_type: raw".to_string(),
        )),
        _ => Err(GetTransportError::NotSupportedError(format!(
            "Transport type {} is not supported",
            transport_type
        ))),
    }
}

#[derive(Debug)]
pub enum GetTransportError {
    CertError(CertError),
    NotSupportedError(String),
    TlsTransportError(TlsInitError),
    OpensslError(ErrorStack),
    IoError(io::Error),
}

impl From<CertError> for GetTransportError {
    fn from(cert_error: CertError) -> Self {
        GetTransportError::CertError(cert_error)
    }
}

impl From<TlsInitError> for GetTransportError {
    fn from(tls_error: TlsInitError) -> Self {
        GetTransportError::TlsTransportError(tls_error)
    }
}

impl From<ErrorStack> for GetTransportError {
    fn from(error_stack: ErrorStack) -> Self {
        GetTransportError::OpensslError(error_stack)
    }
}

impl From<io::Error> for GetTransportError {
    fn from(io_error: io::Error) -> Self {
        GetTransportError::IoError(io_error)
    }
}
