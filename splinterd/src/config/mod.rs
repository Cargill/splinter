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

//! Configuration to provide the necessary values to start up the Splinter daemon.
//!
//! These values may be sourced from a toml file, command line arguments, environment variables
//! or pre-defined defaults. This module allows for configuration values from each of these
//! sources to be combined into a final `Config` object.

mod builder;
mod clap;
mod default;
mod env;
mod error;
#[cfg(feature = "log-config")]
mod logger;
mod partial;
mod toml;

use std::time::Duration;

pub use crate::config::clap::ClapPartialConfigBuilder;
pub use crate::config::default::DefaultPartialConfigBuilder;
pub use crate::config::env::EnvPartialConfigBuilder;
pub use crate::config::toml::TomlPartialConfigBuilder;
pub use builder::{ConfigBuilder, PartialConfigBuilder};
pub use error::ConfigError;
#[cfg(feature = "log-config")]
pub use logger::LogConfig;
pub use partial::{ConfigSource, PartialConfig};

/// `Config` is the final representation of configuration values. This final config object assembles
/// values from `PartialConfig` builder objects generated from various sources.
#[derive(Debug)]
pub struct Config {
    config_dir: (String, ConfigSource),
    storage: Option<(String, ConfigSource)>,
    tls_cert_dir: (String, ConfigSource),
    tls_ca_file: (String, ConfigSource),
    tls_client_cert: (String, ConfigSource),
    tls_client_key: (String, ConfigSource),
    tls_server_cert: (String, ConfigSource),
    tls_server_key: (String, ConfigSource),
    #[cfg(feature = "https-bind")]
    tls_rest_api_cert: (String, ConfigSource),
    #[cfg(feature = "https-bind")]
    tls_rest_api_key: (String, ConfigSource),
    #[cfg(feature = "service-endpoint")]
    service_endpoint: (String, ConfigSource),
    network_endpoints: (Vec<String>, ConfigSource),
    advertised_endpoints: (Vec<String>, ConfigSource),
    peers: (Vec<String>, ConfigSource),
    node_id: Option<(String, ConfigSource)>,
    display_name: Option<(String, ConfigSource)>,
    rest_api_endpoint: (String, ConfigSource),
    database: (String, ConfigSource),
    registries: (Vec<String>, ConfigSource),
    registry_auto_refresh: (u64, ConfigSource),
    registry_forced_refresh: (u64, ConfigSource),
    heartbeat: (u64, ConfigSource),
    admin_timeout: (Duration, ConfigSource),
    state_dir: (String, ConfigSource),
    tls_insecure: (bool, ConfigSource),
    no_tls: (bool, ConfigSource),
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<(Vec<String>, ConfigSource)>,
    #[cfg(feature = "biome-credentials")]
    enable_biome_credentials: (bool, ConfigSource),
    #[cfg(feature = "oauth")]
    oauth_provider: Option<(String, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_client_id: Option<(String, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_client_secret: Option<(String, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_redirect_url: Option<(String, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_openid_url: Option<(String, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_openid_auth_params: Option<(Vec<(String, String)>, ConfigSource)>,
    #[cfg(feature = "oauth")]
    oauth_openid_scopes: Option<(Vec<String>, ConfigSource)>,
    strict_ref_counts: (bool, ConfigSource),
    #[cfg(feature = "metrics")]
    metrics_db: Option<(String, ConfigSource)>,
    #[cfg(feature = "metrics")]
    metrics_url: Option<(String, ConfigSource)>,
    #[cfg(feature = "metrics")]
    metrics_username: Option<(String, ConfigSource)>,
    #[cfg(feature = "metrics")]
    metrics_password: Option<(String, ConfigSource)>,
    #[cfg(feature = "log-config")]
    log_config: Option<(LogConfig, ConfigSource)>,
    #[cfg(feature = "log-config")]
    verbosity: (log::Level, ConfigSource),
}

impl Config {
    pub fn config_dir(&self) -> &str {
        &self.config_dir.0
    }

    pub fn storage(&self) -> Option<&str> {
        if let Some((storage, _)) = &self.storage {
            Some(storage)
        } else {
            None
        }
    }

    pub fn tls_cert_dir(&self) -> &str {
        &self.tls_cert_dir.0
    }

    pub fn tls_ca_file(&self) -> &str {
        &self.tls_ca_file.0
    }

    pub fn tls_client_cert(&self) -> &str {
        &self.tls_client_cert.0
    }

    pub fn tls_client_key(&self) -> &str {
        &self.tls_client_key.0
    }

    pub fn tls_server_cert(&self) -> &str {
        &self.tls_server_cert.0
    }

    pub fn tls_server_key(&self) -> &str {
        &self.tls_server_key.0
    }

    #[cfg(feature = "https-bind")]
    pub fn tls_rest_api_cert(&self) -> &str {
        &self.tls_rest_api_cert.0
    }

    #[cfg(feature = "https-bind")]
    pub fn tls_rest_api_key(&self) -> &str {
        &self.tls_rest_api_key.0
    }

    #[cfg(feature = "service-endpoint")]
    pub fn service_endpoint(&self) -> &str {
        &self.service_endpoint.0
    }

    pub fn network_endpoints(&self) -> &[String] {
        &self.network_endpoints.0
    }

    pub fn advertised_endpoints(&self) -> &[String] {
        &self.advertised_endpoints.0
    }

    pub fn peers(&self) -> &[String] {
        &self.peers.0
    }

    pub fn node_id(&self) -> Option<&str> {
        if let Some((id, _)) = &self.node_id {
            Some(id)
        } else {
            None
        }
    }

    pub fn display_name(&self) -> Option<&str> {
        if let Some((name, _)) = &self.display_name {
            Some(name)
        } else {
            None
        }
    }

    pub fn rest_api_endpoint(&self) -> &str {
        &self.rest_api_endpoint.0
    }

    pub fn database(&self) -> &str {
        &self.database.0
    }

    pub fn registries(&self) -> &[String] {
        &self.registries.0
    }

    pub fn registry_auto_refresh(&self) -> u64 {
        self.registry_auto_refresh.0
    }

    pub fn registry_forced_refresh(&self) -> u64 {
        self.registry_forced_refresh.0
    }

    pub fn heartbeat(&self) -> u64 {
        self.heartbeat.0
    }

    pub fn admin_timeout(&self) -> Duration {
        self.admin_timeout.0
    }

    pub fn state_dir(&self) -> &str {
        &self.state_dir.0
    }

    pub fn tls_insecure(&self) -> bool {
        self.tls_insecure.0
    }

    pub fn no_tls(&self) -> bool {
        self.no_tls.0
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn whitelist(&self) -> Option<&[String]> {
        if let Some((list, _)) = &self.whitelist {
            Some(list)
        } else {
            None
        }
    }

    #[cfg(feature = "biome-credentials")]
    pub fn enable_biome_credentials(&self) -> bool {
        self.enable_biome_credentials.0
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_provider(&self) -> Option<&str> {
        if let Some((provider, _)) = &self.oauth_provider {
            Some(provider)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_client_id(&self) -> Option<&str> {
        if let Some((client_id, _)) = &self.oauth_client_id {
            Some(client_id)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_client_secret(&self) -> Option<&str> {
        if let Some((client_secret, _)) = &self.oauth_client_secret {
            Some(client_secret)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_redirect_url(&self) -> Option<&str> {
        if let Some((redirect_url, _)) = &self.oauth_redirect_url {
            Some(redirect_url)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_url(&self) -> Option<&str> {
        if let Some((openid_url, _)) = &self.oauth_openid_url {
            Some(openid_url)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_auth_params(&self) -> Option<&[(String, String)]> {
        if let Some((auth_params, _)) = &self.oauth_openid_auth_params {
            Some(auth_params)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_scopes(&self) -> Option<&[String]> {
        if let Some((scopes, _)) = &self.oauth_openid_scopes {
            Some(scopes)
        } else {
            None
        }
    }

    pub fn strict_ref_counts(&self) -> bool {
        self.strict_ref_counts.0
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_db(&self) -> Option<&str> {
        if let Some((db, _)) = &self.metrics_db {
            Some(db)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_url(&self) -> Option<&str> {
        if let Some((url, _)) = &self.metrics_url {
            Some(url)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_username(&self) -> Option<&str> {
        if let Some((username, _)) = &self.metrics_username {
            Some(username)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_password(&self) -> Option<&str> {
        if let Some((password, _)) = &self.metrics_password {
            Some(password)
        } else {
            None
        }
    }

    #[cfg(feature = "log-config")]
    pub fn log_config(&self) -> Option<&LogConfig> {
        match &self.log_config {
            Some((c, _)) => Some(c),
            _ => None,
        }
    }

    #[cfg(feature = "log-config")]
    pub fn log_config_source(&self) -> Option<&ConfigSource> {
        match &self.log_config {
            Some((_, source)) => Some(source),
            _ => None,
        }
    }

    #[cfg(feature = "log-config")]
    pub fn verbosity_source(&self) -> Option<&ConfigSource> {
        Some(&self.verbosity.1)
    }

    #[cfg(feature = "log-config")]
    pub fn verbosity(&self) -> Option<&log::Level> {
        Some(&self.verbosity.0)
    }

    pub fn config_dir_source(&self) -> &ConfigSource {
        &self.config_dir.1
    }

    fn storage_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.storage {
            Some(source)
        } else {
            None
        }
    }

    fn tls_cert_dir_source(&self) -> &ConfigSource {
        &self.tls_cert_dir.1
    }

    fn tls_ca_file_source(&self) -> &ConfigSource {
        &self.tls_ca_file.1
    }

    fn tls_client_cert_source(&self) -> &ConfigSource {
        &self.tls_client_cert.1
    }

    fn tls_client_key_source(&self) -> &ConfigSource {
        &self.tls_client_key.1
    }

    fn tls_server_cert_source(&self) -> &ConfigSource {
        &self.tls_server_cert.1
    }

    fn tls_server_key_source(&self) -> &ConfigSource {
        &self.tls_server_key.1
    }

    #[cfg(feature = "https-bind")]
    fn tls_rest_api_cert_source(&self) -> &ConfigSource {
        &self.tls_rest_api_cert.1
    }

    #[cfg(feature = "https-bind")]
    fn tls_rest_api_key_source(&self) -> &ConfigSource {
        &self.tls_rest_api_key.1
    }

    #[cfg(feature = "service-endpoint")]
    fn service_endpoint_source(&self) -> &ConfigSource {
        &self.service_endpoint.1
    }

    fn network_endpoints_source(&self) -> &ConfigSource {
        &self.network_endpoints.1
    }

    fn advertised_endpoints_source(&self) -> &ConfigSource {
        &self.advertised_endpoints.1
    }

    fn peers_source(&self) -> &ConfigSource {
        &self.peers.1
    }

    fn node_id_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.node_id {
            Some(source)
        } else {
            None
        }
    }

    fn display_name_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.display_name {
            Some(source)
        } else {
            None
        }
    }

    fn rest_api_endpoint_source(&self) -> &ConfigSource {
        &self.rest_api_endpoint.1
    }

    fn database_source(&self) -> &ConfigSource {
        &self.database.1
    }

    fn registries_source(&self) -> &ConfigSource {
        &self.registries.1
    }

    fn registry_auto_refresh_source(&self) -> &ConfigSource {
        &self.registry_auto_refresh.1
    }

    fn registry_forced_refresh_source(&self) -> &ConfigSource {
        &self.registry_forced_refresh.1
    }

    fn heartbeat_source(&self) -> &ConfigSource {
        &self.heartbeat.1
    }

    fn admin_timeout_source(&self) -> &ConfigSource {
        &self.admin_timeout.1
    }

    fn state_dir_source(&self) -> &ConfigSource {
        &self.state_dir.1
    }

    fn tls_insecure_source(&self) -> &ConfigSource {
        &self.tls_insecure.1
    }

    fn no_tls_source(&self) -> &ConfigSource {
        &self.no_tls.1
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn whitelist_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.whitelist {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "biome-credentials")]
    pub fn enable_biome_credentials_source(&self) -> &ConfigSource {
        &self.enable_biome_credentials.1
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_provider_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_provider {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_client_id_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_client_id {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_client_secret_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_client_secret {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_redirect_url_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_redirect_url {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_url_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_openid_url {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_auth_params_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_openid_auth_params {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "oauth")]
    pub fn oauth_openid_scopes_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.oauth_openid_scopes {
            Some(source)
        } else {
            None
        }
    }

    fn strict_ref_counts_source(&self) -> &ConfigSource {
        &self.strict_ref_counts.1
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_db_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.metrics_db {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_url_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.metrics_url {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_username_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.metrics_username {
            Some(source)
        } else {
            None
        }
    }

    #[cfg(feature = "metrics")]
    pub fn metrics_password_source(&self) -> Option<&ConfigSource> {
        if let Some((_, source)) = &self.metrics_password {
            Some(source)
        } else {
            None
        }
    }

    #[allow(clippy::cognitive_complexity)]
    /// Displays the configuration value along with where the value was sourced from.
    pub fn log_as_debug(&self) {
        debug!(
            "Config: config_dir: {} (source: {:?})",
            self.config_dir(),
            self.config_dir_source()
        );
        if let (Some(id), Some(source)) = (self.storage(), self.storage_source()) {
            debug!("Config: storage: {} (source: {:?})", id, source,);
        }
        debug!(
            "Config: tls_ca_file: {} (source: {:?})",
            self.tls_ca_file(),
            self.tls_ca_file_source()
        );
        debug!(
            "Config: tls_cert_dir: {} (source: {:?})",
            self.tls_cert_dir(),
            self.tls_cert_dir_source()
        );
        debug!(
            "Config: tls_client_cert: {} (source: {:?})",
            self.tls_client_cert(),
            self.tls_client_cert_source()
        );
        debug!(
            "Config: tls_client_key: {} (source: {:?})",
            self.tls_client_key(),
            self.tls_client_key_source()
        );
        debug!(
            "Config: tls_server_cert: {} (source: {:?})",
            self.tls_server_cert(),
            self.tls_server_cert_source()
        );
        debug!(
            "Config: tls_server_key: {} (source: {:?})",
            self.tls_server_key(),
            self.tls_server_key_source()
        );
        #[cfg(feature = "https-bind")]
        {
            debug!(
                "Config: tls_rest_api_cert: {} (source: {:?})",
                self.tls_rest_api_cert(),
                self.tls_rest_api_cert_source()
            );
            debug!(
                "Config: tls_rest_api_key: {} (source: {:?})",
                self.tls_rest_api_key(),
                self.tls_rest_api_key_source()
            );
        }
        #[cfg(feature = "service-endpoint")]
        debug!(
            "Config: service_endpoint: {} (source: {:?})",
            self.service_endpoint(),
            self.service_endpoint_source()
        );
        debug!(
            "Config: network_endpoints: {:?} (source: {:?})",
            self.network_endpoints(),
            self.network_endpoints_source()
        );
        debug!(
            "Config: advertised_endpoints: {:?} (source: {:?})",
            self.advertised_endpoints(),
            self.advertised_endpoints_source()
        );
        debug!(
            "Config: peers: {:?} (source: {:?})",
            self.peers(),
            self.peers_source()
        );
        if let (Some(id), Some(source)) = (self.node_id(), self.node_id_source()) {
            debug!("Config: node_id: {} (source: {:?})", id, source,);
        }
        if let (Some(name), Some(source)) = (self.display_name(), self.display_name_source()) {
            debug!("Config: display_name: {} (source: {:?})", name, source,);
        }
        debug!(
            "Config: rest_api_endpoint: {} (source: {:?})",
            self.rest_api_endpoint(),
            self.rest_api_endpoint_source()
        );
        debug!(
            "Config: registries: {:?} (source: {:?})",
            self.registries(),
            self.registries_source()
        );
        debug!(
            "Config: registry_auto_refresh: {} (source: {:?})",
            self.registry_auto_refresh(),
            self.registry_auto_refresh_source()
        );
        debug!(
            "Config: registry_forced_refresh: {} (source: {:?})",
            self.registry_forced_refresh(),
            self.registry_forced_refresh_source()
        );
        debug!(
            "Config: state_dir: {} (source: {:?})",
            self.state_dir(),
            self.state_dir_source()
        );
        debug!(
            "Config: heartbeat: {} (source: {:?})",
            self.heartbeat(),
            self.heartbeat_source()
        );
        debug!(
            "Config: admin_timeout: {:?} (source: {:?})",
            self.admin_timeout(),
            self.admin_timeout_source()
        );
        debug!(
            "database: {} (source: {:?})",
            self.database(),
            self.database_source(),
        );
        debug!(
            "Config: tls_insecure: {:?} (source: {:?})",
            self.tls_insecure(),
            self.tls_insecure_source()
        );
        debug!(
            "Config: no_tls: {:?} (source: {:?})",
            self.no_tls(),
            self.no_tls_source()
        );
        #[cfg(feature = "rest-api-cors")]
        self.log_whitelist();
        #[cfg(feature = "biome-credentials")]
        debug!(
            "Config: enable_biome_credentials: {:?} (source: {:?})",
            self.enable_biome_credentials(),
            self.enable_biome_credentials_source()
        );
        #[cfg(feature = "oauth")]
        {
            if let (Some(provider), Some(source)) =
                (self.oauth_provider(), self.oauth_provider_source())
            {
                debug!(
                    "Config: oauth_provider: {} (source: {:?})",
                    provider, source,
                );
            }
            if let (Some(client_id), Some(source)) =
                (self.oauth_client_id(), self.oauth_client_id_source())
            {
                debug!(
                    "Config: oauth_client_id: {} (source: {:?})",
                    client_id, source,
                );
            }
            if let (Some(_), Some(source)) = (
                self.oauth_client_secret(),
                self.oauth_client_secret_source(),
            ) {
                debug!(
                    "Config: oauth_client_secret: <HIDDEN> (source: {:?})",
                    source,
                );
            }
            if let (Some(redirect_url), Some(source)) =
                (self.oauth_redirect_url(), self.oauth_redirect_url_source())
            {
                debug!(
                    "Config: oauth_redirect_url: {} (source: {:?})",
                    redirect_url, source,
                );
            }
            if let (Some(openid_url), Some(source)) =
                (self.oauth_openid_url(), self.oauth_openid_url_source())
            {
                debug!(
                    "Config: oauth_openid_url: {} (source: {:?})",
                    openid_url, source,
                );
            }
            if let (Some(auth_params), Some(source)) = (
                self.oauth_openid_auth_params(),
                self.oauth_openid_auth_params_source(),
            ) {
                debug!(
                    "Config: oauth_openid_auth_params: {:?} (source: {:?})",
                    auth_params, source,
                );
            }
            if let (Some(scopes), Some(source)) = (
                self.oauth_openid_scopes(),
                self.oauth_openid_scopes_source(),
            ) {
                debug!("Config: oauth_scopes: {:?} (source: {:?})", scopes, source,);
            }
        }
        debug!(
            "Config: strict_ref_counts: {:?} (source: {:?})",
            self.strict_ref_counts(),
            self.strict_ref_counts_source()
        );
        #[cfg(feature = "metrics")]
        {
            if let (Some(db), Some(source)) = (self.metrics_db(), self.metrics_db_source()) {
                debug!("Config: metrics_db: {:?} (source: {:?})", db, source,);
            }

            if let (Some(url), Some(source)) = (self.metrics_url(), self.metrics_url_source()) {
                debug!("Config: metrics_url: {:?} (source: {:?})", url, source,);
            }

            if let (Some(username), Some(source)) =
                (self.metrics_username(), self.metrics_username_source())
            {
                debug!(
                    "Config: metrics_username: {:?} (source: {:?})",
                    username, source,
                );
            }

            if let (Some(_), Some(source)) =
                (self.metrics_password(), self.metrics_password_source())
            {
                debug!("Config: metrics_password: <HIDDEN> (source: {:?})", source,);
            }
        }
        #[cfg(feature = "log-config")]
        {
            debug!(
                "Config: log_config: {:?} (source: {:?})",
                self.log_config(),
                self.log_config_source()
            );
        }
    }

    #[cfg(feature = "rest-api-cors")]
    fn log_whitelist(&self) {
        if let (Some(list), Some(source)) = (self.whitelist(), self.whitelist_source()) {
            debug!("Config: whitelist: {:?} (source: {:?})", list, source,);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::time::Duration;

    use ::clap::ArgMatches;
    use ::toml::{map::Map, to_string, Value};

    use crate::config::{
        ClapPartialConfigBuilder, DefaultPartialConfigBuilder, EnvPartialConfigBuilder,
        TomlPartialConfigBuilder,
    };

    /// Path to example config toml file.
    static TEST_TOML: &str = "config_test.toml";

    static EXAMPLE_TLS_CERT_DIR: &str = "test/certs/";

    /// Values present in the example config TEST_TOML file.
    static EXAMPLE_STORAGE: &str = "memory";
    static EXAMPLE_CA_CERTS: &str = "ca.pem";
    static EXAMPLE_CLIENT_CERT: &str = "client.crt";
    static EXAMPLE_CLIENT_KEY: &str = "private/client.key";
    static EXAMPLE_SERVER_CERT: &str = "server.crt";
    static EXAMPLE_SERVER_KEY: &str = "private/server.key";
    #[cfg(feature = "service-endpoint")]
    static EXAMPLE_SERVICE_ENDPOINT: &str = "tcp://127.0.0.1:8043";
    static EXAMPLE_NETWORK_ENDPOINT: &str = "tcps://127.0.0.1:8044";
    static EXAMPLE_ADVERTISED_ENDPOINT: &str = "localhost:8044";
    static EXAMPLE_NODE_ID: &str = "012";
    static EXAMPLE_DISPLAY_NAME: &str = "Node 1";

    static DEFAULT_CLIENT_CERT: &str = "client.crt";
    static DEFAULT_CLIENT_KEY: &str = "private/client.key";
    static DEFAULT_SERVER_CERT: &str = "server.crt";
    static DEFAULT_SERVER_KEY: &str = "private/server.key";
    static DEFAULT_CA_CERT: &str = "ca.pem";

    /// Converts a list of tuples to a toml `Table` `Value` used to write a toml file.
    pub fn get_toml_value() -> Value {
        let values = vec![
            ("storage".to_string(), EXAMPLE_STORAGE.to_string()),
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
            #[cfg(feature = "service-endpoint")]
            (
                "service_endpoint".to_string(),
                EXAMPLE_SERVICE_ENDPOINT.to_string(),
            ),
            ("node_id".to_string(), EXAMPLE_NODE_ID.to_string()),
            ("display_name".to_string(), EXAMPLE_DISPLAY_NAME.to_string()),
            ("version".to_string(), "1".to_string()),
        ];

        let mut config_values = Map::new();
        values.iter().for_each(|v| {
            config_values.insert(v.0.clone(), Value::String(v.1.clone()));
        });
        Value::Table(config_values)
    }

    /// Creates an `ArgMatches` object to be used to construct a `ClapPartialConfigBuilder` object.
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
        (@arg tls_ca_file: --("tls-ca-file") +takes_value)
        (@arg tls_cert_dir: --("tls-cert-dir") +takes_value)
        (@arg tls_client_cert: --("tls-client-cert") +takes_value)
        (@arg tls_server_cert: --("tls-server-cert") +takes_value)
        (@arg tls_server_key:  --("tls-server-key") +takes_value)
        (@arg tls_client_key:  --("tls-client-key") +takes_value)
        (@arg rest_api_endpoint: --("rest-api-endpoint") +takes_value)
        (@arg tls_insecure: --("tls-insecure"))
        (@arg no_tls: --("no-tls")))
        .get_matches_from(args)
    }

    #[test]
    /// This test verifies that a finalized `Config` object may be constructed from just
    /// a `DefaultPartialConfigBuilder` object, in the following steps:
    ///
    /// 1. An empty `ConfigBuilder` object is created.
    /// 2. A `PartialConfig` built from a `DefaultPartialConfigBuilder` is added to the
    ///    `ConfigBuilder`.
    ///
    /// This test then verifies the final `Config` object built from the `ConfigBuilder` object has
    /// resulted in a default `Config` object, as the node_id is not required.
    fn test_default_final_config() {
        // Create a new `ConfigBuilder` object.
        let mut builder = ConfigBuilder::new();
        // Add a `PartialConfig` built from a `DefaultPartialConfigBuilder` object to the
        // `ConfigBuilder`.
        builder = builder.with_partial_config(
            DefaultPartialConfigBuilder::new()
                .build()
                .expect("Unable to build DefaultPartialConfigBuilder"),
        );
        // Build the final `Config` object.
        let final_config = builder.build();
        // Asserts the final `Config` was successfully built.
        assert!(final_config.is_ok());
    }

    #[test]
    /// This test verifies that a finalized `Config` object constructed from just
    /// a `TomlPartialConfigBuilder` object will be unsuccessful because of the missing values, in
    /// the following steps:
    ///
    /// 1. An empty `ConfigBuilder` object is created.
    /// 2. The example config toml, TEST_TOML, is created, read and converted to a string.
    /// 3. A `TomlPartialConfigBuilder` object is constructed by passing in the toml string created
    ///    in the previous step.
    /// 4. The `TomlPartialConfigBuilder` object is added to the `ConfigBuilder`.
    ///
    /// This test then verifies the final `Config` object built from the `ConfigBuilder` object has
    /// resulted in an error because of the missing values.
    fn test_final_config_toml_err() {
        // Create a new `ConfigBuilder` object.
        let mut builder = ConfigBuilder::new();
        // Create an example toml string.
        let toml_string = to_string(&get_toml_value()).expect("Could not encode TOML value");
        // Create a `TomlPartialConfigBuilder` object from the toml string.
        let toml_builder = TomlPartialConfigBuilder::new(toml_string, TEST_TOML.to_string())
            .expect(&format!(
                "Unable to create TomlPartialConfigBuilder from: {}",
                TEST_TOML
            ));
        // Add a `PartialConfig` built from a `DefaultPartialConfigBuilder` object to the
        // `ConfigBuilder`.
        builder = builder.with_partial_config(
            toml_builder
                .build()
                .expect("Unable to build TomlPartialConfigBuilder"),
        );
        // Build the final `Config` object.
        let final_config = builder.build();
        // Asserts the final `Config` was not successfully built.
        assert!(final_config.is_err());
    }

    #[test]
    /// This test verifies that a `Config` object, constructed from just a `ClapPartialConfigBuilder`
    /// object, is unsuccessful because of the missing values, in the following steps:
    ///
    /// 1. An empty `ConfigBuilder` object is created.
    /// 2. An example `ArgMatches` object is created using `create_arg_matches`.
    /// 3. A `ClapPartialConfigBuilder` object is constructed by passing in the example `ArgMatches`
    ///    created in the previous step.
    /// 4. A `PartialConfig` built from the `ClapPartialConfigBuilder` is added to the `ConfigBuilder`.
    ///
    /// This test then verifies the `Config` object built from the `ClapPartialConfigBuilder` has
    /// resulted in an error because of the missing values.
    fn test_clap_final_config_err() {
        // Create a new `ConfigBuilder` object.
        let mut builder = ConfigBuilder::new();
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
            "--tls-insecure",
            "--no-tls",
        ];
        // Create an example `ArgMatches` object to initialize the `ClapPartialConfigBuilder`.
        let matches = create_arg_matches(args);
        // Create a new `CommandLine` object from the arg matches.
        let command_config = ClapPartialConfigBuilder::new(matches);
        // Add a `PartialConfig` built from a `DefaultPartialConfigBuilder` object to the
        // `ConfigBuilder`.
        builder = builder.with_partial_config(
            command_config
                .build()
                .expect("Unable to build ClapPartialConfigBuilder"),
        );
        let final_config = builder.build();
        // Assert the `Config` object was not successfully built.
        assert!(final_config.is_err());
    }

    #[test]
    // This test potentially interactions with other tests that set the environment variables used
    // within. It also fails to reset the environment variables to their original values.
    #[ignore]
    /// This test verifies that a `Config` object, constructed from multiple config modules, contains
    /// the correct values, giving `ClapPartialConfigBuilder` values ultimate precedence, using the
    /// following steps:
    ///
    /// 1. An empty `ConfigBuilder` object is created.
    /// 2. A `PartialConfig` is created from the `EnvPartialConfigBuilder` module.
    /// 3. A `PartialConfig` is created from the `DefaultPartialConfigBuilder` module.
    /// 4. A `PartialConfig` is created from the `TomlPartialConfigBuilder` module, using the
    ///    TEST_TOML string.
    /// 5. An example `ArgMatches` object is created using `create_arg_matches`.
    /// 6. A `ClapPartialConfigBuilder` object is constructed by passing in the example `ArgMatches`
    ///    created in the previous step.
    /// 7. All `PartialConfig` objects are added to the `ConfigBuilder` and the final `Config`
    ///    object is built.
    ///
    /// This test then verifies the `Config` object built from the `ConfigBuilder` object by
    /// asserting each expected value.
    fn test_final_config_precedence() {
        // Set the environment variables to populate the `EnvPartialConfigBuilder` object.
        env::set_var("SPLINTER_STATE_DIR", "test/state/");
        env::set_var("SPLINTER_CERT_DIR", "test/certs/");
        // Create a new `ConfigBuilder` object.
        let builder = ConfigBuilder::new();
        // Arguments to be used to create a `ClapPartialConfigBuilder` object.
        let args = vec![
            "configtest",
            "--node-id",
            "123",
            "--display-name",
            "Node 1",
            "--no-tls",
        ];
        // Create an example `ArgMatches` object to initialize the `ClapPartialConfigBuilder`.
        let matches = create_arg_matches(args);
        // Create a new `CommandLine` object from the arg matches.
        let command_config = ClapPartialConfigBuilder::new(matches)
            .build()
            .expect("Unable to build ClapPartialConfigBuilder");

        // Create an example toml string.
        let toml_string = to_string(&get_toml_value()).expect("Could not encode TOML value");
        // Create a `TomlPartialConfigBuilder` object from the toml string.
        let toml_config = TomlPartialConfigBuilder::new(toml_string, TEST_TOML.to_string())
            .expect(&format!(
                "Unable to create TomlPartialConfigBuilder from: {}",
                TEST_TOML
            ))
            .build()
            .expect("Unable to build TomlPartialConfigBuilder");

        // Create a `PartialConfig` from the `EnvPartialConfigBuilder` module.
        let env_config = EnvPartialConfigBuilder::new()
            .build()
            .expect("Unable to build EnvPartialConfigBuilder");

        // Create a `PartialConfig` from the `DefaultPartialConfigBuilder` module.
        let default_config = DefaultPartialConfigBuilder::new()
            .build()
            .expect("Unable to build DefaultPartialConfigBuilder");

        // Add the `PartialConfigs` to the final `ConfigBuilder` in the order of precedence.
        let final_config = builder
            .with_partial_config(command_config)
            .with_partial_config(toml_config)
            .with_partial_config(env_config)
            .with_partial_config(default_config)
            .build()
            .expect("Unable to build final Config.");

        // Assert the final configuration values.
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `storage`, but the `TomlPartialConfigBuilder` value should have precedence (source should
        // be `Toml`).
        assert_eq!(
            (final_config.storage(), final_config.storage_source()),
            (
                Some(EXAMPLE_STORAGE),
                Some(&ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                })
            )
        );

        // Both the `DefaultPartialConfigBuilder` and `ClapPartialConfigBuilder` had values for
        // `no-tls`, but the `ClapPartialConfigBuilder` value should have precedence (source
        // should be `CommandLine`).
        assert_eq!(
            (final_config.no_tls(), final_config.no_tls_source()),
            (true, &ConfigSource::CommandLine)
        );

        // The `DefaultPartialConfigBuilder` and `EnvPartialConfigBuilder` had values for
        // `tls_cert_dir`, but the `EnvPartialConfigBuilder` value should have precedence (source
        // should be `Environment`).
        assert_eq!(
            (
                final_config.tls_cert_dir(),
                final_config.tls_cert_dir_source()
            ),
            ("test/certs/", &ConfigSource::Environment)
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `tls_ca_file`, but the `TomlPartialConfigBuilder `value should have precedence (source
        // should be `Toml`).
        assert_eq!(
            (
                final_config.tls_ca_file(),
                final_config.tls_ca_file_source()
            ),
            (
                format!("{}{}", EXAMPLE_TLS_CERT_DIR, EXAMPLE_CA_CERTS).as_str(),
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                },
            )
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `tls_client_cert`, but the `TomlPartialConfigBuilder` value should have precedence
        // (source should be `Toml`).
        assert_eq!(
            (
                final_config.tls_client_cert(),
                final_config.tls_client_cert_source()
            ),
            (
                format!("{}{}", EXAMPLE_TLS_CERT_DIR, EXAMPLE_CLIENT_CERT).as_str(),
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                }
            )
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `tls_client_key`, but the `TomlPartialConfigBuilder` value should have precedence (source
        // should be `Toml`).
        assert_eq!(
            (
                final_config.tls_client_key(),
                final_config.tls_client_key_source()
            ),
            (
                format!("{}{}", EXAMPLE_TLS_CERT_DIR, EXAMPLE_CLIENT_KEY).as_str(),
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                },
            )
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `tls_server_cert`, but the `TomlPartialConfigBuilder` value should have precedence
        // (source should be `Toml`).
        assert_eq!(
            (
                final_config.tls_server_cert(),
                final_config.tls_server_cert_source()
            ),
            (
                format!("{}{}", EXAMPLE_TLS_CERT_DIR, EXAMPLE_SERVER_CERT).as_str(),
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                }
            )
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `tls_server_key`, but the `TomlPartialConfigBuilder` value should have precedence (source
        // should be `Toml`).
        assert_eq!(
            (
                final_config.tls_server_key(),
                final_config.tls_server_key_source()
            ),
            (
                format!("{}{}", EXAMPLE_TLS_CERT_DIR, EXAMPLE_SERVER_KEY).as_str(),
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                }
            )
        );
        // Both the `DefaultPartialConfigBuilder` and `TomlPartialConfigBuilder` had values for
        // `service_endpoint`, but the `TomlPartialConfigBuilder` value should have precedence
        // (source should be `Toml`).
        #[cfg(feature = "service-endpoint")]
        assert_eq!(
            (
                final_config.service_endpoint(),
                final_config.service_endpoint_source()
            ),
            (
                EXAMPLE_SERVICE_ENDPOINT,
                &ConfigSource::Toml {
                    file: TEST_TOML.to_string()
                }
            )
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for `network_endpoints`
        // (source should be `Default`).
        assert_eq!(
            (
                final_config.network_endpoints(),
                final_config.network_endpoints_source()
            ),
            (
                &[EXAMPLE_NETWORK_ENDPOINT.to_string()] as &[String],
                &ConfigSource::Default,
            )
        );
        // `advertised_endpoints` defaults to `network_endpoints` (source should be `Default`).
        assert_eq!(
            (
                final_config.advertised_endpoints(),
                final_config.advertised_endpoints_source()
            ),
            (
                &[EXAMPLE_NETWORK_ENDPOINT.to_string()] as &[String],
                &ConfigSource::Default,
            )
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for `peers` (source
        // should be `Default`).
        assert_eq!(
            (final_config.peers(), final_config.peers_source()),
            (&[] as &[String], &ConfigSource::Default,)
        );
        // Both the `TomlPartialConfigBuilder` and `ClapPartialConfigBuilder` had values for `node_id`,
        // but the `ClapPartialConfigBuilder` value should have precedence (source should be
        // `CommandLine`).
        assert_eq!(
            (final_config.node_id(), final_config.node_id_source()),
            (Some("123"), Some(&ConfigSource::CommandLine))
        );
        // The `TomlPartialConfigBuilder` and `ClapPartialConfigBuilder` had values for `display_name`,
        // but the `ClapPartialConfigBuilder` value should have precedence (source should be
        // `CommandLine`).
        assert_eq!(
            (
                final_config.display_name(),
                final_config.display_name_source()
            ),
            (Some("Node 1"), Some(&ConfigSource::CommandLine))
        );
        // The DefaultPartialConfigBuilder is the only config with a value for `rest_api_endpoint`
        // (source should be Default).
        assert_eq!(
            (
                final_config.rest_api_endpoint(),
                final_config.rest_api_endpoint_source()
            ),
            ("127.0.0.1:8080", &ConfigSource::Default)
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for `database` (source
        // should be `Default`). Should have default state file name with `EnvPartialConfigBuilder`
        // value for `state_dir`.
        assert_eq!(
            (final_config.database(), final_config.database_source()),
            ("test/state/splinter_state.db", &ConfigSource::Default)
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for
        // `registry_auto_refresh` (source should be `Default`).
        assert_eq!(
            (
                final_config.registry_auto_refresh(),
                final_config.registry_auto_refresh_source()
            ),
            (600, &ConfigSource::Default)
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for
        // `registry_forced_refresh` (source should be `Default`).
        assert_eq!(
            (
                final_config.registry_forced_refresh(),
                final_config.registry_forced_refresh_source()
            ),
            (10, &ConfigSource::Default)
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for `heartbeat`
        // (source should be `Default`).
        assert_eq!(
            (final_config.heartbeat(), final_config.heartbeat_source()),
            (30, &ConfigSource::Default)
        );
        // The `DefaultPartialConfigBuilder` is the only config with a value for
        // `admin_timeout` (source should be `Default`).
        assert_eq!(
            (
                final_config.admin_timeout(),
                final_config.admin_timeout_source()
            ),
            (Duration::from_secs(30), &ConfigSource::Default)
        );
        // Both the `DefaultPartialConfigBuilder` and `EnvPartialConfigBuilder` had values for
        // `state_dir`, but the `EnvPartialConfigBuilder` value should have precedence (source
        // should be `Environment`).
        assert_eq!(
            (final_config.state_dir(), final_config.state_dir_source()),
            ("test/state/", &ConfigSource::Environment)
        );
    }

    #[test]
    /// This test verifies that a `Config` object, created from a `DefaultPartialConfigBuilder` and
    /// `ClapPartialConfigBuilder` object holds the correct file paths, using the following steps:
    ///
    /// 1. An empty `ConfigBuilder` object is created.
    /// 2. A `PartialConfig` is created from the `DefaultPartialConfigBuilder` module.
    /// 3. An example `ArgMatches` object is created using `create_arg_matches`.
    /// 4. A `ClapPartialConfigBuilder` object is constructed by passing in the example `ArgMatches`
    ///    created in the previous step.
    /// 5. All `PartialConfig` objects are added to the `ConfigBuilder` and the final `Config`
    ///    object is built.
    ///
    /// This test then verifies the `Config` object built holds the correct file paths. The `cert_dir`
    /// value passed into the `ClapPartialConfigBuilder` object should be appended to the default
    /// file names for the certificate files.
    fn test_final_config_file_paths() {
        // Create a new `ConfigBuilder` object.
        let builder = ConfigBuilder::new();
        // Arguments to be used to create a C`lapPartialConfigBuilder` object, passing in a `cert_dir`.
        let args = vec![
            "configtest",
            "--node-id",
            "123",
            "--display-name",
            "Node 1",
            "--tls-cert-dir",
            "/my_files/",
        ];
        // Create an example `ArgMatches` object to initialize the `ClapPartialConfigBuilder`.
        let matches = create_arg_matches(args);
        // Create a new `CommandLine` object from the arg matches.
        let command_config = ClapPartialConfigBuilder::new(matches)
            .build()
            .expect("Unable to build ClapPartialConfigBuilder");

        // Create a `PartialConfig` from the `DefaultPartialConfigBuilder` module.
        let default_config = DefaultPartialConfigBuilder::new()
            .build()
            .expect("Unable to build DefaultPartialConfigBuilder");

        // Add the `PartialConfigs` to the final `ConfigBuilder` in the order of precedence.
        let final_config = builder
            .with_partial_config(command_config)
            .with_partial_config(default_config)
            .build()
            .expect("Unable to build final Config.");

        // The `DefaultPartialConfigBuilder` and `EnvPartialConfigBuilder` had values for `cert_dir`,
        // but the `EnvPartialConfigBuilder` value should have precedence (source should be
        // `Environment`).
        assert_eq!(
            (
                final_config.tls_cert_dir(),
                final_config.tls_cert_dir_source()
            ),
            ("/my_files/", &ConfigSource::CommandLine)
        );
        // The `DefaultPartialConfigBuilder` had a value for the `ca_file`, and since the `cert_dir`
        // value was provided to the `ClapPartialConfigBuilder`, the `cert_dir` value should be
        // appended to the default file name.
        assert_eq!(
            (
                final_config.tls_ca_file(),
                final_config.tls_ca_file_source()
            ),
            (
                format!("{}{}", "/my_files/", DEFAULT_CA_CERT).as_str(),
                &ConfigSource::Default,
            )
        );
        // The `DefaultPartialConfigBuilder had a value for the client_cert, and since the `cert_dir`
        // value was provided to the `ClapPartialConfigBuilder`, the `cert_dir` value should be
        // appended to the default file name.
        assert_eq!(
            (
                final_config.tls_client_cert(),
                final_config.tls_client_cert_source()
            ),
            (
                format!("{}{}", "/my_files/", DEFAULT_CLIENT_CERT).as_str(),
                &ConfigSource::Default,
            )
        );
        // The `DefaultPartialConfigBuilder` had a value for the `client_key`, and since the `cert_dir`
        // value was provided to the `ClapPartialConfigBuilder`, the `cert_dir` value should be
        // appended to the default file name.
        assert_eq!(
            (
                final_config.tls_client_key(),
                final_config.tls_client_key_source()
            ),
            (
                format!("{}{}", "/my_files/", DEFAULT_CLIENT_KEY).as_str(),
                &ConfigSource::Default,
            )
        );
        // The `DefaultPartialConfigBuilder` had a value for the `server_cert`, and since the
        // `cert_dir` value was provided to the `ClapPartialConfigBuilder`, the `cert_dir` value
        // should be appended to the default file name.
        assert_eq!(
            (
                final_config.tls_server_cert(),
                final_config.tls_server_cert_source()
            ),
            (
                format!("{}{}", "/my_files/", DEFAULT_SERVER_CERT).as_str(),
                &ConfigSource::Default,
            )
        );
        // The `DefaultPartialConfigBuilder` had a value for the `server_key`, and since the
        // `cert_dir` value was provided to the `ClapPartialConfigBuilder`, the `cert_dir` value
        // should be appended to the default file name.
        assert_eq!(
            (
                final_config.tls_server_key(),
                final_config.tls_server_key_source()
            ),
            (
                format!("{}{}", "/my_files/", DEFAULT_SERVER_KEY).as_str(),
                &ConfigSource::Default,
            )
        );
    }
}
