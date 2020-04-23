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
    storage: Option<String>,
    cert_dir: Option<String>,
    ca_certs: Option<String>,
    client_cert: Option<String>,
    client_key: Option<String>,
    server_cert: Option<String>,
    server_key: Option<String>,
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
    registry_auto_refresh_interval: Option<u64>,
    registry_forced_refresh_interval: Option<u64>,
    heartbeat_interval: Option<u64>,
    admin_service_coordinator_timeout: Option<Duration>,
    state_dir: Option<String>,
    insecure: Option<bool>,
    no_tls: Option<bool>,
    #[cfg(feature = "biome")]
    biome_enabled: Option<bool>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
}

impl PartialConfig {
    #[allow(dead_code)]
    pub fn new(source: ConfigSource) -> Self {
        PartialConfig {
            source,
            storage: None,
            cert_dir: None,
            ca_certs: None,
            client_cert: None,
            client_key: None,
            server_cert: None,
            server_key: None,
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
            registry_auto_refresh_interval: None,
            registry_forced_refresh_interval: None,
            heartbeat_interval: None,
            admin_service_coordinator_timeout: None,
            state_dir: None,
            insecure: None,
            no_tls: None,
            #[cfg(feature = "biome")]
            biome_enabled: None,
            #[cfg(feature = "rest-api-cors")]
            whitelist: None,
        }
    }

    pub fn source(&self) -> ConfigSource {
        self.source.clone()
    }

    pub fn storage(&self) -> Option<String> {
        self.storage.clone()
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

    pub fn registry_auto_refresh_interval(&self) -> Option<u64> {
        self.registry_auto_refresh_interval
    }

    pub fn registry_forced_refresh_interval(&self) -> Option<u64> {
        self.registry_forced_refresh_interval
    }

    pub fn heartbeat_interval(&self) -> Option<u64> {
        self.heartbeat_interval
    }

    pub fn admin_service_coordinator_timeout(&self) -> Option<Duration> {
        self.admin_service_coordinator_timeout
    }

    pub fn state_dir(&self) -> Option<String> {
        self.state_dir.clone()
    }

    pub fn insecure(&self) -> Option<bool> {
        self.insecure
    }

    pub fn no_tls(&self) -> Option<bool> {
        self.no_tls
    }

    #[cfg(feature = "biome")]
    pub fn biome_enabled(&self) -> Option<bool> {
        self.biome_enabled
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn whitelist(&self) -> Option<Vec<String>> {
        self.whitelist.clone()
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
    /// Adds a `cert_dir` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `cert_dir` - Directory containing any certificates and keys to be used.
    ///
    pub fn with_cert_dir(mut self, cert_dir: Option<String>) -> Self {
        self.cert_dir = cert_dir;
        self
    }

    #[allow(dead_code)]
    /// Adds a `ca_certs` value to the  PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `ca_certs` - List of certificate authority certificates (*.pem files).
    ///
    pub fn with_ca_certs(mut self, ca_certs: Option<String>) -> Self {
        self.ca_certs = ca_certs;
        self
    }

    #[allow(dead_code)]
    /// Adds a `client_cert` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `client_cert` - A certificate signed by a certificate authority. Used by the daemon when
    ///                   it is acting as a client, sending messages.
    ///
    pub fn with_client_cert(mut self, client_cert: Option<String>) -> Self {
        self.client_cert = client_cert;
        self
    }

    #[allow(dead_code)]
    /// Adds a `client_key` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `client_key` - Private key used by daemon when it is acting as a client.
    ///
    pub fn with_client_key(mut self, client_key: Option<String>) -> Self {
        self.client_key = client_key;
        self
    }

    #[allow(dead_code)]
    /// Adds a `server_cert` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `server_cert` - A certificate signed by a certificate authority. Used by the daemon when
    ///                   it is acting as a server, receiving messages.
    ///
    pub fn with_server_cert(mut self, server_cert: Option<String>) -> Self {
        self.server_cert = server_cert;
        self
    }

    #[allow(dead_code)]
    /// Adds a `server_key` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `server_key` - Private key used by daemon when it is acting as a server.
    ///
    pub fn with_server_key(mut self, server_key: Option<String>) -> Self {
        self.server_key = server_key;
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

    /// Adds a `registry_auto_refresh_interval` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `registry_auto_refresh_interval` - How often remote registries should be refreshed in the
    ///   background.
    ///
    pub fn with_registry_auto_refresh_interval(
        mut self,
        registry_auto_refresh_interval: Option<u64>,
    ) -> Self {
        self.registry_auto_refresh_interval = registry_auto_refresh_interval;
        self
    }

    /// Adds a `registry_forced_refresh_interval` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `registry_forced_refresh_interval` - How long before remote registries should be
    ///   refreshed on read.
    ///
    pub fn with_registry_forced_refresh_interval(
        mut self,
        registry_forced_refresh_interval: Option<u64>,
    ) -> Self {
        self.registry_forced_refresh_interval = registry_forced_refresh_interval;
        self
    }

    #[allow(dead_code)]
    /// Adds a `heartbeat_interval` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `heartbeat_interval` - How often heartbeat should be sent.
    ///
    pub fn with_heartbeat_interval(mut self, heartbeat_interval: Option<u64>) -> Self {
        self.heartbeat_interval = heartbeat_interval;
        self
    }

    #[allow(dead_code)]
    /// Adds a `timeout` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The coordinator timeout for admin service proposals (in milliseconds).
    ///
    pub fn with_admin_service_coordinator_timeout(mut self, timeout: Option<u64>) -> Self {
        let duration: Option<Duration> = match timeout {
            Some(t) => Some(Duration::from_millis(t)),
            _ => None,
        };
        self.admin_service_coordinator_timeout = duration;
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
    /// Adds a `insecure` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `insecure` - Accept all peer certificates, ignoring TLS verification.
    ///
    pub fn with_insecure(mut self, insecure: Option<bool>) -> Self {
        self.insecure = insecure;
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
    /// Adds a `biome_enabled` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `biome_enabled` - Enable biome REST API routes
    ///
    pub fn with_biome_enabled(mut self, biome_enabled: Option<bool>) -> Self {
        self.biome_enabled = biome_enabled;
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
