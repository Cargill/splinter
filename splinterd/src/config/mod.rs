// Copyright 2018 Cargill Incorporated
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

#[cfg(feature = "config-builder")]
mod builder;
mod error;
#[cfg(feature = "config-toml")]
mod toml;

#[cfg(feature = "config-toml")]
pub use crate::config::toml::TomlConfig;
#[cfg(feature = "config-builder")]
pub use builder::ConfigBuilder;
pub use error::ConfigError;

#[cfg(not(feature = "config-toml"))]
use std::fs::File;
#[cfg(not(feature = "config-toml"))]
use std::io::Read;

#[cfg(not(feature = "config-toml"))]
use serde_derive::Deserialize;
#[cfg(not(feature = "config-toml"))]
use toml;

#[derive(Deserialize, Default, Debug)]
pub struct Config {
    storage: Option<String>,
    transport: Option<String>,
    cert_dir: Option<String>,
    ca_certs: Option<String>,
    client_cert: Option<String>,
    client_key: Option<String>,
    server_cert: Option<String>,
    server_key: Option<String>,
    service_endpoint: Option<String>,
    network_endpoint: Option<String>,
    peers: Option<Vec<String>>,
    node_id: Option<String>,
    bind: Option<String>,
    #[cfg(feature = "database")]
    database: Option<String>,
    registry_backend: Option<String>,
    registry_file: Option<String>,
    heartbeat_interval: Option<u64>,
}

impl Config {
    #[cfg(not(feature = "config-toml"))]
    pub fn from_file(mut f: File) -> Result<Config, ConfigError> {
        let mut toml = String::new();
        f.read_to_string(&mut toml)?;

        toml::from_str::<Config>(&toml).map_err(ConfigError::from)
    }

    pub fn storage(&self) -> Option<String> {
        self.storage.clone()
    }

    pub fn transport(&self) -> Option<String> {
        self.transport.clone()
    }

    pub fn cert_dir(&self) -> Option<String> {
        self.cert_dir.clone()
    }

    pub fn ca_certs(&self) -> Option<String> {
        self.ca_certs.clone()
    }

    pub fn client_cert(&self) -> Option<String> {
        self.client_cert.clone()
    }

    pub fn client_key(&self) -> Option<String> {
        self.client_key.clone()
    }

    pub fn server_cert(&self) -> Option<String> {
        self.server_cert.clone()
    }

    pub fn server_key(&self) -> Option<String> {
        self.server_key.clone()
    }

    pub fn service_endpoint(&self) -> Option<String> {
        self.service_endpoint.clone()
    }

    pub fn network_endpoint(&self) -> Option<String> {
        self.network_endpoint.clone()
    }

    pub fn peers(&self) -> Option<Vec<String>> {
        self.peers.clone()
    }

    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    pub fn bind(&self) -> Option<String> {
        self.bind.clone()
    }

    #[cfg(feature = "database")]
    pub fn database(&self) -> Option<String> {
        self.database.clone()
    }

    pub fn registry_backend(&self) -> Option<String> {
        self.registry_backend.clone()
    }

    pub fn registry_file(&self) -> Option<String> {
        self.registry_file.clone()
    }

    pub fn heartbeat_interval(&self) -> Option<u64> {
        self.heartbeat_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    /// Paths to existing example config toml files from the top-level Splinterd directory.
    static TEST_TOML1: &str = "sample_configs/splinterd.toml.example";
    // This toml file is only used in testing the TomlConfig module and therefore only used with
    // the `toml-config` feature.
    #[cfg(feature = "config-toml")]
    static TEST_TOML2: &str = "sample_configs/splinterd.toml.example2";

    /// Values present in the existing example config toml files.
    static STORAGE: &str = "yaml";
    static TRANSPORT: &str = "tls";
    static CA_CERTS: &str = "certs/ca.pem";
    static CLIENT_CERT: &str = "certs/client.crt";
    static CLIENT_KEY: &str = "certs/client.key";
    static SERVER_CERT: &str = "certs/server.crt";
    static SERVER_KEY: &str = "certs/server.key";

    /// Config values unique to the TEST_TOML1 file.
    static SERVICE_ENDPOINT: &str = "127.0.0.1:8043";
    static NETWORK_ENDPOINT: &str = "127.0.0.1:8044";
    static NODE_ID: &str = "012";

    /// Config values unique to the TEST_TOML2 file.
    // These values are only used in testing the TomlConfig module and therefore only used with
    // the `toml-config` feature.
    #[cfg(feature = "config-toml")]
    static SERVICE_ENDPOINT2: &str = "127.0.0.1:8045";
    #[cfg(feature = "config-toml")]
    static NETWORK_ENDPOINT2: &str = "127.0.0.1:8046";
    #[cfg(feature = "config-toml")]
    static NODE_ID2: &str = "345";

    /// Creates a Config struct based on the values from the TEST_TOML1 file.
    fn construct_config_example() -> Config {
        Config {
            storage: Some(STORAGE.to_string()),
            transport: Some(TRANSPORT.to_string()),
            cert_dir: None,
            ca_certs: Some(CA_CERTS.to_string()),
            client_cert: Some(CLIENT_CERT.to_string()),
            client_key: Some(CLIENT_KEY.to_string()),
            server_cert: Some(SERVER_CERT.to_string()),
            server_key: Some(SERVER_KEY.to_string()),
            service_endpoint: Some(SERVICE_ENDPOINT.to_string()),
            network_endpoint: Some(NETWORK_ENDPOINT.to_string()),
            peers: Some(vec![]),
            node_id: Some(NODE_ID.to_string()),
            bind: None,
            #[cfg(feature = "database")]
            database: None,
            registry_backend: None,
            registry_file: None,
            heartbeat_interval: None,
        }
    }

    /// Creates a Config struct based on the values from the TEST_TOML2 file.
    // This function is only used in testing the TomlConfig module and therefore only used with
    // the `toml-config` feature.
    #[cfg(feature = "config-toml")]
    fn construct_config_example2() -> Config {
        Config {
            storage: Some(STORAGE.to_string()),
            transport: Some(TRANSPORT.to_string()),
            cert_dir: None,
            ca_certs: Some(CA_CERTS.to_string()),
            client_cert: Some(CLIENT_CERT.to_string()),
            client_key: Some(CLIENT_KEY.to_string()),
            server_cert: Some(SERVER_CERT.to_string()),
            server_key: Some(SERVER_KEY.to_string()),
            service_endpoint: Some(SERVICE_ENDPOINT2.to_string()),
            network_endpoint: Some(NETWORK_ENDPOINT2.to_string()),
            peers: Some(vec![NETWORK_ENDPOINT.to_string()]),
            node_id: Some(NODE_ID2.to_string()),
            bind: None,
            #[cfg(feature = "database")]
            database: None,
            registry_backend: None,
            registry_file: None,
            heartbeat_interval: None,
        }
    }

    /// Directly compares two Config structs.
    fn compare_configs(config1: Config, config2: Config) {
        assert_eq!(config1.storage, config2.storage);
        assert_eq!(config1.transport, config2.transport);
        assert_eq!(config1.cert_dir, config2.cert_dir);
        assert_eq!(config1.ca_certs, config2.ca_certs);
        assert_eq!(config1.client_cert, config2.client_cert);
        assert_eq!(config1.client_key, config2.client_key);
        assert_eq!(config1.server_cert, config2.server_cert);
        assert_eq!(config1.server_key, config2.server_key);
        assert_eq!(config1.service_endpoint, config2.service_endpoint);
        assert_eq!(config1.network_endpoint, config2.network_endpoint);
        assert_eq!(config1.peers, config2.peers);
        assert_eq!(config1.node_id, config2.node_id);
        assert_eq!(config1.bind, config2.bind);
        #[cfg(feature = "database")]
        assert_eq!(config1.database, config2.database);
        assert_eq!(config1.registry_backend, config2.registry_backend);
        assert_eq!(config1.registry_file, config2.registry_file);
        assert_eq!(config1.heartbeat_interval, config2.heartbeat_interval);
    }

    #[cfg(not(feature = "config-toml"))]
    #[test]
    /// This test verifies that a Config object, constructed from the TEST_TOML1 file using the
    /// Config module's `from_file` method, contains the correct values using the following
    /// steps:
    ///
    /// 1. The example config toml file, TEST_TOML1, is opened.
    /// 2. A Config object is created by passing the opened file into the `from_file` function
    ///    defined in the Config module.
    /// 3. Construct a Config object with the values manually set to the values from the TEST_TOML1
    ///    file using the `construct_config_example.` This object will be used to verify the values
    ///    in the Config object created in step 2.
    ///
    /// This test then verifies the Config object generated from the `from_file` method in step 2
    /// contains the correct values by comparing it against the example Config object using the
    /// `compare_configs` function.
    fn test_config_from_file() {
        // Opening the toml file using the TEST_TOML1 path
        let config_file =
            fs::File::open(TEST_TOML1).expect(&format!("Unable to load {}", TEST_TOML1));
        // Use the config module's `from_file` method to construct a Config object from the
        // config_file previously opened.
        let generated_config = Config::from_file(config_file).unwrap();
        // Construct an example Config object.
        let example_config = construct_config_example();
        // Compare the generated Config object against a manually constructed example Config object.
        compare_configs(generated_config, example_config);
    }

    #[cfg(feature = "config-builder")]
    #[test]
    /// This test verifies that a Config object is accurately constructed by chaining the builder
    /// methods from a new ConfigBuilder object. The following steps are performed in this test:
    ///
    /// 1. An empty ConfigBuilder object is constructed.
    /// 2. The fields of the ConfigBuilder object are populated by chaining the builder methods.
    ///    Note: The values used are from an example Config object constructed in the
    ///    `construct_config_example` function.
    /// 3. Construct a Config object with the values manually set in the `construct_config_example.`
    ///    This object will be used to verify the values in the Config object created in step 2.
    ///
    /// This test then verifies the Config object built from chaining the builder methods in step
    /// 2 contains the correct values by comparing it against the example Config object using the
    /// `compare_configs` function.
    fn test_config_builder_chain() {
        // Create a new ConfigBuilder object.
        let config_builder = ConfigBuilder::new();
        // Populate the Config fields by chaining the ConfigBuilder methods. The final method,
        // `build` converts the ConfigBuilder object to a Config object.
        let built_config = config_builder
            .with_storage(STORAGE.to_string())
            .with_transport(TRANSPORT.to_string())
            .with_ca_certs(CA_CERTS.to_string())
            .with_client_cert(CLIENT_CERT.to_string())
            .with_client_key(CLIENT_KEY.to_string())
            .with_server_cert(SERVER_CERT.to_string())
            .with_server_key(SERVER_KEY.to_string())
            .with_service_endpoint(SERVICE_ENDPOINT.to_string())
            .with_network_endpoint(NETWORK_ENDPOINT.to_string())
            .with_peers(vec![])
            .with_node_id(NODE_ID.to_string())
            .build();
        // Construct an example Config object.
        let example_config = construct_config_example();
        // Compare the generated Config object against a manually constructed example Config object.
        compare_configs(built_config, example_config);
    }

    #[cfg(feature = "config-builder")]
    #[test]
    /// This test verifies that a Config object is accurately constructed by separately applying
    /// the builder methods to a new ConfigBuilder object. The following steps are performed in
    /// this test:
    ///
    /// 1. An empty ConfigBuilder object is constructed.
    /// 2. The fields of the ConfigBuilder object are populated by separately applying the builder
    ///    methods. Note: The values used are from an example Config object constructed in the
    ///    `construct_config_example` function.
    /// 3. Construct an example Config object with values manually set in `construct_config_example.`
    ///    This object will be used to verify the values in the Config object created in step 2.
    ///
    /// This test then verifies the Config object built from chaining the builder methods in step
    /// 2 contains the correct values by comparing it against the example Config object using the
    /// `compare_configs` function.
    fn test_config_builder_separate() {
        // Create a new ConfigBuilder object.
        let mut config_builder = ConfigBuilder::new();
        // Populate the Config fields by separately applying the ConfigBuilder methods.
        // `build` converts the ConfigBuilder object to a Config object.
        config_builder = config_builder.with_storage(STORAGE.to_string());
        config_builder = config_builder.with_transport(TRANSPORT.to_string());
        config_builder = config_builder.with_ca_certs(CA_CERTS.to_string());
        config_builder = config_builder.with_client_cert(CLIENT_CERT.to_string());
        config_builder = config_builder.with_client_key(CLIENT_KEY.to_string());
        config_builder = config_builder.with_server_cert(SERVER_CERT.to_string());
        config_builder = config_builder.with_server_key(SERVER_KEY.to_string());
        config_builder = config_builder.with_service_endpoint(SERVICE_ENDPOINT.to_string());
        config_builder = config_builder.with_network_endpoint(NETWORK_ENDPOINT.to_string());
        config_builder = config_builder.with_peers(vec![]);
        config_builder = config_builder.with_node_id(NODE_ID.to_string());
        // The `build` method converts the ConfigBuilder object to a Config object.
        let built_config = config_builder.build();
        // Construct an example Config object.
        let example_config = construct_config_example();
        // Compare the generated Config object against a manually constructed example Config object.
        compare_configs(built_config, example_config);
    }

    #[cfg(feature = "config-toml")]
    #[test]
    /// This test verifies that a Config object, constructed from the TomlConfig module, contains
    /// the correct values using the following steps:
    ///
    /// 1. An empty ConfigBuilder object is constructed.
    /// 2. The example config toml file, TEST_TOML1, is read and converted to a string.
    /// 3. A TomlConfig object is constructed by passing in the toml string created in the previous
    ///    step.
    /// 4. Apply the TomlConfig object created in the previous step to the ConfigBuilder created.
    ///    Note: The TomlConfig object's values should have precedence, so populated values in the
    ///    TomlConfig object should overwrite existing ones.
    /// 5. Construct a Config object with the values manually set to the values from the TEST_TOML1
    ///    file using the `construct_config_example.` This object will be used to verify the values
    ///    in the Config object created in step 4.
    ///
    /// This test then verifies the built Config object contains the correct values by comparing
    /// it against the example Config object using the `compare_configs` function.
    fn test_toml_builder() {
        // Create a new ConfigBuilder object, with all empty fields.
        let empty_builder = ConfigBuilder::new();
        // Read the TEST_TOML1 example file to a string.
        let toml_string =
            fs::read_to_string(TEST_TOML1).expect(&format!("Unable to load {}", TEST_TOML1));
        // Create a TomlConfig object from the toml string.
        let toml_builder = TomlConfig::new(toml_string)
            .expect(&format!("Unable to create TomlConfig from: {}", TEST_TOML1));
        // Apply the TomlConfig object to the empty builder initially created.
        let built_config = toml_builder.apply_to_builder(empty_builder).build();
        // Construct an example Config object with values from the TEST_TOML1 file.
        let example_config = construct_config_example();
        // Compare the generated Config object against a manually constructed example Config object.
        compare_configs(built_config, example_config);
    }

    #[cfg(feature = "config-toml")]
    #[test]
    /// This test verifies that a Config object, constructed from the TomlConfig module's
    /// `apply_to_builder` method overwrites the correct values from a ConfigBuilder object.
    ///
    /// 1. A ConfigBuilder is constructed using preset values.
    /// 2. An example config toml file, TEST_TOML2, is read and converted to a string. Note:
    ///    This toml file contains some values different from the values used to construct the
    ///    ConfigBuilder in step 1.
    /// 3. A TomlConfig object is constructed by passing in the toml string created in the previous
    ///    step.
    /// 4. Apply the TomlConfig object created in the previous step to the ConfigBuilder created.
    ///    Note: The TomlConfig object's values should have precedence, so populated values in the
    ///    TomlConfig object should overwrite existing ones.
    /// 5. Construct a Config object with the values manually set to the values from the TEST_TOML2
    ///    file using the `construct_config_example2.` This object will be used to verify the values
    ///    in the Config object created in step 4.
    ///
    /// This test then verifies the built Config object contains the correct values by comparing
    /// it against the example Config object using the `compare_configs` function.
    fn test_toml_builder_precedence() {
        // Create a ConfigBuilder with populated fields.
        let populated_builder = ConfigBuilder::new()
            .with_storage(STORAGE.to_string())
            .with_transport(TRANSPORT.to_string())
            .with_ca_certs(CA_CERTS.to_string())
            .with_client_cert(CLIENT_CERT.to_string())
            .with_client_key(CLIENT_KEY.to_string())
            .with_server_cert(SERVER_CERT.to_string())
            .with_server_key(SERVER_KEY.to_string())
            .with_service_endpoint(SERVICE_ENDPOINT.to_string())
            .with_network_endpoint(NETWORK_ENDPOINT.to_string())
            .with_peers(vec![])
            .with_node_id(NODE_ID.to_string());
        // Read the TEST_TOML2 example file to a string.
        let toml_string =
            fs::read_to_string(TEST_TOML2).expect(&format!("Unable to load {}", TEST_TOML2));
        // Create a TomlConfig object from the toml string.
        let toml_builder = TomlConfig::new(toml_string)
            .expect(&format!("Unable to create TomlConfig from: {}", TEST_TOML1));
        // Apply the TomlConfig object to the builder initially created.
        let built_config = toml_builder.apply_to_builder(populated_builder).build();
        // Construct an example Config object with values from the TEST_TOML2 file.
        let example_config = construct_config_example2();
        // Compare the generated Config object against a manually constructed example Config object.
        compare_configs(built_config, example_config);
    }
}
