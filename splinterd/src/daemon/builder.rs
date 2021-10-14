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

use std::time::Duration;

use cylinder::Signer;
use splinter::mesh::Mesh;
use splinter::peer::PeerAuthorizationToken;

use crate::daemon::error::CreateError;
use crate::daemon::SplinterDaemon;

#[derive(Default)]
pub struct SplinterDaemonBuilder {
    #[cfg(feature = "authorization-handler-allow-keys")]
    config_dir: Option<String>,
    state_dir: Option<String>,
    #[cfg(feature = "service-endpoint")]
    service_endpoint: Option<String>,
    network_endpoints: Option<Vec<String>>,
    advertised_endpoints: Option<Vec<String>>,
    initial_peers: Option<Vec<String>>,
    node_id: Option<String>,
    display_name: Option<String>,
    rest_api_endpoint: Option<String>,
    #[cfg(feature = "https-bind")]
    rest_api_server_cert: Option<String>,
    #[cfg(feature = "https-bind")]
    rest_api_server_key: Option<String>,
    db_url: Option<String>,
    registries: Vec<String>,
    registry_auto_refresh: Option<u64>,
    registry_forced_refresh: Option<u64>,
    heartbeat: Option<u64>,
    admin_timeout: Duration,
    #[cfg(feature = "rest-api-cors")]
    whitelist: Option<Vec<String>>,
    #[cfg(feature = "biome-credentials")]
    enable_biome_credentials: Option<bool>,
    #[cfg(feature = "oauth")]
    oauth_provider: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_id: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_client_secret: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_redirect_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_url: Option<String>,
    #[cfg(feature = "oauth")]
    oauth_openid_auth_params: Option<Vec<(String, String)>>,
    #[cfg(feature = "oauth")]
    oauth_openid_scopes: Option<Vec<String>>,
    strict_ref_counts: Option<bool>,
    signers: Option<Vec<Box<dyn Signer>>>,
    peering_token: Option<PeerAuthorizationToken>,
    #[cfg(feature = "scabbard-database-support")]
    enable_lmdb_state: bool,
}

impl SplinterDaemonBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "authorization-handler-allow-keys")]
    pub fn with_config_dir(mut self, value: String) -> Self {
        self.config_dir = Some(value);
        self
    }

    pub fn with_state_dir(mut self, value: String) -> Self {
        self.state_dir = Some(value);
        self
    }

    #[cfg(feature = "service-endpoint")]
    pub fn with_service_endpoint(mut self, value: String) -> Self {
        self.service_endpoint = Some(value);
        self
    }

    pub fn with_network_endpoints(mut self, value: Vec<String>) -> Self {
        self.network_endpoints = Some(value);
        self
    }

    pub fn with_advertised_endpoints(mut self, value: Vec<String>) -> Self {
        self.advertised_endpoints = Some(value);
        self
    }

    pub fn with_initial_peers(mut self, value: Vec<String>) -> Self {
        self.initial_peers = Some(value);
        self
    }

    pub fn with_node_id(mut self, value: Option<String>) -> Self {
        self.node_id = value;
        self
    }

    pub fn with_display_name(mut self, value: Option<String>) -> Self {
        self.display_name = value;
        self
    }

    pub fn with_rest_api_endpoint(mut self, value: String) -> Self {
        self.rest_api_endpoint = Some(value);
        self
    }

    #[cfg(feature = "https-bind")]
    pub fn with_rest_api_server_cert(mut self, value: String) -> Self {
        self.rest_api_server_cert = Some(value);
        self
    }

    #[cfg(feature = "https-bind")]
    pub fn with_rest_api_server_key(mut self, value: String) -> Self {
        self.rest_api_server_key = Some(value);
        self
    }

    pub fn with_db_url(mut self, value: String) -> Self {
        self.db_url = Some(value);
        self
    }

    pub fn with_registries(mut self, registries: Vec<String>) -> Self {
        self.registries = registries;
        self
    }

    pub fn with_registry_auto_refresh(mut self, value: u64) -> Self {
        self.registry_auto_refresh = Some(value);
        self
    }

    pub fn with_registry_forced_refresh(mut self, value: u64) -> Self {
        self.registry_forced_refresh = Some(value);
        self
    }

    pub fn with_heartbeat(mut self, value: u64) -> Self {
        self.heartbeat = Some(value);
        self
    }

    pub fn with_admin_timeout(mut self, value: Duration) -> Self {
        self.admin_timeout = value;
        self
    }

    #[cfg(feature = "rest-api-cors")]
    pub fn with_whitelist(mut self, value: Option<Vec<String>>) -> Self {
        self.whitelist = value;
        self
    }

    #[cfg(feature = "biome-credentials")]
    pub fn with_enable_biome_credentials(mut self, value: bool) -> Self {
        self.enable_biome_credentials = Some(value);
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_provider(mut self, value: Option<String>) -> Self {
        self.oauth_provider = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_client_id(mut self, value: Option<String>) -> Self {
        self.oauth_client_id = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_client_secret(mut self, value: Option<String>) -> Self {
        self.oauth_client_secret = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_redirect_url(mut self, value: Option<String>) -> Self {
        self.oauth_redirect_url = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_openid_url(mut self, value: Option<String>) -> Self {
        self.oauth_openid_url = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_openid_auth_params(mut self, value: Option<Vec<(String, String)>>) -> Self {
        self.oauth_openid_auth_params = value;
        self
    }

    #[cfg(feature = "oauth")]
    pub fn with_oauth_openid_scopes(mut self, value: Option<Vec<String>>) -> Self {
        self.oauth_openid_scopes = value;
        self
    }

    pub fn with_strict_ref_counts(mut self, strict_ref_counts: bool) -> Self {
        self.strict_ref_counts = Some(strict_ref_counts);
        self
    }

    pub fn with_signers(mut self, value: Vec<Box<dyn Signer>>) -> Self {
        self.signers = Some(value);
        self
    }

    pub fn with_peering_token(mut self, value: PeerAuthorizationToken) -> Self {
        self.peering_token = Some(value);
        self
    }

    #[cfg(feature = "scabbard-database-support")]
    pub fn with_lmdb_state_enabled(mut self) -> Self {
        self.enable_lmdb_state = true;
        self
    }

    pub fn build(self) -> Result<SplinterDaemon, CreateError> {
        let heartbeat = self.heartbeat.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: heartbeat".to_string())
        })?;

        let mesh = Mesh::new(512, 128);

        #[cfg(feature = "authorization-handler-allow-keys")]
        let config_dir = self.config_dir.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: config_dir".to_string())
        })?;

        let state_dir = self.state_dir.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: state_dir".to_string())
        })?;

        #[cfg(feature = "service-endpoint")]
        let service_endpoint = self.service_endpoint.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: service_endpoint".to_string())
        })?;

        let network_endpoints = self.network_endpoints.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: network_endpoints".to_string())
        })?;

        let advertised_endpoints = self.advertised_endpoints.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: advertised_endpoints".to_string())
        })?;

        let initial_peers = self.initial_peers.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: initial_peers".to_string())
        })?;

        let rest_api_endpoint = self.rest_api_endpoint.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: rest_api_endpoint".to_string())
        })?;

        #[cfg(feature = "https-bind")]
        let rest_api_ssl_settings = match (self.rest_api_server_cert, self.rest_api_server_key) {
            (Some(cert), Some(key)) => Some((cert, key)),
            (Some(_), None) | (None, Some(_)) => {
                return Err(CreateError::MissingRequiredField(
                    "Both rest_api_server_cert and rest_api_server_key must be set".into(),
                ))
            }
            (None, None) => None,
        };

        let db_url = self
            .db_url
            .ok_or_else(|| CreateError::MissingRequiredField("Missing field: db_url".to_string()))?
            .parse()
            .map_err(|e| {
                CreateError::InvalidArgument(format!("Invalid database URL provided: {}", e))
            })?;

        let registry_auto_refresh = self.registry_auto_refresh.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: registry_auto_refresh".to_string())
        })?;

        let registry_forced_refresh = self.registry_forced_refresh.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: registry_forced_refresh".to_string())
        })?;

        #[cfg(feature = "biome-credentials")]
        let enable_biome_credentials = self.enable_biome_credentials.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: enable_biome_credentials".to_string())
        })?;

        let strict_ref_counts = self.strict_ref_counts.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: strict_ref_counts".to_string())
        })?;

        let signers = self.signers.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: signers".to_string())
        })?;

        let peering_token = self.peering_token.ok_or_else(|| {
            CreateError::MissingRequiredField("Missing field: peering_token".to_string())
        })?;

        Ok(SplinterDaemon {
            #[cfg(feature = "authorization-handler-allow-keys")]
            config_dir,
            state_dir,
            #[cfg(feature = "service-endpoint")]
            service_endpoint,
            network_endpoints,
            advertised_endpoints,
            initial_peers,
            mesh,
            node_id: self.node_id,
            display_name: self.display_name,
            rest_api_endpoint,
            #[cfg(feature = "https-bind")]
            rest_api_ssl_settings,
            db_url,
            registries: self.registries,
            registry_auto_refresh,
            registry_forced_refresh,
            admin_timeout: self.admin_timeout,
            #[cfg(feature = "rest-api-cors")]
            whitelist: self.whitelist,
            #[cfg(feature = "biome-credentials")]
            enable_biome_credentials,
            #[cfg(feature = "oauth")]
            oauth_provider: self.oauth_provider,
            #[cfg(feature = "oauth")]
            oauth_client_id: self.oauth_client_id,
            #[cfg(feature = "oauth")]
            oauth_client_secret: self.oauth_client_secret,
            #[cfg(feature = "oauth")]
            oauth_redirect_url: self.oauth_redirect_url,
            #[cfg(feature = "oauth")]
            oauth_openid_url: self.oauth_openid_url,
            #[cfg(feature = "oauth")]
            oauth_openid_auth_params: self.oauth_openid_auth_params,
            #[cfg(feature = "oauth")]
            oauth_openid_scopes: self.oauth_openid_scopes,
            heartbeat,
            strict_ref_counts,
            signers,
            peering_token,
            #[cfg(feature = "scabbard-database-support")]
            enable_lmdb_state: self.enable_lmdb_state,
        })
    }
}
