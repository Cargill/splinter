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

//! `PartialConfig` builder using default values.

use std::collections::HashMap;

use crate::config::{ConfigError, ConfigSource, PartialConfig, PartialConfigBuilder};

use super::logging::{
    RootConfig, UnnamedAppenderConfig, UnnamedLoggerConfig, DEFAULT_LOGGING_PATTERN,
};
use super::ScabbardState;

const CONFIG_DIR: &str = "/etc/splinter";
const TLS_CERT_DIR: &str = "/etc/splinter/certs";
const STATE_DIR: &str = "/var/lib/splinter";

const TLS_CLIENT_CERT: &str = "client.crt";
const TLS_CLIENT_KEY: &str = "private/client.key";
const TLS_SERVER_CERT: &str = "server.crt";
const TLS_SERVER_KEY: &str = "private/server.key";
#[cfg(feature = "https-bind")]
const TLS_REST_API_CERT: &str = "rest_api.crt";
#[cfg(feature = "https-bind")]
const TLS_REST_API_KEY: &str = "private/rest_api.key";
const TLS_CA_FILE: &str = "ca.pem";

#[cfg(not(feature = "https-bind"))]
const REST_API_ENDPOINT: &str = "http://127.0.0.1:8080";
#[cfg(feature = "https-bind")]
const REST_API_ENDPOINT: &str = "https://127.0.0.1:8443";
#[cfg(feature = "service-endpoint")]
const SERVICE_ENDPOINT: &str = "tcp://127.0.0.1:8043";
const NETWORK_ENDPOINT: &str = "tcps://127.0.0.1:8044";
const DATABASE: &str = "splinter_state.db";

const REGISTRY_AUTO_REFRESH: u64 = 600; // 600 seconds = 10 minutes
const REGISTRY_FORCED_REFRESH: u64 = 10; // 10 seconds
const HEARTBEAT: u64 = 30; // 30 seconds
const ADMIN_TIMEOUT: u64 = 30; // 30 seconds

const PEERING_KEY_NAME: &str = "splinterd";

#[cfg(feature = "config-allow-keys")]
const ALLOW_KEYS_FILE: &str = "allow_keys";

pub struct DefaultPartialConfigBuilder;

impl DefaultPartialConfigBuilder {
    pub fn new() -> Self {
        DefaultPartialConfigBuilder {}
    }
}

/// Constructs a `PartialConfig` object from the `DefaultPartialConfigBuilder`.
impl PartialConfigBuilder for DefaultPartialConfigBuilder {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let mut partial_config = PartialConfig::new(ConfigSource::Default);

        partial_config = partial_config
            .with_config_dir(Some(String::from(CONFIG_DIR)))
            .with_tls_cert_dir(Some(String::from(TLS_CERT_DIR)))
            .with_tls_ca_file(Some(String::from(TLS_CA_FILE)))
            .with_tls_client_cert(Some(String::from(TLS_CLIENT_CERT)))
            .with_tls_client_key(Some(String::from(TLS_CLIENT_KEY)))
            .with_tls_server_cert(Some(String::from(TLS_SERVER_CERT)))
            .with_tls_server_key(Some(String::from(TLS_SERVER_KEY)))
            .with_network_endpoints(Some(vec![String::from(NETWORK_ENDPOINT)]))
            .with_peers(Some(vec![]))
            .with_rest_api_endpoint(Some(String::from(REST_API_ENDPOINT)))
            .with_database(Some(String::from(DATABASE)))
            .with_registries(Some(vec![]))
            .with_registry_auto_refresh(Some(REGISTRY_AUTO_REFRESH))
            .with_registry_forced_refresh(Some(REGISTRY_FORCED_REFRESH))
            .with_heartbeat(Some(HEARTBEAT))
            .with_admin_timeout(Some(ADMIN_TIMEOUT))
            .with_state_dir(Some(String::from(STATE_DIR)))
            .with_tls_insecure(Some(false))
            .with_no_tls(Some(false))
            .with_strict_ref_counts(Some(false))
            .with_peering_key(Some(String::from(PEERING_KEY_NAME)))
            .with_scabbard_state(Some(ScabbardState::Database))
            .with_scabbard_autocleanup(Some(true));

        #[cfg(feature = "https-bind")]
        {
            partial_config = partial_config
                .with_tls_rest_api_cert(Some(String::from(TLS_REST_API_CERT)))
                .with_tls_rest_api_key(Some(String::from(TLS_REST_API_KEY)));
        }

        #[cfg(feature = "service-endpoint")]
        {
            partial_config =
                partial_config.with_service_endpoint(Some(String::from(SERVICE_ENDPOINT)))
        }

        #[cfg(feature = "biome-credentials")]
        {
            partial_config = partial_config.with_enable_biome_credentials(Some(false))
        }

        let root_logger: Option<RootConfig> = Some(RootConfig {
            appenders: vec!["stdout".to_string()],
            level: log::Level::Warn,
        });
        let stdout = UnnamedAppenderConfig {
            encoder: String::from(DEFAULT_LOGGING_PATTERN),
            kind: super::logging::RawLogTarget::Stdout,
            size: None,
            filename: None,
            level: None,
        };
        let loggers = vec![
            (
                "splinter".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "splinterd".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "sabre_sdk".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "sawtooth".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "sawtooth_sabre".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "scabbard".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "cylinder".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
            (
                "transact".to_string(),
                UnnamedLoggerConfig {
                    appenders: None,
                    level: Some(log::Level::Trace),
                },
            ),
        ]
        .into_iter()
        .collect::<HashMap<String, UnnamedLoggerConfig>>();
        let mut appenders = HashMap::new();
        appenders.insert("stdout".to_string(), stdout);
        partial_config = partial_config
            .with_root_logger(root_logger)
            .with_appenders(Some(appenders))
            .with_loggers(Some(loggers))
            .with_verbosity(Some(log::Level::Info));

        #[cfg(feature = "config-allow-keys")]
        {
            partial_config =
                partial_config.with_allow_keys_file(Some(String::from(ALLOW_KEYS_FILE)))
        }

        Ok(partial_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::time::Duration;

    /// Asserts config values based on the default values.
    fn assert_default_values(config: PartialConfig) {
        assert_eq!(config.tls_cert_dir(), Some(String::from(TLS_CERT_DIR)));
        assert_eq!(config.tls_ca_file(), Some(String::from(TLS_CA_FILE)));
        assert_eq!(
            config.tls_client_cert(),
            Some(String::from(TLS_CLIENT_CERT))
        );
        assert_eq!(config.tls_client_key(), Some(String::from(TLS_CLIENT_KEY)));
        assert_eq!(
            config.tls_server_cert(),
            Some(String::from(TLS_SERVER_CERT))
        );
        assert_eq!(config.tls_server_key(), Some(String::from(TLS_SERVER_KEY)));
        #[cfg(feature = "https-bind")]
        {
            assert_eq!(
                config.tls_rest_api_cert(),
                Some(String::from(TLS_REST_API_CERT))
            );
            assert_eq!(
                config.tls_rest_api_key(),
                Some(String::from(TLS_REST_API_KEY))
            );
        }
        #[cfg(feature = "service-endpoint")]
        assert_eq!(
            config.service_endpoint(),
            Some(String::from(SERVICE_ENDPOINT))
        );
        assert_eq!(
            config.network_endpoints(),
            Some(vec![String::from(NETWORK_ENDPOINT)])
        );
        assert_eq!(config.peers(), Some(vec![]));
        assert_eq!(config.node_id(), None);
        assert_eq!(config.display_name(), None);
        assert_eq!(
            config.rest_api_endpoint(),
            Some(String::from(REST_API_ENDPOINT))
        );
        assert_eq!(config.database(), Some(String::from(DATABASE)));
        assert_eq!(config.registries(), Some(vec![]));
        assert_eq!(config.registry_auto_refresh(), Some(REGISTRY_AUTO_REFRESH));
        assert_eq!(
            config.registry_forced_refresh(),
            Some(REGISTRY_FORCED_REFRESH)
        );
        assert_eq!(config.heartbeat(), Some(HEARTBEAT));
        assert_eq!(
            config.admin_timeout(),
            Some(Duration::from_secs(ADMIN_TIMEOUT))
        );
        assert_eq!(config.state_dir(), Some(String::from(STATE_DIR)));
        assert_eq!(config.tls_insecure(), Some(false));
        assert_eq!(config.no_tls(), Some(false));
        // Assert the source is correctly identified for this `PartialConfig` object.
        assert_eq!(config.source(), ConfigSource::Default);
    }

    #[test]
    /// This test verifies that a `PartialConfig` object is accurately constructed by using the
    /// `build` method implemented by the `DefaultPartialConfigBuilder` module. The following steps
    /// are performed:
    ///
    /// 1. An empty `DefaultPartialConfigBuilder` object is constructed, which implements the
    ///    `PartialConfigBuilder` trait.
    /// 2. A `PartialConfig` object is created by calling the `build` method of the
    ///    `DefaultPartialConfigBuilder` object.
    ///
    /// This test then verifies the `PartialConfig` object built from the
    /// `DefaultPartialConfigBuilder` has the correct values by asserting each expected value.
    fn test_default_builder() {
        // Create a new DefaultPartialConfigBuilder object, which implements the
        // PartialConfigBuilder trait.
        let default_config = DefaultPartialConfigBuilder::new();
        // Create a `PartialConfig` object using the `build` method.
        let partial_config = default_config
            .build()
            .expect("Unable to build DefaultPartialConfigBuilder");
        // Compare the generated `PartialConfig` object against the expected values.
        assert_default_values(partial_config);
    }
}
