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

use crate::config::{ConfigError, ConfigSource, PartialConfig, PartialConfigBuilder};

const STORAGE: &str = "yaml";

const CONFIG_DIR: &str = "/etc/splinter";
const TLS_CERT_DIR: &str = "/etc/splinter/certs";
const STATE_DIR: &str = "/var/lib/splinter";

const TLS_CLIENT_CERT: &str = "client.crt";
const TLS_CLIENT_KEY: &str = "private/client.key";
const TLS_SERVER_CERT: &str = "server.crt";
const TLS_SERVER_KEY: &str = "private/server.key";
const TLS_CA_FILE: &str = "ca.pem";

const BIND: &str = "127.0.0.1:8080";
const SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
const NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
#[cfg(feature = "database")]
const DATABASE: &str = "127.0.0.1:5432";

const REGISTRY_AUTO_REFRESH: u64 = 600; // 600 seconds = 10 minutes
const REGISTRY_FORCED_REFRESH: u64 = 10; // 10 seconds
const HEARTBEAT: u64 = 30; // 30 seconds
const ADMIN_TIMEOUT: u64 = 30; // 30 seconds

/// Holds the default configuration values.
pub struct DefaultPartialConfigBuilder;

impl DefaultPartialConfigBuilder {
    pub fn new() -> Self {
        DefaultPartialConfigBuilder {}
    }
}

impl PartialConfigBuilder for DefaultPartialConfigBuilder {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let mut partial_config = PartialConfig::new(ConfigSource::Default);

        partial_config = partial_config
            .with_config_dir(Some(String::from(CONFIG_DIR)))
            .with_storage(Some(String::from(STORAGE)))
            .with_tls_cert_dir(Some(String::from(TLS_CERT_DIR)))
            .with_tls_ca_file(Some(String::from(TLS_CA_FILE)))
            .with_tls_client_cert(Some(String::from(TLS_CLIENT_CERT)))
            .with_tls_client_key(Some(String::from(TLS_CLIENT_KEY)))
            .with_tls_server_cert(Some(String::from(TLS_SERVER_CERT)))
            .with_tls_server_key(Some(String::from(TLS_SERVER_KEY)))
            .with_service_endpoint(Some(String::from(SERVICE_ENDPOINT)))
            .with_network_endpoints(Some(vec![String::from(NETWORK_ENDPOINT)]))
            .with_peers(Some(vec![]))
            .with_bind(Some(String::from(BIND)))
            .with_registries(Some(vec![]))
            .with_registry_auto_refresh(Some(REGISTRY_AUTO_REFRESH))
            .with_registry_forced_refresh(Some(REGISTRY_FORCED_REFRESH))
            .with_heartbeat(Some(HEARTBEAT))
            .with_admin_timeout(Some(ADMIN_TIMEOUT))
            .with_state_dir(Some(String::from(STATE_DIR)))
            .with_tls_insecure(Some(false))
            .with_no_tls(Some(false));

        #[cfg(feature = "biome")]
        {
            partial_config = partial_config.with_enable_biome(Some(false));
        }

        #[cfg(feature = "database")]
        {
            partial_config = partial_config.with_database(Some(String::from(DATABASE)));
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
        assert_eq!(config.storage(), Some(String::from(STORAGE)));
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
        assert_eq!(config.bind(), Some(String::from(BIND)));
        #[cfg(feature = "database")]
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
        #[cfg(feature = "biome")]
        assert_eq!(config.enable_biome(), Some(false));
        // Assert the source is correctly identified for this PartialConfig object.
        assert_eq!(config.source(), ConfigSource::Default);
    }

    #[test]
    /// This test verifies that a PartialConfig object is accurately constructed by using the
    /// `build` method implemented by the DefaultPartialConfigBuilder module. The following steps
    /// are performed:
    ///
    /// 1. An empty DefaultPartialConfigBuilder object is constructed, which implements the
    ///    PartialConfigBuilder trait.
    /// 2. A PartialConfig object is created by calling the `build` method of the
    ///    DefaultPartialConfigBuilder object.
    ///
    /// This test then verifies the PartialConfig object built from the DefaulConfig object has
    /// the correct values by asserting each expected value.
    fn test_default_builder() {
        // Create a new DefaultPartialConfigBuilder object, which implements the
        // PartialConfigBuilder trait.
        let default_config = DefaultPartialConfigBuilder::new();
        // Create a PartialConfig object using the `build` method.
        let partial_config = default_config
            .build()
            .expect("Unable to build DefaultPartialConfigBuilder");
        // Compare the generated PartialConfig object against the expected values.
        assert_default_values(partial_config);
    }
}
