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

//! `PartialConfig` builder using values from splinterd command line arguments.

use crate::config::{ConfigError, ConfigSource, PartialConfig, PartialConfigBuilder};
use clap::{ArgMatches, ErrorKind};

#[cfg(feature = "scabbard-database-support")]
use crate::config::scabbard_state::ScabbardState;

/// `PartialConfig` builder which holds command line arguments, represented as clap `ArgMatches`.
pub struct ClapPartialConfigBuilder<'a> {
    matches: ArgMatches<'a>,
}

// Parses a u64 value from a clap argument.
fn parse_value(matches: &ArgMatches, arg: &str) -> Result<Option<u64>, ConfigError> {
    match value_t!(matches.value_of(arg), u64) {
        Ok(v) => Ok(Some(v)),
        Err(e) => match e.kind {
            ErrorKind::ValueValidation => Err(ConfigError::InvalidArgument(e.to_string())),
            _ => Ok(None),
        },
    }
}

impl<'a> ClapPartialConfigBuilder<'a> {
    pub fn new(matches: ArgMatches<'a>) -> Self {
        ClapPartialConfigBuilder { matches }
    }
}

/// Implementation of the `PartialConfigBuilder` trait to create a `PartialConfig` object from the
/// command line config options.
impl<'a> PartialConfigBuilder for ClapPartialConfigBuilder<'_> {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let mut partial_config = PartialConfig::new(ConfigSource::CommandLine);

        partial_config = partial_config
            .with_config_dir(self.matches.value_of("config_dir").map(String::from))
            .with_tls_cert_dir(self.matches.value_of("tls_cert_dir").map(String::from))
            .with_tls_ca_file(self.matches.value_of("tls_ca_file").map(String::from))
            .with_tls_client_cert(self.matches.value_of("tls_client_cert").map(String::from))
            .with_tls_client_key(self.matches.value_of("tls_client_key").map(String::from))
            .with_tls_server_cert(self.matches.value_of("tls_server_cert").map(String::from))
            .with_tls_server_key(self.matches.value_of("tls_server_key").map(String::from))
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
            .with_rest_api_endpoint(self.matches.value_of("rest_api_endpoint").map(String::from))
            .with_database(self.matches.value_of("database").map(String::from))
            .with_registries(
                self.matches
                    .values_of("registries")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
            .with_registry_auto_refresh(parse_value(&self.matches, "registry_auto_refresh")?)
            .with_registry_forced_refresh(parse_value(&self.matches, "registry_forced_refresh")?)
            .with_heartbeat(parse_value(&self.matches, "heartbeat")?)
            .with_tls_insecure(if self.matches.is_present("tls_insecure") {
                Some(true)
            } else {
                None
            })
            .with_no_tls(if self.matches.is_present("no_tls") {
                Some(true)
            } else {
                None
            })
            .with_state_dir(self.matches.value_of("state_dir").map(String::from))
            .with_peering_key(self.matches.value_of("peering_key").map(String::from));

        #[cfg(feature = "https-bind")]
        {
            partial_config = partial_config
                .with_tls_rest_api_cert(
                    self.matches.value_of("tls_rest_api_cert").map(String::from),
                )
                .with_tls_rest_api_key(self.matches.value_of("tls_rest_api_key").map(String::from));
        }

        #[cfg(feature = "service-endpoint")]
        {
            partial_config = partial_config
                .with_service_endpoint(self.matches.value_of("service_endpoint").map(String::from))
        }

        #[cfg(feature = "rest-api-cors")]
        {
            partial_config = partial_config.with_whitelist(
                self.matches
                    .values_of("whitelist")
                    .map(|values| values.map(String::from).collect::<Vec<String>>()),
            )
        }

        #[cfg(feature = "biome-credentials")]
        {
            partial_config = partial_config.with_enable_biome_credentials(Some(
                self.matches.is_present("enable_biome_credentials"),
            ))
        }

        #[cfg(feature = "oauth")]
        {
            partial_config = partial_config
                .with_oauth_provider(self.matches.value_of("oauth_provider").map(String::from))
                .with_oauth_client_id(self.matches.value_of("oauth_client_id").map(String::from))
                .with_oauth_client_secret(
                    self.matches
                        .value_of("oauth_client_secret")
                        .map(String::from),
                )
                .with_oauth_redirect_url(
                    self.matches
                        .value_of("oauth_redirect_url")
                        .map(String::from),
                )
                .with_oauth_openid_url(self.matches.value_of("oauth_openid_url").map(String::from))
                .with_oauth_openid_auth_params(
                    self.matches
                        .values_of("oauth_openid_auth_params")
                        .map(|values| {
                            values
                                .map(|value| {
                                    let mut parts = value.splitn(2, '=');
                                    match (parts.next(), parts.next()) {
                                        (Some(key), Some(val)) => {
                                            Ok((key.to_owned(), val.to_owned()))
                                        }
                                        (Some(_), None) => Err(ConfigError::InvalidArgument(
                                            "OAuth OpenID auth parameters must be in the format \
                                             <key>=<value>"
                                                .to_string(),
                                        )),
                                        // splitn always returns at least one item
                                        _ => unreachable!(),
                                    }
                                })
                                .collect::<Result<_, _>>()
                        })
                        .transpose()?,
                )
                .with_oauth_openid_scopes(
                    self.matches
                        .values_of("oauth_openid_scopes")
                        .map(|values| values.map(String::from).collect()),
                )
        }

        #[cfg(feature = "tap")]
        {
            partial_config = partial_config
                .with_influx_db(self.matches.value_of("influx_db").map(String::from))
                .with_influx_url(self.matches.value_of("influx_url").map(String::from))
                .with_influx_username(self.matches.value_of("influx_username").map(String::from))
                .with_influx_password(self.matches.value_of("influx_password").map(String::from))
        }
        #[cfg(feature = "log-config")]
        {
            partial_config =
                partial_config.with_verbosity(match self.matches.occurrences_of("verbose") {
                    0 => None,
                    1 => Some(log::Level::Info),
                    2 => Some(log::Level::Debug),
                    _ => Some(log::Level::Trace),
                });
        }

        #[cfg(feature = "scabbard-database-support")]
        {
            partial_config = partial_config.with_scabbard_state(
                self.matches.value_of("scabbard_state").map(|s| match s {
                    "lmdb" => ScabbardState::Lmdb,
                    "database" => ScabbardState::Database,
                    // Clap is configured to only accept these two values.
                    _ => unreachable!(),
                }),
            );
        }

        Ok(partial_config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    use clap::ArgMatches;

    /// Example configuration values.
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
    static EXAMPLE_NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
    static EXAMPLE_ADVERTISED_ENDPOINT: &str = "localhost:8044";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";
    static EXAMPLE_STATE_DIR: &str = "/var/lib/splinter/";

    /// Asserts config values based on the example values.
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
        assert_eq!(config.tls_insecure(), Some(true));
        assert_eq!(config.no_tls(), Some(true));
        assert_eq!(config.state_dir(), Some(EXAMPLE_STATE_DIR.to_string()));
    }

    /// Creates an `ArgMatches` object to be used to construct a `ClapPartialConfigBuilder` object.
    fn create_arg_matches(args: Vec<&str>) -> ArgMatches<'static> {
        #[cfg(not(feature = "https-bind"))]
        {
            clap_app!(configtest =>
                (version: crate_version!())
                (about: "Config-Test")
                (@arg config: -c --config +takes_value)
                (@arg node_id: --("node-id") +takes_value)
                (@arg display_name: --("display-name") +takes_value)
                (@arg network_endpoints: -n --("network-endpoints") +takes_value +multiple)
                (@arg advertised_endpoints: -a --("advertised-endpoints") +takes_value +multiple)
                (@arg service_endpoint: --("service-endpoint") +takes_value)
                (@arg peers: --peers +takes_value +multiple)
                (@arg tls_ca_file: --("tls-ca-file") +takes_value)
                (@arg tls_cert_dir: --("tls-cert-dir") +takes_value)
                (@arg tls_client_cert: --("tls-client-cert") +takes_value)
                (@arg tls_server_cert: --("tls-server-cert") +takes_value)
                (@arg tls_server_key:  --("tls-server-key") +takes_value)
                (@arg tls_client_key:  --("tls-client-key") +takes_value)
                (@arg rest_api_endpoint: --("rest-api-endpoint") +takes_value)
                (@arg tls_insecure: --("tls-insecure"))
                (@arg no_tls: --("no-tls"))
                (@arg state_dir: --("state-dir") + takes_value))
            .get_matches_from(args)
        }

        #[cfg(feature = "https-bind")]
        {
            clap_app!(configtest =>
                (version: crate_version!())
                (about: "Config-Test")
                (@arg config: -c --config +takes_value)
                (@arg node_id: --("node-id") +takes_value)
                (@arg display_name: --("display-name") +takes_value)
                (@arg network_endpoints: -n --("network-endpoints") +takes_value +multiple)
                (@arg advertised_endpoints: -a --("advertised-endpoints") +takes_value +multiple)
                (@arg service_endpoint: --("service-endpoint") +takes_value)
                (@arg peers: --peers +takes_value +multiple)
                (@arg tls_ca_file: --("tls-ca-file") +takes_value)
                (@arg tls_cert_dir: --("tls-cert-dir") +takes_value)
                (@arg tls_client_cert: --("tls-client-cert") +takes_value)
                (@arg tls_client_key:  --("tls-client-key") +takes_value)
                (@arg tls_server_cert: --("tls-server-cert") +takes_value)
                (@arg tls_server_key:  --("tls-server-key") +takes_value)
                (@arg tls_rest_api_cert: --("tls-rest-api-cert") +takes_value)
                (@arg tls_rest_api_key:  --("tls-rest-api-key") +takes_value)
                (@arg rest_api_endpoint: --("rest-api-endpoint") +takes_value)
                (@arg tls_insecure: --("tls-insecure"))
                (@arg no_tls: --("no-tls"))
                (@arg state_dir: --("state-dir") + takes_value))
            .get_matches_from(args)
        }
    }

    #[test]
    /// This test verifies that a `PartialConfig` object, constructed from the
    /// `ClapPartialConfigBuilder` module, contains the correct values using the following steps:
    ///
    /// 1. An example `ArgMatches` object is created using `create_arg_matches`.
    /// 2. A `ClapPartialConfigBuilder` object is constructed by passing in the example `ArgMatches`
    ///    created in the previous step.
    /// 3. The `ClapPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    ///
    /// This test then verifies the `PartialConfig` object built from the `ClapPartialConfigBuilder`
    /// object by asserting each expected value.
    fn test_command_line_config() {
        let args = vec![
            "configtest",
            "--node-id",
            EXAMPLE_NODE_ID,
            "--display-name",
            EXAMPLE_DISPLAY_NAME,
            "--network-endpoints",
            EXAMPLE_NETWORK_ENDPOINT,
            "--advertised-endpoints",
            EXAMPLE_ADVERTISED_ENDPOINT,
            #[cfg(feature = "service-endpoint")]
            "--service-endpoint",
            #[cfg(feature = "service-endpoint")]
            EXAMPLE_SERVICE_ENDPOINT,
            "--tls-ca-file",
            EXAMPLE_CA_CERTS,
            "--tls-client-cert",
            EXAMPLE_CLIENT_CERT,
            "--tls-client-key",
            EXAMPLE_CLIENT_KEY,
            "--tls-server-cert",
            EXAMPLE_SERVER_CERT,
            "--tls-server-key",
            EXAMPLE_SERVER_KEY,
            #[cfg(feature = "https-bind")]
            "--tls-rest-api-cert",
            #[cfg(feature = "https-bind")]
            EXAMPLE_REST_API_CERT,
            #[cfg(feature = "https-bind")]
            "--tls-rest-api-key",
            #[cfg(feature = "https-bind")]
            EXAMPLE_REST_API_KEY,
            "--tls-insecure",
            "--no-tls",
            "--state-dir",
            EXAMPLE_STATE_DIR,
        ];
        // Create an example ArgMatches object to initialize the `ClapPartialConfigBuilder`.
        let matches = create_arg_matches(args);
        // Create a new `ClapPartialConfigBuilder` object from the arg matches.
        let command_config = ClapPartialConfigBuilder::new(matches);
        // Build a `PartialConfig` from the `ClapPartialConfigBuilder` object created.
        let built_config = command_config
            .build()
            .expect("Unable to build ClapPartialConfigBuilder");
        // Assert the source is correctly identified for this `PartialConfig` object.
        assert_eq!(built_config.source(), ConfigSource::CommandLine);
        // Compare the generated `PartialConfig` object against the expected values.
        assert_config_values(built_config);
    }
}
