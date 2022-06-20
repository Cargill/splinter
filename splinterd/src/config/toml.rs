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

//! `PartialConfig` builder using values from a toml config file.

use crate::config::PartialConfigBuilder;
use crate::config::{ConfigError, ConfigSource, PartialConfig};
use log::Level;
use serde::de::Visitor;
use serde::Deserialize as DeserializeTrait;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::convert::TryInto;

use super::logging::{default_pattern, UnnamedAppenderConfig, UnnamedLoggerConfig};
use super::ScabbardState;

/// `TOML_VERSION` represents the version of the toml config file.
/// The version determines the most current valid toml config entries.
const TOML_VERSION: &str = "1";

#[derive(Deserialize, Clone, Debug)]
pub enum TomlRawLogTarget {
    #[serde(alias = "stdout")]
    Stdout,
    #[serde(alias = "stderr")]
    Stderr,
    #[serde(alias = "file")]
    File,
    #[serde(alias = "rolling_file")]
    RollingFile,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TomlUnnamedAppenderConfig {
    #[serde(default = "default_pattern")]
    #[serde(alias = "pattern")]
    pub encoder: String,
    pub kind: TomlRawLogTarget,
    pub filename: Option<String>,
    pub size: Option<TomlLogFileSize>,
    pub level: Option<TomlLogLevel>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TomlUnnamedLoggerConfig {
    pub appenders: Option<Vec<String>>,
    pub level: Option<TomlLogLevel>,
}

#[derive(Deserialize, Clone, Debug)]
pub enum TomlLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<TomlLogLevel> for Level {
    fn from(toml: TomlLogLevel) -> Self {
        match toml {
            TomlLogLevel::Warn => Level::Warn,
            TomlLogLevel::Info => Level::Info,
            TomlLogLevel::Error => Level::Error,
            TomlLogLevel::Debug => Level::Debug,
            TomlLogLevel::Trace => Level::Trace,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TomlLogFileSize {
    size: u64,
}

impl From<TomlLogFileSize> for u64 {
    fn from(bytes: TomlLogFileSize) -> Self {
        bytes.size
    }
}

impl<'de> DeserializeTrait<'de> for TomlLogFileSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(TomlLogFileSizeVisitor)
    }
}

struct TomlLogFileSizeVisitor;

impl<'de> Visitor<'de> for TomlLogFileSizeVisitor {
    type Value = TomlLogFileSize;
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // serde calls these methods hints and its not always clear which method gets used. Hence
        // the visit_string and visitr_str methods both being defined.
        self.visit_str(&v)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        //floats support a bunch of different formats, this supports <digit[s]>.<digit[s]>
        let numeric: Result<f32, _> = v
            .chars()
            .take_while(|x| x.is_digit(10) || *x == '.')
            .collect::<String>()
            .parse();
        // Units can be K,M,G for kilo, mega, giga bytes.
        let multiple = v
            .chars()
            .skip_while(|x| x.is_digit(10) || *x == '.')
            .take_while(|c| c.is_alphabetic())
            .collect::<String>();
        let multiple = match multiple.as_str() {
            "M" => Ok(1_000_000),
            "K" => Ok(1_000),
            "G" => Ok(1_000_000_000),
            _ => Err(E::custom("unit could not be parsed".to_string())),
        };
        match (numeric, multiple) {
            (Ok(float), Ok(mult)) => Ok(TomlLogFileSize {
                size: (float * mult as f32).trunc() as u64,
            }),
            (Err(e), _) => Err(E::custom(format!("size could not be parsed: {}", e))),
            (_, Err(e)) => Err(e),
        }
    }
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "<float><K|M|G>")
    }
}

/// `TomlConfig` object which holds values defined in a toml file. This struct must be
/// treated as part of the external API of splinter because changes here
/// will impact the valid format of the config file.
#[derive(Deserialize, Default, Debug)]
struct TomlConfig {
    tls_cert_dir: Option<String>,
    tls_ca_file: Option<String>,
    tls_client_cert: Option<String>,
    tls_client_key: Option<String>,
    tls_server_cert: Option<String>,
    tls_server_key: Option<String>,
    #[cfg(feature = "https-bind")]
    tls_rest_api_cert: Option<String>,
    #[cfg(feature = "https-bind")]
    tls_rest_api_key: Option<String>,
    #[cfg(feature = "service-endpoint")]
    service_endpoint: Option<String>,
    network_endpoints: Option<Vec<String>>,
    advertised_endpoints: Option<Vec<String>>,
    peers: Option<Vec<String>>,
    node_id: Option<String>,
    display_name: Option<String>,
    rest_api_endpoint: Option<String>,
    database: Option<String>,
    registries: Option<Vec<String>>,
    registry_auto_refresh: Option<u64>,
    registry_forced_refresh: Option<u64>,
    heartbeat: Option<u64>,
    admin_timeout: Option<u64>,
    version: Option<String>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "oauth")]
    oauth_provider: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_id: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_secret: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_redirect_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_auth_params: Option<Vec<(String, String)>>,
    #[cfg(feature = "oauth")]
    oauth_openid_scopes: Option<Vec<String>>,
    #[cfg(feature = "tap")]
    influx_db: Option<String>,
    #[cfg(feature = "tap")]
    influx_url: Option<String>,
    #[cfg(feature = "tap")]
    influx_username: Option<String>,
    #[cfg(feature = "tap")]
    influx_password: Option<String>,
    peering_key: Option<String>,
    appenders: Option<HashMap<String, TomlUnnamedAppenderConfig>>,
    loggers: Option<HashMap<String, TomlUnnamedLoggerConfig>>,
    scabbard_state: Option<ScabbardStateToml>,
    #[cfg(feature = "disable-scabbard-autocleanup")]
    scabbard_enable_autocleanup: Option<bool>,
    config_dir: Option<String>,
    state_dir: Option<String>,

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
            .with_tls_cert_dir(self.toml_config.tls_cert_dir)
            .with_tls_ca_file(self.toml_config.tls_ca_file)
            .with_tls_client_cert(self.toml_config.tls_client_cert)
            .with_tls_client_key(self.toml_config.tls_client_key)
            .with_tls_server_cert(self.toml_config.tls_server_cert)
            .with_tls_server_key(self.toml_config.tls_server_key)
            .with_network_endpoints(self.toml_config.network_endpoints)
            .with_advertised_endpoints(self.toml_config.advertised_endpoints)
            .with_peers(self.toml_config.peers)
            .with_node_id(self.toml_config.node_id)
            .with_display_name(self.toml_config.display_name)
            .with_rest_api_endpoint(self.toml_config.rest_api_endpoint)
            .with_database(self.toml_config.database)
            .with_registries(self.toml_config.registries)
            .with_registry_auto_refresh(self.toml_config.registry_auto_refresh)
            .with_registry_forced_refresh(self.toml_config.registry_forced_refresh)
            .with_heartbeat(self.toml_config.heartbeat)
            .with_admin_timeout(self.toml_config.admin_timeout)
            .with_peering_key(self.toml_config.peering_key)
            .with_config_dir(self.toml_config.config_dir)
            .with_state_dir(self.toml_config.state_dir)
            .with_scabbard_state(self.toml_config.scabbard_state.map(|inner| inner.into()));

        #[cfg(feature = "disable-scabbard-autocleanup")]
        {
            partial_config = partial_config
                .with_scabbard_autocleanup(self.toml_config.scabbard_enable_autocleanup);
        }

        #[cfg(feature = "https-bind")]
        {
            partial_config = partial_config
                .with_tls_rest_api_cert(self.toml_config.tls_rest_api_cert)
                .with_tls_rest_api_key(self.toml_config.tls_rest_api_key);
        }

        #[cfg(feature = "service-endpoint")]
        {
            partial_config = partial_config.with_service_endpoint(self.toml_config.service_endpoint)
        }

        #[cfg(feature = "rest-api-cors")]
        {
            partial_config = partial_config.with_whitelist(self.toml_config.whitelist);
        }

        #[cfg(feature = "oauth")]
        {
            partial_config = partial_config
                .with_oauth_provider(self.toml_config.oauth_provider)
                .with_oauth_client_id(self.toml_config.oauth_client_id)
                .with_oauth_client_secret(self.toml_config.oauth_client_secret)
                .with_oauth_redirect_url(self.toml_config.oauth_redirect_url)
                .with_oauth_openid_url(self.toml_config.oauth_openid_url)
                .with_oauth_openid_auth_params(self.toml_config.oauth_openid_auth_params)
                .with_oauth_openid_scopes(self.toml_config.oauth_openid_scopes);
        }

        #[cfg(feature = "tap")]
        {
            partial_config = partial_config
                .with_influx_db(self.toml_config.influx_db)
                .with_influx_url(self.toml_config.influx_url)
                .with_influx_username(self.toml_config.influx_username)
                .with_influx_password(self.toml_config.influx_password)
        }

        if let Some(mut loggers) = self.toml_config.loggers {
            if let Some(unnamed) = loggers.remove("root") {
                partial_config = partial_config
                    .with_root_logger(Some(unnamed.try_into()?))
                    .with_loggers(Some(
                        loggers
                            .drain()
                            .map(|pair| (pair.0, pair.1.into()))
                            .collect::<HashMap<String, UnnamedLoggerConfig>>(),
                    ));
            } else {
                partial_config = partial_config.with_loggers(Some(
                    loggers
                        .drain()
                        .map(|pair| (pair.0, pair.1.into()))
                        .collect::<HashMap<String, UnnamedLoggerConfig>>(),
                ));
            }
        } else {
            partial_config = partial_config.with_loggers(None);
        }
        if let Some(mut appenders) = self.toml_config.appenders {
            partial_config = partial_config.with_appenders(Some(
                appenders
                    .drain()
                    .map(|(name, conf)| (name, conf.into()))
                    .collect::<HashMap<String, UnnamedAppenderConfig>>(),
            ));
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

#[derive(Deserialize, Debug)]
pub enum ScabbardStateToml {
    #[serde(rename = "database")]
    Database,
    #[serde(rename = "lmdb")]
    Lmdb,
}

impl From<ScabbardStateToml> for ScabbardState {
    fn from(other: ScabbardStateToml) -> Self {
        match other {
            ScabbardStateToml::Lmdb => ScabbardState::Lmdb,
            ScabbardStateToml::Database => ScabbardState::Database,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::LoggerConfig;

    use super::*;

    use std::time::Duration;

    use toml::{map::Map, Value};

    /// Path to an example config toml file.
    static TEST_TOML: &str = "config_test.toml";

    /// Example configuration values.
    static EXAMPLE_CERT_DIR: &str = "/cert_dir";
    static EXAMPLE_CA_CERTS: &str = "certs/ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "certs/client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "certs/client.key";
    static EXAMPLE_SERVER_CERT: &str = "certs/server.crt";
    static EXAMPLE_SERVER_KEY: &str = "certs/server.key";
    #[cfg(feature = "https-bind")]
    static EXAMPLE_REST_API_CERT: &str = "certs/rest_api.crt";
    #[cfg(feature = "https-bind")]
    static EXAMPLE_REST_API_KEY: &str = "certs/rest_api.key";
    #[cfg(feature = "service-endpoint")]
    static EXAMPLE_SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";
    static EXAMPLE_HEARTBEAT: u64 = 20;
    static EXAMPLE_REGISTRY_AUTO: u64 = 19;
    static EXAMPLE_REGISTRY_FORCE: u64 = 18;
    static EXAMPLE_ADMIN_TIMEOUT: u64 = 17;
    #[cfg(feature = "oauth")]
    static EXAMPLE_OAUTH_OPENID_AUTH_PARAM_KEY: &str = "key";
    #[cfg(feature = "oauth")]
    static EXAMPLE_OAUTH_OPENID_AUTH_PARAM_VAL: &str = "val";
    #[cfg(feature = "oauth")]
    static EXAMPLE_OAUTH_OPENID_SCOPE: &str = "scope";

    /// Converts a list of tuples to a toml `Table` `Value` used to write a toml file.
    fn get_toml_value() -> Value {
        let values = vec![
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
            #[cfg(feature = "https-bind")]
            (
                "tls_rest_api_cert".to_string(),
                EXAMPLE_REST_API_CERT.to_string(),
            ),
            #[cfg(feature = "https-bind")]
            (
                "tls_rest_api_key".to_string(),
                EXAMPLE_REST_API_KEY.to_string(),
            ),
            #[cfg(feature = "service-endpoint")]
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

        #[cfg(feature = "oauth")]
        {
            config_values.insert(
                "oauth_openid_auth_params".into(),
                Value::try_from(vec![vec![
                    EXAMPLE_OAUTH_OPENID_AUTH_PARAM_KEY.to_string(),
                    EXAMPLE_OAUTH_OPENID_AUTH_PARAM_VAL.to_string(),
                ]])
                .expect("Failed to parse oauth_openid_auth_params"),
            );
            config_values.insert(
                "oauth_openid_scopes".into(),
                Value::try_from(vec![EXAMPLE_OAUTH_OPENID_SCOPE])
                    .expect("Failed to parse oauth_openid_scopes"),
            );
        }

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
        assert_eq!(config.network_endpoints(), None);
        assert_eq!(config.advertised_endpoints(), None);
        assert_eq!(config.peers(), None);
        assert_eq!(config.node_id(), Some(EXAMPLE_NODE_ID.to_string()));
        assert_eq!(
            config.display_name(),
            Some(EXAMPLE_DISPLAY_NAME.to_string())
        );
        assert_eq!(config.rest_api_endpoint(), None);
        assert_eq!(config.database(), None);
        assert_eq!(config.registries(), None);
        assert_eq!(config.registry_auto_refresh(), None);
        assert_eq!(config.registry_forced_refresh(), None);
        assert_eq!(config.heartbeat(), None);
        assert_eq!(config.admin_timeout(), None);
        #[cfg(feature = "oauth")]
        assert_eq!(
            config.oauth_openid_auth_params(),
            Some(vec![(
                EXAMPLE_OAUTH_OPENID_AUTH_PARAM_KEY.into(),
                EXAMPLE_OAUTH_OPENID_AUTH_PARAM_VAL.into()
            )])
        );
        #[cfg(feature = "oauth")]
        assert_eq!(
            config.oauth_openid_scopes(),
            Some(vec![EXAMPLE_OAUTH_OPENID_SCOPE.into()])
        );
    }

    /// Asserts config values based on the example configuration values.
    fn assert_deprecated_config_values(config: PartialConfig) {
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
        #[cfg(feature = "service-endpoint")]
        assert_eq!(config.service_endpoint(), None);
        assert_eq!(config.network_endpoints(), None);
        assert_eq!(config.advertised_endpoints(), None);
        assert_eq!(config.peers(), None);
        assert_eq!(config.node_id(), None);
        assert_eq!(config.display_name(), None);
        assert_eq!(config.rest_api_endpoint(), None);
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

    static FULL_TOML_CONFIG: &str = r#"
            version = "1"
            config_dir = "/etc/splinter"
            state_dir = "/var/lib/splinter"
            database = "splinter_state.db"
            node_id = "node_id"
            display_name = "display_name"
            network_endpoints = [ "tcps://127.0.0.1:8044" ]
            rest_api_endpoint = "http://127.0.0.1:8080"
            advertised_endpoints = [ "tcps://127.0.0.1:8044" ]
            peers = ["splinter.dev"]
            peering_key = "splinterd"
            heartbeat = 30
            admin_timeout = 30
            allow_keys_file = "allow_keys"
            registries = ["file:///etc/splinter/registry.yaml"]
            registry_auto_refresh = 600
            registry_forced_refresh = 10
            tls_cert_dir = "/etc/splinter/certs"
            tls_ca_file = "/etc/splinter/certs/ca.pem"
            tls_client_cert = "/etc/splinter/certs/client.crt"
            tls_client_key = "/etc/splinter/certs/private/client.key"
            tls_server_cert = "/etc/splinter/certs/server.crt"
            tls_server_key = "/etc/splinter/certs/private/server.key"
            oauth_provider = "google"
            oauth_client_id = "qwerty"
            oauth_client_secret = "QWERTY"
            oauth_redirect_url = "splinter.dev"
            oauth_openid_url = "splinter.dev"
            oauth_openid_auth_params = [["test","test1"]]
            oauth_openid_scopes = ["test"]
            influx_url = "splinter.dev"
            influx_db = "database"
            influx_username = "username"
            influx_password = "pa$$w0rd"
            [appenders.stdout]
            kind = "stdout"
            pattern = "[{d(%Y-%m-%d %H:%M:%S%.3f)}] T[{T}] {l} [{M}] {m}\n"
            [appenders.rolling_file]
            kind = "rolling_file"
            filename = "/var/log/splinter/splinterd.log"
            size = "16.0M"
            [loggers.splinter]
            appenders = [ "stdout", "rolling_file"]
            level = "Warn"
        "#;
    #[test]
    fn test_full_toml_config() {
        let toml = TomlPartialConfigBuilder::new(
            FULL_TOML_CONFIG.to_string(),
            "fake_file_path".to_string(),
        )
        .expect("Could not deserialize full toml")
        .build()
        .expect("A config error has occured");
        assert!(matches!(toml.config_dir(), Some(text) if text == "/etc/splinter"));
        assert!(matches!(toml.state_dir() , Some(text) if text == "/var/lib/splinter"));
        assert!(matches!(toml.database() , Some(text) if text == "splinter_state.db"));
        assert!(matches!(toml.node_id() , Some(text) if text == "node_id"));
        assert!(matches!(toml.display_name() , Some(text) if text == "display_name"));
        assert!(
            matches!(toml.network_endpoints() , Some(vec) if    matches!(vec.get(0), Some(text) if text == "tcps://127.0.0.1:8044"))
        );
        assert!(matches!(toml.rest_api_endpoint() , Some(text) if text == "http://127.0.0.1:8080"));
        assert!(
            matches!(toml.advertised_endpoints() , Some(vec) if matches!(vec.get(0), Some(text) if text == "tcps://127.0.0.1:8044")  )
        );
        assert!(
            matches!(toml.peers(), Some(vec) if matches!(vec.get(0), Some(text) if text == "splinter.dev") )
        );
        assert!(matches!(toml.peering_key() , Some(text) if text == "splinterd"));
        assert!(matches!(toml.heartbeat(), Some(30)));
        assert!(matches!(
            toml.admin_timeout(),
            Some(duration) if duration == Duration::from_secs(30)
        ));
        //assert!(matches!(toml.allow_keys_file() , Some(text) if text == "allow_keys"));
        assert!(
            matches!(toml.registries() ,Some(vec) if vec[..] == ["file:///etc/splinter/registry.yaml"])
        );
        assert!(matches!(toml.registry_auto_refresh(), Some(600)));
        assert!(matches!(toml.registry_forced_refresh(), Some(10)));
        assert!(matches!(toml.tls_cert_dir() , Some(text) if text == "/etc/splinter/certs"));
        assert!(matches!(toml.tls_ca_file() , Some(text) if text == "/etc/splinter/certs/ca.pem"));
        assert!(
            matches!(toml.tls_client_cert() , Some(text) if text == "/etc/splinter/certs/client.crt")
        );
        assert!(
            matches!(toml.tls_client_key() , Some(text) if text == "/etc/splinter/certs/private/client.key")
        );
        assert!(
            matches!(toml.tls_server_cert() , Some(text) if text == "/etc/splinter/certs/server.crt")
        );
        assert!(
            matches!(toml.tls_server_key() , Some(text) if text == "/etc/splinter/certs/private/server.key")
        );

        #[cfg(feature = "oauth")]
        {
            assert!(matches!(toml.oauth_provider() , Some(text) if text == "google"));
            assert!(matches!(toml.oauth_client_id() , Some(text) if text == "qwerty"));
            assert!(matches!(toml.oauth_client_secret() , Some(text) if text == "QWERTY"));
            assert!(matches!(toml.oauth_redirect_url() , Some(text) if text == "splinter.dev"));
            assert!(matches!(toml.oauth_openid_url() , Some(text) if text == "splinter.dev"));
            assert!(
                matches!(toml.oauth_openid_auth_params(), Some(vec) if matches!(vec.get(0),Some(pair) if pair == &("test".to_string(), "test1".to_string())))
            );
            assert!(
                matches!(toml.oauth_openid_scopes(), Some(vec) if matches!(vec.get(0), Some(val) if val == "test"))
            );
        }

        #[cfg(feature = "tap")]
        {
            assert!(matches!(toml.influx_url() , Some(text) if text == "splinter.dev"));
            assert!(matches!(toml.influx_db() , Some(text) if text == "database"));
            assert!(matches!(toml.influx_username() , Some(text) if text == "username"));
            assert!(matches!(toml.influx_password() , Some(text) if text == "pa$$w0rd"));
        }

        let appenders = toml.appenders();
        assert!(appenders.is_some());
        let appenders = appenders.unwrap();
        assert!(appenders.contains_key("stdout"));
        assert!(appenders.get("stdout").is_some());
        let stdout = appenders.get("stdout").unwrap();
        assert!(matches!(stdout.kind, crate::config::RawLogTarget::Stdout));
        assert!(stdout.size.is_none());
        assert!(stdout.filename.is_none());
        assert_eq!(stdout.encoder, default_pattern());

        assert!(appenders.contains_key("rolling_file"));
        assert!(appenders.get("rolling_file").is_some());
        let rolling_file = appenders.get("rolling_file").unwrap();
        assert!(matches!(
            rolling_file.kind,
            crate::config::RawLogTarget::RollingFile
        ));
        assert!(rolling_file.size.is_some());
        assert_eq!(rolling_file.size.unwrap(), 16_000_000);
        assert!(rolling_file.filename.is_some());
        assert_eq!(
            rolling_file.filename.as_ref().unwrap(),
            "/var/log/splinter/splinterd.log"
        );
        assert_eq!(rolling_file.encoder, default_pattern());

        let loggers = toml.loggers();
        assert!(loggers.is_some());
        let loggers = loggers.unwrap();
        assert!(loggers.contains_key("splinter"));
        let splinter = loggers.get("splinter").unwrap();
        let splinter: LoggerConfig = ("splinter".to_string(), splinter.clone()).into();
        assert!(matches!(splinter.level,Some(level) if matches!(level, Level::Warn)));
        assert!(splinter.appenders.is_some());
        let appenders = splinter.appenders.unwrap();
        assert!(matches!(appenders.get(0), Some(val) if val == "stdout"));
        assert!(matches!(appenders.get(1), Some(val) if val == "rolling_file"));
    }
}
