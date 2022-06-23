// Copyright 2018-2022 Cargill Incorporated
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

//! `ConfigBuilder` implementation to construct a finalized `Config` object.
//!
//! Takes various `PartialConfig` objects and finalizes the config values sourced from the
//! `PartialConfigs` to construct a `Config` object to be used to start up the Splinter daemon.

use std::collections::HashMap;
use std::convert::TryInto;
use std::path::Path;

use crate::config::error::ConfigError;
use crate::config::{Config, ConfigSource, PartialConfig};

use super::{AppenderConfig, LoggerConfig};

pub trait PartialConfigBuilder {
    /// Takes all values set in a config object to create a `PartialConfig` object.
    ///
    fn build(self) -> Result<PartialConfig, ConfigError>;
}

// Constructs the tls config file paths by checking whether the file is an absolute or relative
// path, otherwise if only a file name is provided, the `cert_dir` option will be appended to the
// file name.
fn get_tls_file_path(cert_dir: &str, file: &str) -> String {
    let file_path = Path::new(file);
    if file_path.is_absolute() || file_path.starts_with("../") || file_path.starts_with("./") {
        file_path
            .to_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| String::from(file))
    } else {
        Path::new(cert_dir)
            .join(file_path)
            .to_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| String::from(file))
    }
}

fn get_database_path(state_dir: &str, database_file: &str) -> String {
    if database_file.starts_with("postgres://") {
        database_file.to_string()
    } else {
        match database_file {
            "memory" | ":memory:" => database_file.to_string(),
            _ => {
                // if the database_file is a file path, return path
                let file_path = Path::new(database_file);
                if file_path.is_absolute()
                    || file_path.starts_with("../")
                    || file_path.starts_with("./")
                {
                    file_path
                        .to_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| String::from(database_file))
                } else {
                    // return state_dir/database_file
                    Path::new(state_dir)
                        .join(database_file)
                        .to_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| String::from(database_file))
                }
            }
        }
    }
}

/// ConfigBuilder collects `PartialConfig` objects from various sources to be used to generate a
/// `Config` object.
pub struct ConfigBuilder {
    partial_configs: Vec<PartialConfig>,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        ConfigBuilder {
            partial_configs: Vec::new(),
        }
    }

    /// Adds a `PartialConfig` to the `ConfigBuilder` object.
    ///
    /// # Arguments
    ///
    /// * `partial` - A `PartialConfig` object generated from any of the config modules.
    ///
    pub fn with_partial_config(mut self, partial: PartialConfig) -> Self {
        self.partial_configs.push(partial);
        self
    }

    /// Builds a `Config` object by incorporating the values from each `PartialConfig` object.
    ///
    pub fn build(self) -> Result<Config, ConfigError> {
        let config_dir = self
            .partial_configs
            .iter()
            .find_map(|p| p.config_dir().map(|v| (v, p.source())))
            .ok_or_else(|| ConfigError::MissingValue("config directory".to_string()))?;
        let tls_cert_dir = self
            .partial_configs
            .iter()
            .find_map(|p| p.tls_cert_dir().map(|v| (v, p.source())))
            .ok_or_else(|| ConfigError::MissingValue("certificate directory".to_string()))?;
        let tls_ca_file = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_ca_file() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("ca file".to_string()))?;
        let tls_client_cert = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_client_cert() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("client certificate".to_string()))?;
        let tls_client_key = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_client_key() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("client key".to_string()))?;
        let tls_server_cert = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_server_cert() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("server certificate".to_string()))?;
        let tls_server_key = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_server_key() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("server key".to_string()))?;
        #[cfg(feature = "https-bind")]
        let tls_rest_api_cert = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_rest_api_cert() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("rest_api certificate".to_string()))?;
        #[cfg(feature = "https-bind")]
        let tls_rest_api_key = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_rest_api_key() {
                Some(v) => Some((get_tls_file_path(&tls_cert_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("rest_api key".to_string()))?;
        let network_endpoints = self
            .partial_configs
            .iter()
            .find_map(|p| p.network_endpoints().map(|v| (v, p.source())))
            .ok_or_else(|| ConfigError::MissingValue("network endpoints".to_string()))?;

        let state_dir = self
            .partial_configs
            .iter()
            .find_map(|p| p.state_dir().map(|v| (v, p.source())))
            .ok_or_else(|| ConfigError::MissingValue("state directory".to_string()))?;

        let database = self
            .partial_configs
            .iter()
            .find_map(|p| match p.database() {
                Some(v) => Some((get_database_path(&state_dir.0, &v), p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("database".to_string()))?;

        // Iterates over the list of `PartialConfig` objects to find the first config with a value
        // for the specific field. If no value is found, an error is returned.
        Ok(Config {
            config_dir,
            tls_cert_dir,
            tls_ca_file,
            tls_client_cert,
            tls_client_key,
            tls_server_cert,
            tls_server_key,
            #[cfg(feature = "https-bind")]
            tls_rest_api_cert,
            #[cfg(feature = "https-bind")]
            tls_rest_api_key,
            #[cfg(feature = "service-endpoint")]
            service_endpoint: self
                .partial_configs
                .iter()
                .find_map(|p| p.service_endpoint().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("service endpoint".to_string()))?,
            advertised_endpoints: self
                .partial_configs
                .iter()
                .find_map(|p| p.advertised_endpoints().map(|v| (v, p.source())))
                // Default to whatever `network_endpoints` is set to
                .unwrap_or((network_endpoints.0.clone(), ConfigSource::Default)),
            network_endpoints,
            peers: self
                .partial_configs
                .iter()
                .find_map(|p| p.peers().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("peers".to_string()))?,
            display_name: self
                .partial_configs
                .iter()
                .find_map(|p| p.display_name().map(|v| (v, p.source()))),
            node_id: self
                .partial_configs
                .iter()
                .find_map(|p| p.node_id().map(|v| (v, p.source()))),
            rest_api_endpoint: self
                .partial_configs
                .iter()
                .find_map(|p| p.rest_api_endpoint().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("rest api endpoint".to_string()))?,
            database,
            registries: self
                .partial_configs
                .iter()
                .find_map(|p| p.registries().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("registries".to_string()))?,
            registry_auto_refresh: self
                .partial_configs
                .iter()
                .find_map(|p| p.registry_auto_refresh().map(|v| (v, p.source())))
                .ok_or_else(|| {
                    ConfigError::MissingValue("registry auto refresh interval".to_string())
                })?,
            registry_forced_refresh: self
                .partial_configs
                .iter()
                .find_map(|p| p.registry_forced_refresh().map(|v| (v, p.source())))
                .ok_or_else(|| {
                    ConfigError::MissingValue("registry forced refresh interval".to_string())
                })?,
            heartbeat: self
                .partial_configs
                .iter()
                .find_map(|p| p.heartbeat().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("heartbeat interval".to_string()))?,
            admin_timeout: self
                .partial_configs
                .iter()
                .find_map(|p| p.admin_timeout().map(|v| (v, p.source())))
                .ok_or_else(|| {
                    ConfigError::MissingValue("admin service coordinator timeout".to_string())
                })?,
            state_dir,
            tls_insecure: self
                .partial_configs
                .iter()
                .find_map(|p| p.tls_insecure().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("insecure".to_string()))?,
            no_tls: self
                .partial_configs
                .iter()
                .find_map(|p| p.no_tls().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("no tls".to_string()))?,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self
                .partial_configs
                .iter()
                .find_map(|p| p.whitelist().map(|v| (v, p.source()))),
            #[cfg(feature = "biome-credentials")]
            enable_biome_credentials: self
                .partial_configs
                .iter()
                .find_map(|p| p.enable_biome_credentials().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("enable_biome_credentials".to_string()))?,
            #[cfg(feature = "oauth")]
            oauth_provider: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_provider().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_client_id: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_client_id().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_client_secret: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_client_secret().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_redirect_url: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_redirect_url().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_openid_url: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_openid_url().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_openid_auth_params: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_openid_auth_params().map(|v| (v, p.source()))),
            #[cfg(feature = "oauth")]
            oauth_openid_scopes: self
                .partial_configs
                .iter()
                .find_map(|p| p.oauth_openid_scopes().map(|v| (v, p.source()))),
            strict_ref_counts: self
                .partial_configs
                .iter()
                .find_map(|p| p.strict_ref_counts().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("strict_ref_counts".to_string()))?,
            #[cfg(feature = "tap")]
            influx_db: self
                .partial_configs
                .iter()
                .find_map(|p| p.influx_db().map(|v| (v, p.source()))),
            #[cfg(feature = "tap")]
            influx_url: self
                .partial_configs
                .iter()
                .find_map(|p| p.influx_url().map(|v| (v, p.source()))),
            #[cfg(feature = "tap")]
            influx_username: self
                .partial_configs
                .iter()
                .find_map(|p| p.influx_username().map(|v| (v, p.source()))),
            #[cfg(feature = "tap")]
            influx_password: self
                .partial_configs
                .iter()
                .find_map(|p| p.influx_password().map(|v| (v, p.source()))),
            peering_key: self
                .partial_configs
                .iter()
                .find_map(|p| p.peering_key().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("peering_key".to_string()))?,
            appenders: Some({
                let appenders = self
                    .partial_configs
                    .iter()
                    .filter_map(|partial| {
                        partial.appenders().map(|vector| {
                            vector
                                .iter()
                                .map(|item| {
                                    let result = (item.0.to_owned(), item.1.to_owned()).try_into();
                                    match result {
                                        Ok(inner) => Ok((inner, partial.source())),
                                        Err(e) => Err(e),
                                    }
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .flatten()
                    .collect::<Result<Vec<(AppenderConfig, ConfigSource)>, ConfigError>>()?;
                let mut map: HashMap<String, &(AppenderConfig, ConfigSource)> = HashMap::new();
                for appender in appenders.iter().rev() {
                    map.insert(appender.0.name.to_owned(), appender);
                }
                map.values()
                    .map(|item| (item.0.to_owned(), item.1.to_owned()))
                    .collect()
            }),
            loggers: Some({
                let loggers = self
                    .partial_configs
                    .iter()
                    .filter_map(|partial| {
                        partial.loggers().map(|vector| {
                            vector
                                .iter()
                                .map(|item| {
                                    (
                                        (item.0.to_owned(), item.1.to_owned()).into(),
                                        partial.source(),
                                    )
                                })
                                .collect::<Vec<(LoggerConfig, ConfigSource)>>()
                        })
                    })
                    .flatten()
                    .collect::<Vec<(LoggerConfig, ConfigSource)>>();
                let mut map: HashMap<String, &(LoggerConfig, ConfigSource)> = HashMap::new();
                for logger in loggers.iter().rev() {
                    map.insert(logger.0.name.to_owned(), logger);
                }
                map.values()
                    .map(|item| (item.0.to_owned(), item.1.to_owned()))
                    .collect()
            }),
            root_logger: self
                .partial_configs
                .iter()
                .find_map(|p| p.root_logger().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("root_logger".to_string()))?,
            verbosity: self
                .partial_configs
                .iter()
                .find_map(|p| p.verbosity().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("verbosity".to_string()))?,
            scabbard_state: self
                .partial_configs
                .iter()
                .find_map(|p| p.scabbard_state().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("scabbard_state".to_string()))?,
            scabbard_autocleanup: self
                .partial_configs
                .iter()
                .find_map(|p| p.scabbard_autocleanup().map(|v| (v, p.source())))
                .ok_or_else(|| ConfigError::MissingValue("scabbard_autocleanup".to_string()))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example configuration values.
    static EXAMPLE_CA_CERTS: &str = "/etc/splinter/certs/ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "/etc/splinter/certs/client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "/etc/splinter/certs/client.key";
    static EXAMPLE_SERVER_CERT: &str = "/etc/splinter/certs/server.crt";
    static EXAMPLE_SERVER_KEY: &str = "/etc/splinter/certs/server.key";
    #[cfg(feature = "https-bind")]
    static EXAMPLE_REST_API_CERT: &str = "/etc/splinter/certs/rest_api.crt";
    #[cfg(feature = "https-bind")]
    static EXAMPLE_REST_API_KEY: &str = "/etc/splinter/certs/rest_api.key";
    #[cfg(feature = "service-endpoint")]
    static EXAMPLE_SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static EXAMPLE_NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
    static EXAMPLE_ADVERTISED_ENDPOINT: &str = "localhost:8044";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";

    /// Asserts the example configuration values.
    fn assert_config_values(config: PartialConfig) {
        assert_eq!(config.tls_cert_dir(), None);
        assert_eq!(config.tls_ca_file(), Some(EXAMPLE_CA_CERTS.to_string()));
        assert_eq!(
            config.tls_client_cert(),
            Some(EXAMPLE_CLIENT_CERT.to_string())
        );
        assert_eq!(
            config.tls_client_key(),
            Some(EXAMPLE_CLIENT_KEY.to_string())
        );
        assert_eq!(
            config.tls_server_cert(),
            Some(EXAMPLE_SERVER_CERT.to_string())
        );
        assert_eq!(
            config.tls_server_key(),
            Some(EXAMPLE_SERVER_KEY.to_string())
        );
        #[cfg(feature = "https-bind")]
        {
            assert_eq!(
                config.tls_rest_api_cert(),
                Some(EXAMPLE_REST_API_CERT.to_string())
            );
            assert_eq!(
                config.tls_rest_api_key(),
                Some(EXAMPLE_REST_API_KEY.to_string())
            );
        }
        #[cfg(feature = "service-endpoint")]
        assert_eq!(
            config.service_endpoint(),
            Some(EXAMPLE_SERVICE_ENDPOINT.to_string())
        );
        assert_eq!(
            config.network_endpoints(),
            Some(vec![EXAMPLE_NETWORK_ENDPOINT.to_string()])
        );
        assert_eq!(
            config.advertised_endpoints(),
            Some(vec![EXAMPLE_ADVERTISED_ENDPOINT.to_string()])
        );
        assert_eq!(config.peers(), Some(vec![]));
        assert_eq!(config.node_id(), Some(EXAMPLE_NODE_ID.to_string()));
        assert_eq!(
            config.display_name(),
            Some(EXAMPLE_DISPLAY_NAME.to_string())
        );
        assert_eq!(config.rest_api_endpoint(), None);
        assert_eq!(config.database(), None);
        assert_eq!(config.registries(), Some(vec![]));
        assert_eq!(config.heartbeat(), None);
        assert_eq!(config.admin_timeout(), None);
    }

    #[test]
    /// This test verifies that a `PartialConfig` object is accurately constructed by chaining the
    /// `PartialConfigBuilder` methods. The following steps are performed:
    ///
    /// 1. An empty `PartialConfig` object is constructed.
    /// 2. The fields of the `PartialConfig` object are populated by chaining the builder methods.
    ///
    /// This test then verifies the `PartialConfig` object built from chaining the builder methods
    /// contains the correct values by asserting each expected value.
    fn test_builder_chain() {
        // Create an empty `PartialConfig` object.
        let mut partial_config = PartialConfig::new(ConfigSource::Default);
        // Populate the `PartialConfig` fields by chaining the builder methods.
        partial_config = partial_config
            .with_tls_cert_dir(None)
            .with_tls_ca_file(Some(EXAMPLE_CA_CERTS.to_string()))
            .with_tls_client_cert(Some(EXAMPLE_CLIENT_CERT.to_string()))
            .with_tls_client_key(Some(EXAMPLE_CLIENT_KEY.to_string()))
            .with_tls_server_cert(Some(EXAMPLE_SERVER_CERT.to_string()))
            .with_tls_server_key(Some(EXAMPLE_SERVER_KEY.to_string()))
            .with_network_endpoints(Some(vec![EXAMPLE_NETWORK_ENDPOINT.to_string()]))
            .with_advertised_endpoints(Some(vec![EXAMPLE_ADVERTISED_ENDPOINT.to_string()]))
            .with_peers(Some(vec![]))
            .with_node_id(Some(EXAMPLE_NODE_ID.to_string()))
            .with_display_name(Some(EXAMPLE_DISPLAY_NAME.to_string()))
            .with_rest_api_endpoint(None)
            .with_registries(Some(vec![]))
            .with_heartbeat(None)
            .with_admin_timeout(None);

        #[cfg(feature = "https-bind")]
        {
            partial_config = partial_config
                .with_tls_rest_api_cert(Some(String::from(EXAMPLE_REST_API_CERT)))
                .with_tls_rest_api_key(Some(String::from(EXAMPLE_REST_API_KEY)));
        }

        #[cfg(feature = "service-endpoint")]
        {
            partial_config =
                partial_config.with_service_endpoint(Some(EXAMPLE_SERVICE_ENDPOINT.to_string()))
        }
        // Compare the generated `PartialConfig` object against the expected values.
        assert_config_values(partial_config);
    }

    #[test]
    /// This test verifies that a `PartialConfig` object is accurately constructed by separately
    /// applying the builder methods. The following steps are performed:
    ///
    /// 1. An empty `PartialConfig` object is constructed.
    /// 2. The fields of the `PartialConfig` object are populated by separately applying the builder
    ///    methods.
    ///
    /// This test then verifies the `PartialConfig` object built from separately applying the builder
    /// methods contains the correct values by asserting each expected value.
    fn test_builder_separate() {
        // Create a new `PartialConfig` object.
        let mut partial_config = PartialConfig::new(ConfigSource::Default);
        // Populate the `PartialConfig` fields by separately applying the builder methods.
        partial_config = partial_config.with_tls_ca_file(Some(EXAMPLE_CA_CERTS.to_string()));
        partial_config = partial_config.with_tls_client_cert(Some(EXAMPLE_CLIENT_CERT.to_string()));
        partial_config = partial_config.with_tls_client_key(Some(EXAMPLE_CLIENT_KEY.to_string()));
        partial_config = partial_config.with_tls_server_cert(Some(EXAMPLE_SERVER_CERT.to_string()));
        partial_config = partial_config.with_tls_server_key(Some(EXAMPLE_SERVER_KEY.to_string()));

        #[cfg(feature = "https-bind")]
        {
            partial_config =
                partial_config.with_tls_rest_api_cert(Some(EXAMPLE_REST_API_CERT.to_string()));
            partial_config =
                partial_config.with_tls_rest_api_key(Some(EXAMPLE_REST_API_KEY.to_string()));
        }

        #[cfg(feature = "service-endpoint")]
        {
            partial_config =
                partial_config.with_service_endpoint(Some(EXAMPLE_SERVICE_ENDPOINT.to_string()));
        }
        partial_config =
            partial_config.with_network_endpoints(Some(vec![EXAMPLE_NETWORK_ENDPOINT.to_string()]));
        partial_config = partial_config
            .with_advertised_endpoints(Some(vec![EXAMPLE_ADVERTISED_ENDPOINT.to_string()]));
        partial_config = partial_config.with_peers(Some(vec![]));
        partial_config = partial_config.with_node_id(Some(EXAMPLE_NODE_ID.to_string()));
        partial_config = partial_config.with_display_name(Some(EXAMPLE_DISPLAY_NAME.to_string()));
        partial_config = partial_config.with_admin_timeout(None);
        partial_config = partial_config.with_registries(Some(vec![]));
        // Compare the generated `PartialConfig` object against the expected values.
        assert_config_values(partial_config);
    }
}
