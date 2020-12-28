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

//! An intermediate representation of the configuration values, used to take the
//! configuration values from different sources into a common representation.

use std::time::Duration;

/// `ConfigSource` displays the source of configuration values, used to identify which of the various
/// config modules were used to create a particular `PartialConfig` object.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum ConfigSource {
    Toml { file: String },
    Default,
    Environment,
    CommandLine,
}

/// `PartialConfig` is an intermediate representation of configuration values, used when combining
/// several sources. As such, all values of the `PartialConfig` are options as it is not necessary
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
    #[cfg(any(feature = "biome-credentials", feature = "biome-key-management"))]
    enable_biome: Option<bool>,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "auth")]
    oauth_provider: Option<String>,
    #[cfg(feature = "auth")]
    oauth_client_id: Option<String>,
    #[cfg(feature = "auth")]
    oauth_client_secret: Option<String>,
    #[cfg(feature = "auth")]
    oauth_redirect_url: Option<String>,
    #[cfg(feature = "auth")]
    oauth_openid_url: Option<String>,
    strict_ref_counts: Option<bool>,
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
            #[cfg(feature = "https-bind")]
            tls_rest_api_cert: None,
            #[cfg(feature = "https-bind")]
            tls_rest_api_key: None,
            #[cfg(feature = "service-endpoint")]
            service_endpoint: None,
            network_endpoints: None,
            advertised_endpoints: None,
            peers: None,
            node_id: None,
            display_name: None,
            rest_api_endpoint: None,
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
            #[cfg(any(feature = "biome-credentials", feature = "biome-key-management"))]
            enable_biome: None,
            #[cfg(feature = "rest-api-cors")]
            whitelist: None,
            #[cfg(feature = "auth")]
            oauth_provider: None,
            #[cfg(feature = "auth")]
            oauth_client_id: None,
            #[cfg(feature = "auth")]
            oauth_client_secret: None,
            #[cfg(feature = "auth")]
            oauth_redirect_url: None,
            #[cfg(feature = "auth")]
            oauth_openid_url: None,
            strict_ref_counts: None,
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

    #[cfg(feature = "https-bind")]
    pub fn tls_rest_api_cert(&self) -> Option<String> {
        self.tls_rest_api_cert.clone()
    }

    #[cfg(feature = "https-bind")]
    pub fn tls_rest_api_key(&self) -> Option<String> {
        self.tls_rest_api_key.clone()
    }

    #[cfg(feature = "service-endpoint")]
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

    pub fn rest_api_endpoint(&self) -> Option<String> {
        self.rest_api_endpoint.clone()
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

    #[cfg(any(feature = "biome-credentials", feature = "biome-key-management"))]
    pub fn enable_biome(&self) -> Option<bool> {
        self.enable_biome
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn whitelist(&self) -> Option<Vec<String>> {
        self.whitelist.clone()
    }

    #[cfg(feature = "auth")]
    pub fn oauth_provider(&self) -> Option<String> {
        self.oauth_provider.clone()
    }

    #[cfg(feature = "auth")]
    pub fn oauth_client_id(&self) -> Option<String> {
        self.oauth_client_id.clone()
    }

    #[cfg(feature = "auth")]
    pub fn oauth_client_secret(&self) -> Option<String> {
        self.oauth_client_secret.clone()
    }

    #[cfg(feature = "auth")]
    pub fn oauth_redirect_url(&self) -> Option<String> {
        self.oauth_redirect_url.clone()
    }

    #[cfg(feature = "auth")]
    pub fn oauth_openid_url(&self) -> Option<String> {
        self.oauth_openid_url.clone()
    }

    pub fn strict_ref_counts(&self) -> Option<bool> {
        self.strict_ref_counts
    }

    /// Adds a `config_dir` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `config_dir` - Directory containing the configuration directories and files.
    ///
    pub fn with_config_dir(mut self, config_dir: Option<String>) -> Self {
        self.config_dir = config_dir;
        self
    }

    /// Adds a `storage` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `storage` - The type of storage that should be used to store circuit state.
    ///
    pub fn with_storage(mut self, storage: Option<String>) -> Self {
        self.storage = storage;
        self
    }

    /// Adds a `tls_cert_dir` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_cert_dir` - Directory containing any certificates and keys to be used.
    ///
    pub fn with_tls_cert_dir(mut self, tls_cert_dir: Option<String>) -> Self {
        self.tls_cert_dir = tls_cert_dir;
        self
    }

    /// Adds a `tls_ca_file` value to the  `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_ca_file` - List of certificate authority certificates (*.pem files).
    ///
    pub fn with_tls_ca_file(mut self, tls_ca_file: Option<String>) -> Self {
        self.tls_ca_file = tls_ca_file;
        self
    }

    /// Adds a `tls_client_cert` value to the `PartialConfig` object.
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

    /// Adds a `tls_client_key` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_client_key` - Private key used by daemon when it is acting as a client.
    ///
    pub fn with_tls_client_key(mut self, tls_client_key: Option<String>) -> Self {
        self.tls_client_key = tls_client_key;
        self
    }

    /// Adds a `tls_server_cert` value to the `PartialConfig` object.
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

    /// Adds a `tls_server_key` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_server_key` - Private key used by daemon when it is acting as a server.
    ///
    pub fn with_tls_server_key(mut self, tls_server_key: Option<String>) -> Self {
        self.tls_server_key = tls_server_key;
        self
    }

    /// Adds a `tls_rest_api_cert` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_rest_api_cert` - A certificate signed by a certificate authority. Used by the daemon
    ///                   when it is acting as a rest_api, receiving messages.
    ///
    #[cfg(feature = "https-bind")]
    pub fn with_tls_rest_api_cert(mut self, tls_rest_api_cert: Option<String>) -> Self {
        self.tls_rest_api_cert = tls_rest_api_cert;
        self
    }

    /// Adds a `tls_rest_api_key` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_rest_api_key` - Private key used by daemon when it is acting as a rest_api.
    ///
    #[cfg(feature = "https-bind")]
    pub fn with_tls_rest_api_key(mut self, tls_rest_api_key: Option<String>) -> Self {
        self.tls_rest_api_key = tls_rest_api_key;
        self
    }

    /// Adds a `service_endpoint` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `service_endpoint` - Endpoint used for service to daemon communication.
    ///
    #[cfg(feature = "service-endpoint")]
    pub fn with_service_endpoint(mut self, service_endpoint: Option<String>) -> Self {
        self.service_endpoint = service_endpoint;
        self
    }

    /// Adds a `network_endpoints` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `network_endpoints` - Endpoints used for daemon to daemon communication.
    ///
    pub fn with_network_endpoints(mut self, network_endpoints: Option<Vec<String>>) -> Self {
        self.network_endpoints = network_endpoints;
        self
    }

    /// Adds a `advertised_endpoints` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `advertised_endpoints` - Publicly visible network endpoints.
    ///
    pub fn with_advertised_endpoints(mut self, advertised_endpoints: Option<Vec<String>>) -> Self {
        self.advertised_endpoints = advertised_endpoints;
        self
    }

    /// Adds a `peers` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `peers` - A list of splinter nodes the daemon will automatically connect to on start up.
    ///
    pub fn with_peers(mut self, peers: Option<Vec<String>>) -> Self {
        self.peers = peers;
        self
    }

    /// Adds a `node_id` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Unique ID for the node.
    ///
    pub fn with_node_id(mut self, node_id: Option<String>) -> Self {
        self.node_id = node_id;
        self
    }

    /// Adds a `display_name` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the node.
    ///
    pub fn with_display_name(mut self, display_name: Option<String>) -> Self {
        self.display_name = display_name;
        self
    }

    /// Adds a `rest_api_endpoint` value to the PartialConfig object.
    ///
    /// # Arguments
    ///
    /// * `rest_api_endpoint` - Connection endpoint for REST API.
    ///
    pub fn with_rest_api_endpoint(mut self, rest_api_endpoint: Option<String>) -> Self {
        self.rest_api_endpoint = rest_api_endpoint;
        self
    }

    #[cfg(feature = "database")]
    /// Adds a `database` value to the `PartialConfig` object, when the `database`
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

    /// Adds a `registries` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `registries` - A list of read-only Splinter registries.
    ///
    pub fn with_registries(mut self, registries: Option<Vec<String>>) -> Self {
        self.registries = registries;
        self
    }

    /// Adds a `registry_auto_refresh` value to the `PartialConfig` object.
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

    /// Adds a `registry_forced_refresh` value to the `PartialConfig` object.
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

    /// Adds a `heartbeat` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `heartbeat` - How often heartbeat should be sent.
    ///
    pub fn with_heartbeat(mut self, heartbeat: Option<u64>) -> Self {
        self.heartbeat = heartbeat;
        self
    }

    /// Adds a `timeout` value to the `PartialConfig` object.
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

    /// Adds a `state_dir` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `state_dir` - The location of the storage directory when storage is YAML.
    ///
    pub fn with_state_dir(mut self, state_dir: Option<String>) -> Self {
        self.state_dir = state_dir;
        self
    }

    /// Adds a `tls_insecure` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `tls_insecure` - Accept all peer certificates, ignoring TLS verification.
    ///
    pub fn with_tls_insecure(mut self, tls_insecure: Option<bool>) -> Self {
        self.tls_insecure = tls_insecure;
        self
    }

    /// Adds a `no-tls` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `no-tls` - Do not configure TLS Transport
    ///
    pub fn with_no_tls(mut self, no_tls: Option<bool>) -> Self {
        self.no_tls = no_tls;
        self
    }

    #[cfg(any(feature = "biome-credentials", feature = "biome-key-management"))]
    /// Adds a `enable_biome` value to the `PartialConfig` object.
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
    /// Adds a `whitelist` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `whitelist` - Add whitelist to the REST API CORS configuration
    ///
    pub fn with_whitelist(mut self, whitelist: Option<Vec<String>>) -> Self {
        self.whitelist = whitelist;
        self
    }

    #[cfg(feature = "auth")]
    /// Adds an `oauth_provider` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `oauth_provider` - Add OAuth provider to the REST API OAuth configuration
    ///
    pub fn with_oauth_provider(mut self, oauth_provider: Option<String>) -> Self {
        self.oauth_provider = oauth_provider;
        self
    }

    #[cfg(feature = "auth")]
    /// Adds an `oauth_client_id` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `oauth_client_id` - Add OAuth client ID to the REST API OAuth configuration
    ///
    pub fn with_oauth_client_id(mut self, oauth_client_id: Option<String>) -> Self {
        self.oauth_client_id = oauth_client_id;
        self
    }

    #[cfg(feature = "auth")]
    /// Adds an `oauth_client_secret` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `oauth_client_secret` - Add OAuth client secret to the REST API OAuth configuration
    ///
    pub fn with_oauth_client_secret(mut self, oauth_client_secret: Option<String>) -> Self {
        self.oauth_client_secret = oauth_client_secret;
        self
    }

    #[cfg(feature = "auth")]
    /// Adds an `oauth_redirect_url` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `oauth_redirect_url` - Add OAuth redirect URL to the REST API OAuth configuration
    ///
    pub fn with_oauth_redirect_url(mut self, oauth_redirect_url: Option<String>) -> Self {
        self.oauth_redirect_url = oauth_redirect_url;
        self
    }

    #[cfg(feature = "auth")]
    /// Adds an `with_oauth_openid_url` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `with_oauth_openid_url` - Add OAuth OpenID URL to the REST API OAuth configuration
    ///
    pub fn with_oauth_openid_url(mut self, oauth_openid_url: Option<String>) -> Self {
        self.oauth_openid_url = oauth_openid_url;
        self
    }

    /// Adds a `strict_ref_counts` value to the `PartialConfig` object.
    ///
    /// # Arguments
    ///
    /// * `strict_ref_counts` - Turns on panics if peer reference counting fails
    ///
    pub fn with_strict_ref_counts(mut self, strict_ref_counts: Option<bool>) -> Self {
        self.strict_ref_counts = strict_ref_counts;
        self
    }
}
