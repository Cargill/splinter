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
use clap::{ArgMatches, ErrorKind};

/// Holds configuration values from command line arguments, represented by clap ArgMatches.
pub struct ClapPartialConfigBuilder<'a> {
    matches: ArgMatches<'a>,
}

fn parse_value(matches: &ArgMatches, arg: &str) -> Result<Option<u64>, ConfigError> {
    match value_t!(matches.value_of(arg), u64) {
        Ok(v) => Ok(Some(v)),
        Err(e) => match e.kind {
            ErrorKind::ValueValidation => Err(ConfigError::InvalidArgument(e)),
            _ => Ok(None),
        },
    }
}

impl<'a> ClapPartialConfigBuilder<'a> {
    pub fn new(matches: ArgMatches<'a>) -> Self {
        ClapPartialConfigBuilder { matches }
    }
}

impl<'a> PartialConfigBuilder for ClapPartialConfigBuilder<'_> {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let mut partial_config = PartialConfig::new(ConfigSource::CommandLine);

        partial_config = partial_config
            .with_storage(self.matches.value_of("storage").map(String::from))
            .with_cert_dir(self.matches.value_of("cert_dir").map(String::from))
            .with_ca_certs(self.matches.value_of("ca_file").map(String::from))
            .with_client_cert(self.matches.value_of("client_cert").map(String::from))
            .with_client_key(self.matches.value_of("client_key").map(String::from))
            .with_server_cert(self.matches.value_of("server_cert").map(String::from))
            .with_server_key(self.matches.value_of("server_key").map(String::from))
            .with_service_endpoint(self.matches.value_of("service_endpoint").map(String::from))
            .with_network_endpoints(
                self.matches
                    .values_of("network_endpoints")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
            .with_advertised_endpoints(
                self.matches
                    .values_of("advertised_endpoints")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
            .with_peers(
                self.matches
                    .values_of("peers")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
            .with_node_id(self.matches.value_of("node_id").map(String::from))
            .with_display_name(self.matches.value_of("display_name").map(String::from))
            .with_bind(self.matches.value_of("bind").map(String::from))
            .with_registries(
                self.matches
                    .values_of("registries")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
            .with_heartbeat_interval(parse_value(&self.matches, "heartbeat_interval")?)
            .with_insecure(if self.matches.is_present("insecure") {
                Some(true)
            } else {
                None
            })
            .with_no_tls(if self.matches.is_present("no_tls") {
                Some(true)
            } else {
                None
            });

        #[cfg(feature = "biome")]
        {
            partial_config =
                partial_config.with_biome_enabled(if self.matches.is_present("biome_enabled") {
                    Some(true)
                } else {
                    None
                });
        }

        #[cfg(feature = "database")]
        {
            partial_config =
                partial_config.with_database(self.matches.value_of("database").map(String::from))
        }

        #[cfg(feature = "registry-remote")]
        {
            partial_config = partial_config
                .with_registry_auto_refresh_interval(parse_value(
                    &self.matches,
                    "registry_auto_refresh_interval",
                )?)
                .with_registry_forced_refresh_interval(parse_value(
                    &self.matches,
                    "registry_forced_refresh_interval",
                )?)
        }

        Ok(partial_config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    use clap::ArgMatches;

    /// Example configuration values.
    static EXAMPLE_STORAGE: &str = "yaml";
    static EXAMPLE_CA_CERTS: &str = "certs/ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "certs/client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "certs/client.key";
    static EXAMPLE_SERVER_CERT: &str = "certs/server.crt";
    static EXAMPLE_SERVER_KEY: &str = "certs/server.key";
    static EXAMPLE_SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static EXAMPLE_NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
    static EXAMPLE_ADVERTISED_ENDPOINT: &str = "localhost:8044";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";

    /// Asserts config values based on the example values.
    fn assert_config_values(config: PartialConfig) {
        assert_eq!(config.storage(), Some(EXAMPLE_STORAGE.to_string()));
        assert_eq!(config.cert_dir(), None);
        assert_eq!(config.ca_certs(), Some(EXAMPLE_CA_CERTS.to_string()));
        assert_eq!(config.client_cert(), Some(EXAMPLE_CLIENT_CERT.to_string()));
        assert_eq!(config.client_key(), Some(EXAMPLE_CLIENT_KEY.to_string()));
        assert_eq!(config.server_cert(), Some(EXAMPLE_SERVER_CERT.to_string()));
        assert_eq!(config.server_key(), Some(EXAMPLE_SERVER_KEY.to_string()));
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
        assert_eq!(config.peers(), None);
        assert_eq!(config.node_id(), Some(EXAMPLE_NODE_ID.to_string()));
        assert_eq!(
            config.display_name(),
            Some(EXAMPLE_DISPLAY_NAME.to_string())
        );
        assert_eq!(config.bind(), None);
        #[cfg(feature = "database")]
        assert_eq!(config.database(), None);
        assert_eq!(config.registries(), None);
        #[cfg(feature = "registry-remote")]
        assert_eq!(config.registry_auto_refresh_interval(), None);
        #[cfg(feature = "registry-remote")]
        assert_eq!(config.registry_forced_refresh_interval(), None);
        assert_eq!(config.heartbeat_interval(), None);
        assert_eq!(config.admin_service_coordinator_timeout(), None);
        assert_eq!(config.insecure(), Some(true));
        assert_eq!(config.no_tls(), Some(true));
    }

    /// Creates an ArgMatches object to be used to construct a ClapPartialConfigBuilder object.
    fn create_arg_matches(args: Vec<&str>) -> ArgMatches<'static> {
        clap_app!(configtest =>
            (version: crate_version!())
            (about: "Config-Test")
            (@arg config: -c --config +takes_value)
            (@arg node_id: --("node-id") +takes_value)
            (@arg display_name: --("display-name") +takes_value)
            (@arg storage: --("storage") +takes_value)
            (@arg network_endpoints: -n --("network-endpoint") +takes_value +multiple)
            (@arg advertised_endpoints: -a --("advertised-endpoint") +takes_value +multiple)
            (@arg service_endpoint: --("service-endpoint") +takes_value)
            (@arg peers: --peer +takes_value +multiple)
            (@arg ca_file: --("ca-file") +takes_value)
            (@arg cert_dir: --("cert-dir") +takes_value)
            (@arg client_cert: --("client-cert") +takes_value)
            (@arg server_cert: --("server-cert") +takes_value)
            (@arg server_key:  --("server-key") +takes_value)
            (@arg client_key:  --("client-key") +takes_value)
            (@arg bind: --("bind") +takes_value)
            (@arg insecure: --("insecure"))
            (@arg no_tls: --("no-tls")))
        .get_matches_from(args)
    }

    #[test]
    /// This test verifies that a PartialConfig object, constructed from the
    /// ClapPartialConfigBuilder module, contains the correct values using the following steps:
    ///
    /// 1. An example ArgMatches object is created using `create_arg_matches`.
    /// 2. A ClapPartialConfigBuilder object is constructed by passing in the example ArgMatches
    ///    created in the previous step.
    /// 3. The ClapPartialConfigBuilder object is transformed to a PartialConfig object using the
    ///    `build`.
    ///
    /// This test then verifies the PartialConfig object built from the ClapPartialConfigBuilder
    /// object by asserting each expected value.
    fn test_command_line_config() {
        let args = vec![
            "configtest",
            "--node-id",
            EXAMPLE_NODE_ID,
            "--display-name",
            EXAMPLE_DISPLAY_NAME,
            "--storage",
            EXAMPLE_STORAGE,
            "--network-endpoint",
            EXAMPLE_NETWORK_ENDPOINT,
            "--advertised-endpoint",
            EXAMPLE_ADVERTISED_ENDPOINT,
            "--service-endpoint",
            EXAMPLE_SERVICE_ENDPOINT,
            "--ca-file",
            EXAMPLE_CA_CERTS,
            "--client-cert",
            EXAMPLE_CLIENT_CERT,
            "--client-key",
            EXAMPLE_CLIENT_KEY,
            "--server-cert",
            EXAMPLE_SERVER_CERT,
            "--server-key",
            EXAMPLE_SERVER_KEY,
            "--insecure",
            "--no-tls",
        ];
        // Create an example ArgMatches object to initialize the ClapPartialConfigBuilder.
        let matches = create_arg_matches(args);
        // Create a new CommandLine object from the arg matches.
        let command_config = ClapPartialConfigBuilder::new(matches);
        // Build a PartialConfig from the ClapPartialConfigBuilder object created.
        let built_config = command_config
            .build()
            .expect("Unable to build ClapPartialConfigBuilder");
        // Assert the source is correctly identified for this PartialConfig object.
        assert_eq!(built_config.source(), ConfigSource::CommandLine);
        // Compare the generated PartialConfig object against the expected values.
        assert_config_values(built_config);
    }
}
