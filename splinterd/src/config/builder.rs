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

//! `ConfigBuilder` implementation to construct a finalized `Config` object.
//!
//! Takes various `PartialConfig` objects and finalizes the config values sourced from the
//! `PartialConfigs` to construct a `Config` object to be used to start up the Splinter daemon.

use std::path::Path;

use crate::config::error::ConfigError;
use crate::config::{Config, ConfigSource, PartialConfig};

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

    #[cfg(feature = "default")]
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
            .find_map(|p| match p.config_dir() {
                Some(v) => Some((v, p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("config directory".to_string()))?;
        let tls_cert_dir = self
            .partial_configs
            .iter()
            .find_map(|p| match p.tls_cert_dir() {
                Some(v) => Some((v, p.source())),
                None => None,
            })
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
        let network_endpoints = self
            .partial_configs
            .iter()
            .find_map(|p| match p.network_endpoints() {
                Some(v) => Some((v, p.source())),
                None => None,
            })
            .ok_or_else(|| ConfigError::MissingValue("network endpoints".to_string()))?;
        // Iterates over the list of `PartialConfig` objects to find the first config with a value
        // for the specific field. If no value is found, an error is returned.
        Ok(Config {
            config_dir,
            storage: self
                .partial_configs
                .iter()
                .find_map(|p| match p.storage() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("storage".to_string()))?,
            tls_cert_dir,
            tls_ca_file,
            tls_client_cert,
            tls_client_key,
            tls_server_cert,
            tls_server_key,
            service_endpoint: self
                .partial_configs
                .iter()
                .find_map(|p| match p.service_endpoint() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("service endpoint".to_string()))?,
            advertised_endpoints: self
                .partial_configs
                .iter()
                .find_map(|p| match p.advertised_endpoints() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                // Default to whatever `network_endpoints` is set to
                .unwrap_or((network_endpoints.0.clone(), ConfigSource::Default)),
            network_endpoints,
            peers: self
                .partial_configs
                .iter()
                .find_map(|p| match p.peers() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("peers".to_string()))?,
            display_name: self
                .partial_configs
                .iter()
                .find_map(|p| match p.display_name() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                }),
            node_id: self.partial_configs.iter().find_map(|p| match p.node_id() {
                Some(v) => Some((v, p.source())),
                None => None,
            }),
            rest_api_endpoint: self
                .partial_configs
                .iter()
                .find_map(|p| match p.rest_api_endpoint() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("rest api endpoint".to_string()))?,
            #[cfg(feature = "database")]
            database: self
                .partial_configs
                .iter()
                .find_map(|p| match p.database() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("database".to_string()))?,
            registries: self
                .partial_configs
                .iter()
                .find_map(|p| match p.registries() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("registries".to_string()))?,
            registry_auto_refresh: self
                .partial_configs
                .iter()
                .find_map(|p| match p.registry_auto_refresh() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| {
                    ConfigError::MissingValue("registry auto refresh interval".to_string())
                })?,
            registry_forced_refresh: self
                .partial_configs
                .iter()
                .find_map(|p| match p.registry_forced_refresh() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| {
                    ConfigError::MissingValue("registry forced refresh interval".to_string())
                })?,
            heartbeat: self
                .partial_configs
                .iter()
                .find_map(|p| match p.heartbeat() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("heartbeat interval".to_string()))?,
            admin_timeout: self
                .partial_configs
                .iter()
                .find_map(|p| match p.admin_timeout() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| {
                    ConfigError::MissingValue("admin service coordinator timeout".to_string())
                })?,

            state_dir: self
                .partial_configs
                .iter()
                .find_map(|p| match p.state_dir() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("state directory".to_string()))?,
            tls_insecure: self
                .partial_configs
                .iter()
                .find_map(|p| match p.tls_insecure() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("insecure".to_string()))?,
            no_tls: self
                .partial_configs
                .iter()
                .find_map(|p| match p.no_tls() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("no tls".to_string()))?,
            #[cfg(feature = "biome")]
            enable_biome: self
                .partial_configs
                .iter()
                .find_map(|p| match p.enable_biome() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                })
                .ok_or_else(|| ConfigError::MissingValue("enable_biome".to_string()))?,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self
                .partial_configs
                .iter()
                .find_map(|p| match p.whitelist() {
                    Some(v) => Some((v, p.source())),
                    None => None,
                }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Example configuration values.
    static EXAMPLE_STORAGE: &str = "yaml";
    static EXAMPLE_CA_CERTS: &str = "/etc/splinter/certs/ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "/etc/splinter/certs/client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "/etc/splinter/certs/client.key";
    static EXAMPLE_SERVER_CERT: &str = "/etc/splinter/certs/server.crt";
    static EXAMPLE_SERVER_KEY: &str = "/etc/splinter/certs/server.key";
    static EXAMPLE_SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static EXAMPLE_NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
    static EXAMPLE_ADVERTISED_ENDPOINT: &str = "localhost:8044";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";

    /// Asserts the example configuration values.
    fn assert_config_values(config: PartialConfig) {
        assert_eq!(config.storage(), Some(EXAMPLE_STORAGE.to_string()));
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
        #[cfg(feature = "database")]
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
            .with_storage(Some(EXAMPLE_STORAGE.to_string()))
            .with_tls_cert_dir(None)
            .with_tls_ca_file(Some(EXAMPLE_CA_CERTS.to_string()))
            .with_tls_client_cert(Some(EXAMPLE_CLIENT_CERT.to_string()))
            .with_tls_client_key(Some(EXAMPLE_CLIENT_KEY.to_string()))
            .with_tls_server_cert(Some(EXAMPLE_SERVER_CERT.to_string()))
            .with_tls_server_key(Some(EXAMPLE_SERVER_KEY.to_string()))
            .with_service_endpoint(Some(EXAMPLE_SERVICE_ENDPOINT.to_string()))
            .with_network_endpoints(Some(vec![EXAMPLE_NETWORK_ENDPOINT.to_string()]))
            .with_advertised_endpoints(Some(vec![EXAMPLE_ADVERTISED_ENDPOINT.to_string()]))
            .with_peers(Some(vec![]))
            .with_node_id(Some(EXAMPLE_NODE_ID.to_string()))
            .with_display_name(Some(EXAMPLE_DISPLAY_NAME.to_string()))
            .with_rest_api_endpoint(None)
            .with_registries(Some(vec![]))
            .with_heartbeat(None)
            .with_admin_timeout(None);
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
        partial_config = partial_config.with_storage(Some(EXAMPLE_STORAGE.to_string()));
        partial_config = partial_config.with_tls_ca_file(Some(EXAMPLE_CA_CERTS.to_string()));
        partial_config = partial_config.with_tls_client_cert(Some(EXAMPLE_CLIENT_CERT.to_string()));
        partial_config = partial_config.with_tls_client_key(Some(EXAMPLE_CLIENT_KEY.to_string()));
        partial_config = partial_config.with_tls_server_cert(Some(EXAMPLE_SERVER_CERT.to_string()));
        partial_config = partial_config.with_tls_server_key(Some(EXAMPLE_SERVER_KEY.to_string()));
        partial_config =
            partial_config.with_service_endpoint(Some(EXAMPLE_SERVICE_ENDPOINT.to_string()));
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
