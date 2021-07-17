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
#[macro_use]
extern crate clap;

mod config;
mod daemon;
mod error;
#[cfg(feature = "log-config")]
mod logging;
mod routes;
mod transport;

#[cfg(feature = "challenge-authorization")]
use cylinder::{load_key_from_path, secp256k1::Secp256k1Context, Context, Signer};
use log4rs::config::{Appender, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;

use rand::{thread_rng, Rng};
#[cfg(feature = "metrics")]
use splinter::metrics::influx::InfluxRecorder;

use crate::config::{
    ClapPartialConfigBuilder, Config, ConfigBuilder, ConfigError, DefaultPartialConfigBuilder,
    EnvPartialConfigBuilder, PartialConfigBuilder, TomlPartialConfigBuilder,
};
use crate::daemon::SplinterDaemonBuilder;
use clap::{clap_app, crate_version};
use clap::{Arg, ArgMatches};

use std::env;
#[cfg(feature = "challenge-authorization")]
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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

    builder.build().map_err(UserError::ConfigError)
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

// load all signing keys from the configured splinterd key file
#[cfg(feature = "challenge-authorization")]
fn load_signer_keys(config_dir: &str) -> Result<Vec<Box<dyn Signer>>, UserError> {
    let splinterd_key_path = Path::new(config_dir).join("keys");
    let paths = match fs::read_dir(splinterd_key_path) {
        Ok(paths) => paths,
        Err(err) => {
            warn!(
                "Starting daemon with no signing keys, \
                unable to read splinterd keys directory: {}",
                err
            );
            return Ok(vec![]);
        }
    };

    let mut signing_keys = vec![];
    for path in paths {
        let path = path
            .map_err(|err| {
                UserError::io_err_with_source(
                    &format!("Unable to get keys in path {}/keys", config_dir),
                    Box::new(err),
                )
            })?
            .path();

        if path.extension() == Some(OsStr::new("priv")) {
            let private_key = load_key_from_path(&path).map_err(UserError::KeyLoadError)?;
            signing_keys.push(Secp256k1Context::new().new_signer(private_key));
        }
    }

    if signing_keys.is_empty() {
        warn!("Starting daemon with no signing keys");
    }

    Ok(signing_keys)
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
                .help("Path to the directory containing state files")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("database")
                .long("database")
                .long_help("DB connection URL")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("enable_biome")
                .long("enable-biome")
                .long_help("Enable the biome subsystem")
                .hidden(true),
        );

    #[cfg(feature = "https-bind")]
    let app = app.arg(
        Arg::with_name("tls_rest_api_cert")
            .long("tls-rest-api-cert")
            .help("File path to the certificate for the node's REST API.")
            .takes_value(true)
            .alias("rest-api-cert"),
    );

    #[cfg(feature = "https-bind")]
    let app = app.arg(
        Arg::with_name("tls_rest_api_key")
            .long("tls-rest-api-key")
            .help("File path to the key for the node's REST API.")
            .takes_value(true)
            .alias("rest-api-key"),
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

    #[cfg(feature = "biome-credentials")]
    let app = app.arg(
        Arg::with_name("enable_biome_credentials")
            .long("enable-biome-credentials")
            .long_help("Enable the Biome credentials for REST API authentication"),
    );

    #[cfg(feature = "oauth")]
    let app = app
        .arg(
            Arg::with_name("oauth_provider")
                .long("oauth-provider")
                .long_help("The OAuth provider used by the REST API")
                .takes_value(true)
                .possible_values(&["azure", "github", "google", "openid"]),
        )
        .arg(
            Arg::with_name("oauth_client_id")
                .long("oauth-client-id")
                .long_help("Client ID for the OAuth provider used by the REST API")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("oauth_client_secret")
                .long("oauth-client-secret")
                .long_help("Client secret for the OAuth provider used by the REST API")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("oauth_redirect_url")
                .long("oauth-redirect-url")
                .long_help("Redirect URL for the OAuth provider used by the REST API")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("oauth_openid_url")
                .long("oauth-openid-url")
                .long_help("URL for an OpenID discovery document used by the REST API")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("oauth_openid_auth_params")
                .long("oauth-openid-auth-params")
                .alias("oauth-openid-auth-param")
                .long_help(
                    "Addtional parameters to add to OAuth OpenID auth requests, formatted as \
                     `key=value` pairs (requires `--oauth-provider openid`)",
                )
                .takes_value(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("oauth_openid_scopes")
                .long("oauth-openid-scopes")
                .alias("oauth-openid-scope")
                .long_help(
                    "Addtional scopes to request from the OAuth OpenID provider (requires \
                     `--oauth-provider openid`)",
                )
                .takes_value(true)
                .multiple(true),
        );

    #[cfg(feature = "metrics")]
    let app = app
        .arg(
            Arg::with_name("metrics_db")
                .long("metrics-db")
                .long_help("The name of the InfluxDB database for metrics collection")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("metrics_url")
                .long("metrics-url")
                .long_help("The URL to connect the InfluxDB database for metrics collection")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("metrics_username")
                .long("metrics-username")
                .long_help("The username used for authorization with the InfluxDB")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("metrics_password")
                .long("metrics-password")
                .long_help("The password used for authorization with the InfluxDB")
                .takes_value(true),
        );

    let matches = app.get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let encoder = PatternEncoder::new("[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n");
    let stdout = log4rs::append::console::ConsoleAppender::builder()
        .encoder(Box::new(encoder))
        .build();
    let config = log4rs::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .logger(Logger::builder().build("hyper", log::LevelFilter::Warn))
        .logger(Logger::builder().build("tokio", log::LevelFilter::Warn));
    #[cfg(feature = "https-bind")]
    let config = config.logger(Logger::builder().build("h2", log::LevelFilter::Warn));
    let log_config = config.build(Root::builder().appender("stdout").build(log_level));
    if let Ok(lc) = log_config {
        let _handle = log4rs::init_config(lc);
    } else {
        unreachable!();
        // log_config  should always return an Ok variant.
        // Err is returned if multiple logger facades are registered which shouldn't ever happen
    }

    if let Err(err) = start_daemon(matches) {
        error!("Failed to start daemon, {}", err);
        std::process::exit(1);
    }
}

#[cfg(feature = "metrics")]
fn setup_metrics_recorder(config: &Config) -> Result<(), UserError> {
    let metrics_configured = config.metrics_db().is_some()
        || config.metrics_url().is_some()
        || config.metrics_username().is_some()
        || config.metrics_password().is_some();

    if metrics_configured {
        let metrics_db = config.metrics_db().ok_or_else(|| {
            UserError::MissingArgument("missing metrics db provider configuration".into())
        })?;

        let metrics_url = config.metrics_url().ok_or_else(|| {
            UserError::MissingArgument("missing metrics url provider configuration".into())
        })?;

        let metrics_username = config.metrics_username().ok_or_else(|| {
            UserError::MissingArgument("missing metrics username provider configuration".into())
        })?;

        let metrics_password = config.metrics_password().ok_or_else(|| {
            UserError::MissingArgument("missing metrics password provider configuration".into())
        })?;

        InfluxRecorder::init(metrics_url, metrics_db, metrics_username, metrics_password)
            .map_err(UserError::MetricsError)?
    }

    Ok(())
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

    let state_dir = config.state_dir();
    if !Path::new(&state_dir).is_dir() {
        return Err(UserError::DaemonError {
            context: format!("state directory {} does not exist", state_dir),
            source: None,
        });
    }

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

    // set up metric recorder as soon as possilbe
    #[cfg(feature = "metrics")]
    setup_metrics_recorder(&config)?;

    let transport = build_transport(&config)?;

    let rest_api_endpoint = config.rest_api_endpoint();

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
        .with_db_url(config.database().to_string())
        .with_registries(config.registries().to_vec())
        .with_registry_auto_refresh(config.registry_auto_refresh())
        .with_registry_forced_refresh(config.registry_forced_refresh())
        .with_heartbeat(config.heartbeat())
        .with_admin_timeout(admin_timeout)
        .with_strict_ref_counts(config.strict_ref_counts());

    #[cfg(feature = "authorization-handler-allow-keys")]
    {
        daemon_builder = daemon_builder.with_config_dir(config.config_dir().to_string());
    }

    #[cfg(feature = "https-bind")]
    {
        daemon_builder = daemon_builder
            .with_rest_api_server_cert(config.tls_rest_api_cert().to_string())
            .with_rest_api_server_key(config.tls_rest_api_key().to_string());
    }

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

    #[cfg(feature = "rest-api-cors")]
    {
        daemon_builder = daemon_builder.with_whitelist(config.whitelist().map(ToOwned::to_owned));
    }

    #[cfg(feature = "biome-credentials")]
    {
        daemon_builder =
            daemon_builder.with_enable_biome_credentials(config.enable_biome_credentials());
    }

    #[cfg(feature = "oauth")]
    {
        daemon_builder = daemon_builder
            .with_oauth_provider(config.oauth_provider().map(ToOwned::to_owned))
            .with_oauth_client_id(config.oauth_client_id().map(ToOwned::to_owned))
            .with_oauth_client_secret(config.oauth_client_secret().map(ToOwned::to_owned))
            .with_oauth_redirect_url(config.oauth_redirect_url().map(ToOwned::to_owned))
            .with_oauth_openid_url(config.oauth_openid_url().map(ToOwned::to_owned))
            .with_oauth_openid_auth_params(config.oauth_openid_auth_params().map(ToOwned::to_owned))
            .with_oauth_openid_scopes(config.oauth_openid_scopes().map(ToOwned::to_owned));
    }

    #[cfg(feature = "challenge-authorization")]
    {
        let signers = load_signer_keys(config.config_dir())?;
        daemon_builder = daemon_builder.with_signers(signers);
    }

    let mut node = daemon_builder.build().map_err(|err| {
        UserError::daemon_err_with_source("unable to build the Splinter daemon", Box::new(err))
    })?;
    node.start(transport)?;
    Ok(())
}
