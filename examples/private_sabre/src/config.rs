// Copyright 2019 Cargill Incorporated
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

use super::error::ConfigurationError;

#[derive(Debug)]
pub struct ConfigBuilder {
    service_id: Option<String>,
    circuit: Option<String>,
    verifiers: Option<Vec<String>>,
    bind: Option<String>,
    connect: Option<String>,
    transport: TransportConfigBuilder,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            service_id: None,
            circuit: None,
            verifiers: None,
            bind: Some("localhost:8000".into()),
            connect: Some("localhost:8043".into()),
            transport: TransportConfigBuilder::default(),
        }
    }
}

impl ConfigBuilder {
    pub fn with_cli_args(&mut self, matches: &clap::ArgMatches<'_>) -> Self {
        Self {
            service_id: matches
                .value_of("service_id")
                .map(ToOwned::to_owned)
                .or_else(|| self.service_id.take()),

            circuit: matches
                .value_of("circuit")
                .map(ToOwned::to_owned)
                .or_else(|| self.circuit.take()),

            verifiers: matches
                .values_of("verifier")
                .map(|verifiers| verifiers.map(ToOwned::to_owned).collect())
                .or_else(|| self.verifiers.take()),

            bind: matches
                .value_of("bind")
                .map(ToOwned::to_owned)
                .or_else(|| self.bind.take()),

            connect: matches
                .value_of("connect")
                .map(ToOwned::to_owned)
                .or_else(|| self.connect.take()),

            transport: self.transport.with_cli_args(matches),
        }
    }

    pub fn build(mut self) -> Result<Config, ConfigurationError> {
        let (bind_host, bind_port) = self
            .bind
            .take()
            .ok_or_else(|| ConfigurationError::MissingValue("bind".into()))
            .and_then(|bind_string| split_endpoint("bind", bind_string))?;

        Ok(Config {
            service_id: self
                .service_id
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("service_id".into()))?,
            circuit: self
                .circuit
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("circuit".into()))?,
            verifiers: self
                .verifiers
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("verifiers".into()))
                .and_then(|verifiers| {
                    if verifiers.is_empty() {
                        Err(ConfigurationError::EmptyValue("verifiers".into()))
                    } else {
                        Ok(verifiers)
                    }
                })?,
            bind_host,
            bind_port,
            connect: self
                .connect
                .take()
                .ok_or_else(|| ConfigurationError::MissingValue("connect".into()))?,
            transport: self.transport.build()?,
        })
    }
}

fn split_endpoint<S: AsRef<str>>(
    field_name: &str,
    s: S,
) -> Result<(String, u16), ConfigurationError> {
    let s = s.as_ref();
    if s.is_empty() {
        return Err(ConfigurationError::EmptyValue(field_name.into()));
    }

    let mut parts = s.split(":");

    let address = parts.next().unwrap();

    let port = if let Some(port_str) = parts.next() {
        match port_str.parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => {
                return Err(ConfigurationError::InvalidValue {
                    config_field_name: field_name.into(),
                    message: "port must be an integer in the range 0 < port < 65535".into(),
                })
            }
        }
    } else {
        return Err(ConfigurationError::InvalidValue {
            config_field_name: field_name.into(),
            message: "must specify a port".into(),
        });
    };

    Ok((address.to_string(), port))
}

#[derive(Debug)]
struct TransportConfigBuilder {
    transport_type: Option<String>,
    ca_file: Option<String>,
    client_key: Option<String>,
    client_cert: Option<String>,
}

impl Default for TransportConfigBuilder {
    fn default() -> Self {
        Self {
            transport_type: Some("raw".into()),
            ca_file: None,
            client_key: None,
            client_cert: None,
        }
    }
}

impl TransportConfigBuilder {
    pub fn with_cli_args(&mut self, matches: &clap::ArgMatches<'_>) -> Self {
        Self {
            transport_type: matches
                .value_of("transport")
                .map(ToOwned::to_owned)
                .or_else(|| self.transport_type.take()),

            ca_file: matches
                .value_of("ca_file")
                .map(ToOwned::to_owned)
                .or_else(|| self.ca_file.take()),

            client_key: matches
                .value_of("client_key")
                .map(ToOwned::to_owned)
                .or_else(|| self.client_key.take()),

            client_cert: matches
                .value_of("client_cert")
                .map(ToOwned::to_owned)
                .or_else(|| self.client_cert.take()),
        }
    }

    pub fn build(mut self) -> Result<TransportConfig, ConfigurationError> {
        match self.transport_type.take() {
            Some(ref s) if s == "raw" => Ok(TransportConfig::Raw),
            Some(ref s) if s == "tls" => Ok(TransportConfig::TLS {
                ca_file: self
                    .ca_file
                    .take()
                    .ok_or_else(|| ConfigurationError::MissingValue("ca_file".into()))?,
                client_key: self
                    .client_key
                    .take()
                    .ok_or_else(|| ConfigurationError::MissingValue("client_key".into()))?,
                client_cert: self
                    .client_cert
                    .take()
                    .ok_or_else(|| ConfigurationError::MissingValue("client_cert".into()))?,
            }),
            Some(s) => Err(ConfigurationError::InvalidValue {
                config_field_name: "transport".into(),
                message: s,
            }),
            None => Err(ConfigurationError::MissingValue("transport".into())),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum TransportConfig {
    Raw,
    TLS {
        ca_file: String,
        client_key: String,
        client_cert: String,
    },
}

#[derive(Debug, PartialEq)]
pub struct Config {
    service_id: String,
    circuit: String,
    verifiers: Vec<String>,
    bind_host: String,
    bind_port: u16,
    connect: String,
    transport: TransportConfig,
}

impl Config {
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    pub fn circuit(&self) -> &str {
        &self.circuit
    }

    pub fn verifiers(&self) -> &[String] {
        &self.verifiers
    }

    pub fn bind_host(&self) -> &str {
        &self.bind_host
    }

    pub fn bind_port(&self) -> u16 {
        self.bind_port
    }

    pub fn connect(&self) -> &str {
        &self.connect
    }

    pub fn transport_config(&self) -> &TransportConfig {
        &self.transport
    }
}

#[cfg(test)]
mod test {
    use clap::{App, Arg};

    use super::*;

    /// Test that the minimum set of require values is needed, the rest of the defaults are set.
    #[test]
    fn minimal_defaults() {
        let matches = App::new("testapp")
            .arg(Arg::with_name("service_id").short("n").takes_value(true))
            .arg(Arg::with_name("circuit").short("c").takes_value(true))
            .arg(Arg::with_name("connect").short("C").takes_value(true))
            .arg(Arg::with_name("bind").short("b").takes_value(true))
            .arg(
                Arg::with_name("transport")
                    .long("transport")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("verifier")
                    .short("V")
                    .takes_value(true)
                    .multiple(true),
            )
            .get_matches_from(vec![
                "testapp",
                "-n",
                "my_service",
                "-c",
                "my_circuit",
                "-V",
                "v1",
            ]);

        let config = ConfigBuilder::default()
            .with_cli_args(&matches)
            .build()
            .expect("Unable to configure app");

        assert_eq!("my_service", config.service_id());
        assert_eq!("my_circuit", config.circuit());
        assert_eq!(&["v1".to_owned()], config.verifiers());
        assert_eq!("localhost:8043", config.connect());
        assert_eq!("localhost", config.bind_host());
        assert_eq!(8000, config.bind_port());

        assert_eq!(&TransportConfig::Raw, config.transport_config());
    }

    /// Test all settings for a raw transport
    #[test]
    fn config_raw_tranport() {
        let matches = App::new("testapp")
            .arg(Arg::with_name("service_id").short("n").takes_value(true))
            .arg(Arg::with_name("circuit").short("c").takes_value(true))
            .arg(Arg::with_name("connect").short("C").takes_value(true))
            .arg(Arg::with_name("bind").short("b").takes_value(true))
            .arg(
                Arg::with_name("transport")
                    .long("transport")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("verifier")
                    .short("V")
                    .takes_value(true)
                    .multiple(true),
            )
            .get_matches_from(vec![
                "testapp",
                "-n",
                "my_service",
                "-c",
                "my_circuit",
                "-C",
                "splinterd:8053",
                "-b",
                "eth0:8080",
                "-V",
                "v1",
                "-V",
                "v2",
                "--transport",
                "raw",
            ]);

        let config = ConfigBuilder::default()
            .with_cli_args(&matches)
            .build()
            .expect("Unable to configure app");

        assert_eq!("my_service", config.service_id());
        assert_eq!("my_circuit", config.circuit());
        assert_eq!(&["v1".to_owned(), "v2".to_owned()], config.verifiers());
        assert_eq!("splinterd:8053", config.connect());
        assert_eq!("eth0", config.bind_host());
        assert_eq!(8080, config.bind_port());

        assert_eq!(&TransportConfig::Raw, config.transport_config());
    }

    #[test]
    fn config_tls_transport() {
        let matches = App::new("testapp")
            .arg(Arg::with_name("service_id").short("n").takes_value(true))
            .arg(Arg::with_name("circuit").short("c").takes_value(true))
            .arg(Arg::with_name("connect").short("C").takes_value(true))
            .arg(Arg::with_name("bind").short("b").takes_value(true))
            .arg(
                Arg::with_name("transport")
                    .long("transport")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("verifier")
                    .short("V")
                    .takes_value(true)
                    .multiple(true),
            )
            .arg(Arg::with_name("ca_file").long("ca-file").takes_value(true))
            .arg(
                Arg::with_name("client_key")
                    .long("client-key")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("client_cert")
                    .long("client-cert")
                    .takes_value(true),
            )
            .get_matches_from(vec![
                "testapp",
                "-n",
                "my_service",
                "-c",
                "my_circuit",
                "-V",
                "v1",
                "--transport",
                "tls",
                "--ca-file",
                "./some_ca_file",
                "--client-cert",
                "./some_client.cert",
                "--client-key",
                "./some_client.key",
            ]);

        let config = ConfigBuilder::default()
            .with_cli_args(&matches)
            .build()
            .expect("Unable to configure app");

        assert_eq!("my_service", config.service_id());
        assert_eq!("my_circuit", config.circuit());
        assert_eq!(&["v1".to_owned()], config.verifiers());
        assert_eq!("localhost:8043", config.connect());
        assert_eq!("localhost", config.bind_host());
        assert_eq!(8000, config.bind_port());

        assert_eq!(
            &TransportConfig::TLS {
                ca_file: "./some_ca_file".into(),
                client_key: "./some_client.key".into(),
                client_cert: "./some_client.cert".into()
            },
            config.transport_config()
        );
    }

    /// Test that the builder fails on a missing service id
    #[test]
    fn missing_field_service_id() {
        let matches = App::new("testapp")
            .arg(Arg::with_name("service_id").short("n").takes_value(true))
            .arg(Arg::with_name("circuit").short("c").takes_value(true))
            .arg(Arg::with_name("connect").short("C").takes_value(true))
            .arg(Arg::with_name("bind").short("b").takes_value(true))
            .arg(
                Arg::with_name("transport")
                    .long("transport")
                    .takes_value(true),
            )
            .arg(
                Arg::with_name("verifier")
                    .short("V")
                    .takes_value(true)
                    .multiple(true),
            )
            .get_matches_from(vec![
                "testapp",
                "-c",
                "my_circuit",
                "-C",
                "splinterd:8053",
                "-b",
                "eth0:8080",
                "-V",
                "v1",
                "-V",
                "v2",
                "--transport",
                "raw",
            ]);

        assert_eq!(
            Err(ConfigurationError::MissingValue("service_id".into())),
            ConfigBuilder::default().with_cli_args(&matches).build()
        );
    }
}
