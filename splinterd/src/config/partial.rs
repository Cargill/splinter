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

use std::time::Duration;

/// ConfigSource displays the source of configuration values, used to identify which of the various
/// config modules were used to create a particular PartialConfig object.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ConfigSource {
    Toml { file: String },
    Default,
    Environment,
    CommandLine,
}

/// PartialConfig is an intermediate representation of configuration values, used when combining
/// several sources. As such, all values of the PartialConfig are options as it is not necessary
/// to provide all values from a single source.
#[derive(Deserialize, Debug)]
pub struct PartialConfig {
    source: ConfigSource,
    config_dir: Option<String>,
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
    bind: Option<String>,
    #[cfg(feature = "database")]
    database: Option<String>,
    registries: Option<Vec<String>>,
    registry_auto_refresh: Option<u64>,
    registry_forced_refresh: Option<u64>,
    heartbeat: Option<u64>,
    admin_timeout: Option<Duration>,
    state_dir: Option<String>,
    tls_insecure: Option<bool>,
    no_tls: Option<bool>,
    #[cfg(feature = "biome")]
    enable_biome: Option<bool>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
}

impl PartialConfig {
    #[allow(dead_code)]
    pub fn new(source: ConfigSource) -> Self {
        PartialConfig {
            source,
            config_dir: None,
            storage: None,
            tls_cert_dir: None,
            tls_ca_file: None,
            tls_client_cert: None,
            tls_client_key: None,
            tls_server_cert: None,
            tls_server_key: None,
            service_endpoint: None,
            network_endpoints: None,
            advertised_endpoints: None,
            peers: None,
            node_id: None,
            display_name: None,
            bind: None,
            #[cfg(feature = "database")]
            database: None,
            registries: None,
            registry_auto_refresh: None,
            registry_forced_refresh: None,
            heartbeat: None,
            admin_timeout: None,
            state_dir: None,
            tls_insecure: None,
            no_tls: None,
            #[cfg(feature = "biome")]
            enable_biome: None,
            #[cfg(feature = "rest-api-cors")]
            whitelist: None,
        }
    }

    pub fn source(&self) -> ConfigSource {
        self.source.clone()
    }

    pub fn config_dir(&self) -> Option<String> {
        self.config_dir.clone()
    }

    pub fn storage(&self) -> Option<String> {
        self.storage.clone()
    }

    pub fn tls_cert_dir(&self) -> Option<String> {
        self.tls_cert_dir.clone()
    }

    pub fn tls_ca_file(&self) -> Option<String> {
        self.tls_ca_file.clone()
    }

    pub fn tls_client_cert(&self) -> Option<String> {
        self.tls_client_cert.clone()
    }

    pub fn tls_client_key(&self) -> Option<String> {
        self.tls_client_key.clone()
    }

    pub fn tls_server_cert(&self) -> Option<String> {
        self.tls_server_cert.clone()
    }

    pub fn tls_server_key(&self) -> Option<String> {
        self.tls_server_key.clone()
    }

    pub fn service_endpoint(&self) -> Option<String> {
        self.service_endpoint.clone()
    }

    pub fn network_endpoints(&self) -> Option<Vec<String>> {
        self.network_endpoints.clone()
    }

    pub fn advertised_endpoints(&self) -> Option<Vec<String>> {
        self.advertised_endpoints.clone()
    }

    pub fn peers(&self) -> Option<Vec<String>> {
        self.peers.clone()
    }

    pub fn node_id(&self) -> Option<String> {
        self.node_id.clone()
    }

    pub fn display_name(&self) -> Option<String> {
        self.display_name.clone()
    }

    pub fn bind(&self) -> Option<String> {
        self.bind.clone()
    }

    #[cfg(feature = "database")]
    pub fn database(&self) -> Option<String> {
        self.database.clone()
    }

    pub fn registries(&self) -> Option<Vec<String>> {
        self.registries.clone()
    }

    pub fn registry_auto_refresh(&self) -> Option<u64> {
        self.registry_auto_refresh
    }

    pub fn registry_forced_refresh(&self) -> Option<u64> {
        self.registry_forced_refresh
    }

    pub fn heartbeat(&self) -> Option<u64> {
        self.heartbeat
    }

    pub fn admin_timeout(&self) -> Option<Duration> {
        self.admin_timeout
    }

    pub fn state_dir(&self) -> Option<String> {
        self.state_dir.clone()
    }

    pub fn tls_insecure(&self) -> Option<bool> {
        self.tls_insecure
    }

    pub fn no_tls(&self) -> Option<bool> {
        self.no_tls
    }

    #[cfg(feature = "biome")]
    pub fn enable_biome(&self) -> Option<bool> {
        self.enable_biome
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn whitelist(&self) -> Option<Vec<String>> {
        self.whitelist.clone()
    }

    /// Adds a `config_dir` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `config_dir` - Directory containing the configuration directories and files.
    ///
    pub fn with_config_dir(mut self, config_dir: Option<String>) -> Self {
        self.config_dir = config_dir;
        self
    }

    #[allow(dead_code)]
    /// Adds a `storage` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `storage` - The type of storage that should be used to store circuit state.
    ///
    pub fn with_storage(mut self, storage: Option<String>) -> Self {
        self.storage = storage;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_cert_dir` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_cert_dir` - Directory containing any certificates and keys to be used.
    ///
    pub fn with_tls_cert_dir(mut self, tls_cert_dir: Option<String>) -> Self {
        self.tls_cert_dir = tls_cert_dir;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_ca_file` value to the  PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_ca_file` - List of certificate authority certificates (*.pem files).
    ///
    pub fn with_tls_ca_file(mut self, tls_ca_file: Option<String>) -> Self {
        self.tls_ca_file = tls_ca_file;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_client_cert` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_client_cert` - A certificate signed by a certificate authority. Used by the daemon
    ///                   when it is acting as a client, sending messages.
    ///
    pub fn with_tls_client_cert(mut self, tls_client_cert: Option<String>) -> Self {
        self.tls_client_cert = tls_client_cert;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_client_key` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_client_key` - Private key used by daemon when it is acting as a client.
    ///
    pub fn with_tls_client_key(mut self, tls_client_key: Option<String>) -> Self {
        self.tls_client_key = tls_client_key;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_server_cert` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_server_cert` - A certificate signed by a certificate authority. Used by the daemon
    ///                   when it is acting as a server, receiving messages.
    ///
    pub fn with_tls_server_cert(mut self, tls_server_cert: Option<String>) -> Self {
        self.tls_server_cert = tls_server_cert;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_server_key` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_server_key` - Private key used by daemon when it is acting as a server.
    ///
    pub fn with_tls_server_key(mut self, tls_server_key: Option<String>) -> Self {
        self.tls_server_key = tls_server_key;
        self
    }

    #[allow(dead_code)]
    /// Adds a `service_endpoint` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `service_endpoint` - Endpoint used for service to daemon communication.
    ///
    pub fn with_service_endpoint(mut self, service_endpoint: Option<String>) -> Self {
        self.service_endpoint = service_endpoint;
        self
    }

    #[allow(dead_code)]
    /// Adds a `network_endpoints` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `network_endpoints` - Endpoints used for daemon to daemon communication.
    ///
    pub fn with_network_endpoints(mut self, network_endpoints: Option<Vec<String>>) -> Self {
        self.network_endpoints = network_endpoints;
        self
    }

    #[allow(dead_code)]
    /// Adds a `advertised_endpoints` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `advertised_endpoints` - Publicly visible network endpoints.
    ///
    pub fn with_advertised_endpoints(mut self, advertised_endpoints: Option<Vec<String>>) -> Self {
        self.advertised_endpoints = advertised_endpoints;
        self
    }

    #[allow(dead_code)]
    /// Adds a `peers` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `peers` - A list of splinter nodes the daemon will automatically connect to on start up.
    ///
    pub fn with_peers(mut self, peers: Option<Vec<String>>) -> Self {
        self.peers = peers;
        self
    }

    #[allow(dead_code)]
    /// Adds a `node_id` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique ID for the node.
    ///
    pub fn with_node_id(mut self, node_id: Option<String>) -> Self {
        self.node_id = node_id;
        self
    }

    #[allow(dead_code)]
    /// Adds a `display_name` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the node.
    ///
    pub fn with_display_name(mut self, display_name: Option<String>) -> Self {
        self.display_name = display_name;
        self
    }

    #[allow(dead_code)]
    /// Adds a `bind` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `bind` - Connection endpoint for REST API.
    ///
    pub fn with_bind(mut self, bind: Option<String>) -> Self {
        self.bind = bind;
        self
    }

    #[cfg(feature = "database")]
    /// Adds a `database` value to the PartialConfig object, when the `database`
    /// feature flag is used.
    ///
    /// # Arguments
    ///
    /// * `database` - Connection endpoint for a database.
    ///
    pub fn with_database(mut self, database: Option<String>) -> Self {
        self.database = database;
        self
    }

    #[allow(dead_code)]
    /// Adds a `registries` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `registries` - A list of read-only node registries.
    ///
    pub fn with_registries(mut self, registries: Option<Vec<String>>) -> Self {
        self.registries = registries;
        self
    }

    #[allow(dead_code)]
    /// Adds a `registry_auto_refresh` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `registry_auto_refresh` - How often remote registries should be refreshed in the
    ///   background.
    ///
    pub fn with_registry_auto_refresh(mut self, registry_auto_refresh: Option<u64>) -> Self {
        self.registry_auto_refresh = registry_auto_refresh;
        self
    }

    #[allow(dead_code)]
    /// Adds a `registry_forced_refresh` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `registry_forced_refresh` - How long before remote registries should be
    ///   refreshed on read.
    ///
    pub fn with_registry_forced_refresh(mut self, registry_forced_refresh: Option<u64>) -> Self {
        self.registry_forced_refresh = registry_forced_refresh;
        self
    }

    #[allow(dead_code)]
    /// Adds a `heartbeat` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `heartbeat` - How often heartbeat should be sent.
    ///
    pub fn with_heartbeat(mut self, heartbeat: Option<u64>) -> Self {
        self.heartbeat = heartbeat;
        self
    }

    #[allow(dead_code)]
    /// Adds a `timeout` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The coordinator timeout for admin service proposals (in milliseconds).
    ///
    pub fn with_admin_timeout(mut self, timeout: Option<u64>) -> Self {
        let duration: Option<Duration> = match timeout {
            Some(t) => Some(Duration::from_secs(t)),
            _ => None,
        };
        self.admin_timeout = duration;
        self
    }

    #[allow(dead_code)]
    /// Adds a `state_dir` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `state_dir` - The location of the storage directory when storage is YAML.
    ///
    pub fn with_state_dir(mut self, state_dir: Option<String>) -> Self {
        self.state_dir = state_dir;
        self
    }

    #[allow(dead_code)]
    /// Adds a `tls_insecure` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `tls_insecure` - Accept all peer certificates, ignoring TLS verification.
    ///
    pub fn with_tls_insecure(mut self, tls_insecure: Option<bool>) -> Self {
        self.tls_insecure = tls_insecure;
        self
    }

    #[allow(dead_code)]
    /// Adds a `no-tls` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `no-tls` - Do not configure TLS Transport
    ///
    pub fn with_no_tls(mut self, no_tls: Option<bool>) -> Self {
        self.no_tls = no_tls;
        self
    }

    #[cfg(feature = "biome")]
    /// Adds a `enable_biome` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `enable_biome` - Enable biome REST API routes
    ///
    pub fn with_enable_biome(mut self, enable_biome: Option<bool>) -> Self {
        self.enable_biome = enable_biome;
        self
    }

    #[cfg(feature = "rest-api-cors")]
    /// Adds a `whitelist` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `whitelist` - Add whitelist to the REST API CORS configuration
    ///
    pub fn with_whitelist(mut self, whitelist: Option<Vec<String>>) -> Self {
        self.whitelist = whitelist;
        self
    }
}
