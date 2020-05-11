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

//! `PartialConfig` builder using values from a toml config file.

use crate::config::PartialConfigBuilder;
use crate::config::{ConfigError, ConfigSource, PartialConfig};

use serde_derive::Deserialize;

/// `TOML_VERSION` represents the version of the toml config file.
/// The version determines the most current valid toml config entries.
const TOML_VERSION: &str = "1";

/// `TomlConfig` object which holds values defined in a toml file. This struct must be
/// treated as part of the external API of splinter because changes here
/// will impact the valid format of the config file.
#[derive(Deserialize, Default, Debug)]
struct TomlConfig {
    storage: Option<String>,
    tls_cert_dir: Option<String>,
    tls_ca_file: Option<String>,
    tls_client_cert: Option<String>,
    tls_client_key: Option<String>,
    tls_server_cert: Option<String>,
    tls_server_key: Option<String>,
    service_endpoint: Option<String>,
    network_endpoints: Option<Vec<String>>,
    advertised_endpoints: Option<Vec<String>>,
    peers: Option<Vec<String>>,
    node_id: Option<String>,
    display_name: Option<String>,
    rest_api_endpoint: Option<String>,
    #[cfg(feature = "database")]
    database: Option<String>,
    registries: Option<Vec<String>>,
    registry_auto_refresh: Option<u64>,
    registry_forced_refresh: Option<u64>,
    heartbeat: Option<u64>,
    admin_timeout: Option<u64>,
    version: Option<String>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,

    // Deprecated values
    cert_dir: Option<String>,
    ca_certs: Option<String>,
    client_cert: Option<String>,
    client_key: Option<String>,
    server_cert: Option<String>,
    server_key: Option<String>,
    heartbeat_interval: Option<u64>,
    registry_auto_refresh_interval: Option<u64>,
    registry_forced_refresh_interval: Option<u64>,
    admin_service_coordinator_timeout: Option<u64>,
    bind: Option<String>,
}

/// `PartialConfig` builder which holds values defined in a toml file.
pub struct TomlPartialConfigBuilder {
    source: Option<ConfigSource>,
    toml_config: TomlConfig,
}

/// Takes a toml file, represented as a string, and the path to the toml file to
/// construct a `TomlPartialConfigBuilder`.
impl TomlPartialConfigBuilder {
    pub fn new(toml: String, toml_path: String) -> Result<TomlPartialConfigBuilder, ConfigError> {
        Ok(TomlPartialConfigBuilder {
            source: Some(ConfigSource::Toml { file: toml_path }),
            toml_config: toml::from_str::<TomlConfig>(&toml).map_err(ConfigError::from)?,
        })
    }
}

/// Implementation of the `PartialConfigBuilder` trait to create a `PartialConfig` object from the
/// toml config file entries.
impl PartialConfigBuilder for TomlPartialConfigBuilder {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let source = match self.source {
            Some(s) => s,
            None => ConfigSource::Toml {
                file: String::from(""),
            },
        };

        if let Some(version) = self.toml_config.version {
            if version != TOML_VERSION {
                let file_path = match &source {
                    ConfigSource::Toml { file } => file.clone(),
                    _ => String::from(""),
                };
                return Err(ConfigError::InvalidVersion(format!(
                    "Config file {} has incompatible version {}, supported version is {}",
                    file_path, version, TOML_VERSION,
                )));
            }
        } else {
            return Err(ConfigError::MissingValue(format!("{:?} version", &source)));
        }

        let mut partial_config = PartialConfig::new(source);

        partial_config = partial_config
            // with current values
            .with_storage(self.toml_config.storage)
            .with_tls_cert_dir(self.toml_config.tls_cert_dir)
            .with_tls_ca_file(self.toml_config.tls_ca_file)
            .with_tls_client_cert(self.toml_config.tls_client_cert)
            .with_tls_client_key(self.toml_config.tls_client_key)
            .with_tls_server_cert(self.toml_config.tls_server_cert)
            .with_tls_server_key(self.toml_config.tls_server_key)
            .with_service_endpoint(self.toml_config.service_endpoint)
            .with_network_endpoints(self.toml_config.network_endpoints)
            .with_advertised_endpoints(self.toml_config.advertised_endpoints)
            .with_peers(self.toml_config.peers)
            .with_node_id(self.toml_config.node_id)
            .with_display_name(self.toml_config.display_name)
            .with_rest_api_endpoint(self.toml_config.rest_api_endpoint)
            .with_registries(self.toml_config.registries)
            .with_registry_auto_refresh(self.toml_config.registry_auto_refresh)
            .with_registry_forced_refresh(self.toml_config.registry_forced_refresh)
            .with_heartbeat(self.toml_config.heartbeat)
            .with_admin_timeout(self.toml_config.admin_timeout);

        #[cfg(feature = "database")]
        {
            partial_config = partial_config.with_database(self.toml_config.database);
        }

        #[cfg(feature = "rest-api-cors")]
        {
            partial_config = partial_config.with_whitelist(self.toml_config.whitelist);
        }

        // deprecated values, only set if the current value was not set
        if partial_config.tls_cert_dir().is_none() {
            partial_config = partial_config.with_tls_cert_dir(self.toml_config.cert_dir)
        }
        if partial_config.tls_ca_file().is_none() {
            partial_config = partial_config.with_tls_ca_file(self.toml_config.ca_certs)
        }
        if partial_config.tls_client_cert().is_none() {
            partial_config = partial_config.with_tls_client_cert(self.toml_config.client_cert)
        }
        if partial_config.tls_client_key().is_none() {
            partial_config = partial_config.with_tls_client_key(self.toml_config.client_key)
        }
        if partial_config.tls_server_cert().is_none() {
            partial_config = partial_config.with_tls_server_cert(self.toml_config.server_cert)
        }
        if partial_config.tls_server_key().is_none() {
            partial_config = partial_config.with_tls_server_key(self.toml_config.server_key)
        }
        if partial_config.heartbeat().is_none() {
            partial_config = partial_config.with_heartbeat(self.toml_config.heartbeat_interval)
        }
        if partial_config.registry_auto_refresh().is_none() {
            partial_config = partial_config
                .with_registry_auto_refresh(self.toml_config.registry_auto_refresh_interval)
        }
        if partial_config.registry_forced_refresh().is_none() {
            partial_config = partial_config
                .with_registry_forced_refresh(self.toml_config.registry_forced_refresh_interval)
        }
        if partial_config.admin_timeout().is_none() {
            partial_config = partial_config
                .with_admin_timeout(self.toml_config.admin_service_coordinator_timeout)
        }
        if partial_config.rest_api_endpoint().is_none() {
            partial_config = partial_config.with_rest_api_endpoint(self.toml_config.bind)
        }

        Ok(partial_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    use toml::{map::Map, Value};

    /// Path to an example config toml file.
    static TEST_TOML: &str = "config_test.toml";

    /// Example configuration values.
    static EXAMPLE_STORAGE: &str = "yaml";
    static EXAMPLE_CERT_DIR: &str = "/cert_dir";
    static EXAMPLE_CA_CERTS: &str = "certs/ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "certs/client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "certs/client.key";
    static EXAMPLE_SERVER_CERT: &str = "certs/server.crt";
    static EXAMPLE_SERVER_KEY: &str = "certs/server.key";
    static EXAMPLE_SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";
    static EXAMPLE_HEARTBEAT: u64 = 20;
    static EXAMPLE_REGISTRY_AUTO: u64 = 19;
    static EXAMPLE_REGISTRY_FORCE: u64 = 18;
    static EXAMPLE_ADMIN_TIMEOUT: u64 = 17;

    /// Converts a list of tuples to a toml `Table` `Value` used to write a toml file.
    fn get_toml_value() -> Value {
        let values = vec![
            ("storage".to_string(), EXAMPLE_STORAGE.to_string()),
            ("tls_ca_file".to_string(), EXAMPLE_CA_CERTS.to_string()),
            (
                "tls_client_cert".to_string(),
                EXAMPLE_CLIENT_CERT.to_string(),
            ),
            ("tls_client_key".to_string(), EXAMPLE_CLIENT_KEY.to_string()),
            (
                "tls_server_cert".to_string(),
                EXAMPLE_SERVER_CERT.to_string(),
            ),
            ("tls_server_key".to_string(), EXAMPLE_SERVER_KEY.to_string()),
            (
                "service_endpoint".to_string(),
                EXAMPLE_SERVICE_ENDPOINT.to_string(),
            ),
            ("node_id".to_string(), EXAMPLE_NODE_ID.to_string()),
            ("display_name".to_string(), EXAMPLE_DISPLAY_NAME.to_string()),
            ("version".to_string(), TOML_VERSION.to_string()),
        ];

        let mut config_values = Map::new();
        values.iter().for_each(|v| {
            config_values.insert(v.0.clone(), Value::String(v.1.clone()));
        });
        Value::Table(config_values)
    }

    /// Converts a list of tuples to a toml `Table` `Value` used to write a toml file.
    fn get_deprecated_toml_value() -> Value {
        let values = vec![
            ("cert_dir".to_string(), EXAMPLE_CERT_DIR.to_string()),
            ("ca_certs".to_string(), EXAMPLE_CA_CERTS.to_string()),
            ("client_cert".to_string(), EXAMPLE_CLIENT_CERT.to_string()),
            ("client_key".to_string(), EXAMPLE_CLIENT_KEY.to_string()),
            ("server_cert".to_string(), EXAMPLE_SERVER_CERT.to_string()),
            ("server_key".to_string(), EXAMPLE_SERVER_KEY.to_string()),
            ("version".to_string(), TOML_VERSION.to_string()),
        ];

        let mut config_values = Map::new();
        values.iter().for_each(|v| {
            config_values.insert(v.0.clone(), Value::String(v.1.clone()));
        });

        let u64_values = vec![
            ("heartbeat_interval".to_string(), EXAMPLE_HEARTBEAT),
            (
                "registry_auto_refresh_interval".to_string(),
                EXAMPLE_REGISTRY_AUTO,
            ),
            (
                "registry_forced_refresh_interval".to_string(),
                EXAMPLE_REGISTRY_FORCE,
            ),
            (
                "admin_service_coordinator_timeout".to_string(),
                EXAMPLE_ADMIN_TIMEOUT,
            ),
        ];

        u64_values.iter().for_each(|v| {
            config_values.insert(v.0.clone(), Value::Integer(v.1.clone() as i64));
        });
        Value::Table(config_values)
    }

    /// Asserts config values based on the example configuration values.
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
        assert_eq!(config.network_endpoints(), None);
        assert_eq!(config.advertised_endpoints(), None);
        assert_eq!(config.peers(), None);
        assert_eq!(config.node_id(), Some(EXAMPLE_NODE_ID.to_string()));
        assert_eq!(
            config.display_name(),
            Some(EXAMPLE_DISPLAY_NAME.to_string())
        );
        assert_eq!(config.rest_api_endpoint(), None);
        #[cfg(feature = "database")]
        assert_eq!(config.database(), None);
        assert_eq!(config.registries(), None);
        assert_eq!(config.registry_auto_refresh(), None);
        assert_eq!(config.registry_forced_refresh(), None);
        assert_eq!(config.heartbeat(), None);
        assert_eq!(config.admin_timeout(), None);
    }

    /// Asserts config values based on the example configuration values.
    fn assert_deprecated_config_values(config: PartialConfig) {
        assert_eq!(config.storage(), None);
        assert_eq!(config.tls_cert_dir(), Some(EXAMPLE_CERT_DIR.to_string()));
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
        assert_eq!(config.service_endpoint(), None);
        assert_eq!(config.network_endpoints(), None);
        assert_eq!(config.advertised_endpoints(), None);
        assert_eq!(config.peers(), None);
        assert_eq!(config.node_id(), None);
        assert_eq!(config.display_name(), None);
        assert_eq!(config.rest_api_endpoint(), None);
        #[cfg(feature = "database")]
        assert_eq!(config.database(), None);
        assert_eq!(config.registries(), None);
        assert_eq!(config.heartbeat(), Some(20));
        assert_eq!(config.registry_auto_refresh(), Some(19));
        assert_eq!(config.registry_forced_refresh(), Some(18));
        assert_eq!(config.admin_timeout(), Some(Duration::from_secs(17)));
    }

    #[test]
    /// This test verifies that a `PartialConfig `object, constructed from the
    /// `TomlPartialConfigBuilder` module, contains the correct values using the following steps:
    ///
    /// 1. An example config toml is string is created.
    /// 2. A `TomlPartialConfigBuilder` object is constructed by passing in the toml string created
    ///    in the previous step.
    /// 3. The `TomlPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    ///
    /// This test then verifies the `PartialConfig` object built from the `TomlPartialConfigBuilder`
    /// object by asserting each expected value.
    fn test_toml_build() {
        // Create an example toml string.
        let toml_string = toml::to_string(&get_toml_value()).expect("Could not encode TOML value");
        // Create a `TomlPartialConfigBuilder` object from the toml string.
        let toml_builder = TomlPartialConfigBuilder::new(toml_string, TEST_TOML.to_string())
            .expect(&format!(
                "Unable to create TomlPartialConfigBuilder from: {}",
                TEST_TOML
            ));
        // Build a `PartialConfig` from the `TomlPartialConfigBuilder `object created.
        let built_config = toml_builder
            .build()
            .expect("Unable to build TomlPartialConfigBuilder");
        // Compare the generated `PartialConfig` object against the expected values.
        assert_config_values(built_config);
    }

    #[test]
    /// This test verifies that a `PartialConfig` object, constructed from the
    /// `TomlPartialConfigBuilder` module, contains the correct values when using deprecated values:
    ///
    /// 1. An example config toml string is created that is only made up of deprecated tls values
    /// 2. A `TomlPartialConfigBuilder` object is constructed by passing in the toml string created
    ///    in the previous step.
    /// 3. The `TomlPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    ///
    /// This test then verifies the `PartialConfig` object built from the `TomlPartialConfigBuilder`
    /// object by asserting each expected tls value was properly set from deprecated values
    fn test_deprecated_toml_build() {
        // Create an example toml string.
        let toml_string =
            toml::to_string(&get_deprecated_toml_value()).expect("Could not encode TOML value");
        // Create a `TomlPartialConfigBuilder` object from the toml string.
        let toml_builder = TomlPartialConfigBuilder::new(toml_string, TEST_TOML.to_string())
            .expect(&format!(
                "Unable to create TomlPartialConfigBuilder from: {}",
                TEST_TOML
            ));
        // Build a `PartialConfig` from the `TomlPartialConfigBuilder` object created.
        let built_config = toml_builder
            .build()
            .expect("Unable to build TomlPartialConfigBuilder");
        // Compare the generated `PartialConfig` object against the expected values.
        assert_deprecated_config_values(built_config);
    }
}
