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

use super::error::BiomeRestConfigBuilderError;
#[cfg(feature = "biome-credentials")]
use crate::biome::credentials::store::PasswordEncryptionCost;

const DEFAULT_ISSUER: &str = "self-issued";
const DEFAULT_DURATION: u64 = 5400; // in seconds = 90 minutes
#[cfg(feature = "biome-credentials")]
const DEFAULT_REFRESH_DURATION: u64 = 5_184_000; // in seconds = 60 days

/// Configuration for Biome REST resources
#[derive(Deserialize, Debug)]
pub struct BiomeRestConfig {
    /// The issuer for JWT tokens issued by this service
    issuer: String,
    /// Duration of JWT tokens issued by this service
    access_token_duration: Duration,
    /// Duration of refresh tokens issued by this service
    #[cfg(feature = "biome-credentials")]
    refresh_token_duration: Duration,
    #[cfg(feature = "biome-credentials")]
    /// Cost for encrypting user's password
    password_encryption_cost: PasswordEncryptionCost,
}

impl BiomeRestConfig {
    /// Returns token issuer string. Defaults to "self-issued".
    pub fn issuer(&self) -> String {
        self.issuer.to_owned()
    }

    /// Returns duration that the access token is valid.
    /// Defaults to 90 minutes.
    pub fn access_token_duration(&self) -> Duration {
        self.access_token_duration.to_owned()
    }

    /// Returns durations the refresh token is valid.
    /// Defaults to 60 days.
    #[cfg(feature = "biome-credentials")]
    pub fn refresh_token_duration(&self) -> Duration {
        self.refresh_token_duration.to_owned()
    }

    /// Returns the password encryption cost. This roughly equates to
    /// how many rounds of hashing passwords will undergo when
    /// being salted. Defaults to 12 rounds of hashing or "high".
    #[cfg(feature = "biome-credentials")]
    pub fn password_encryption_cost(&self) -> PasswordEncryptionCost {
        self.password_encryption_cost
    }
}

/// Builder for BiomeRestConfig
pub struct BiomeRestConfigBuilder {
    issuer: Option<String>,
    access_token_duration: Option<Duration>,
    #[cfg(feature = "biome-credentials")]
    refresh_token_duration: Option<Duration>,
    #[cfg(feature = "biome-credentials")]
    password_encryption_cost: Option<String>,
}

impl Default for BiomeRestConfigBuilder {
    fn default() -> BiomeRestConfigBuilder {
        BiomeRestConfigBuilder {
            issuer: Some(DEFAULT_ISSUER.to_string()),
            access_token_duration: Some(Duration::from_secs(DEFAULT_DURATION)),
            #[cfg(feature = "biome-credentials")]
            refresh_token_duration: Some(Duration::from_secs(DEFAULT_REFRESH_DURATION)),
            #[cfg(feature = "biome-credentials")]
            password_encryption_cost: Some("high".to_string()),
        }
    }
}

impl BiomeRestConfigBuilder {
    // Creates a new instance of BiomeRestConfigBuilder.
    pub fn new() -> Self {
        BiomeRestConfigBuilder {
            issuer: None,
            access_token_duration: None,
            #[cfg(feature = "biome-credentials")]
            refresh_token_duration: None,
            #[cfg(feature = "biome-credentials")]
            password_encryption_cost: None,
        }
    }

    /// Adds an issuer to the builder.
    pub fn with_issuer(mut self, issuer: &str) -> Self {
        self.issuer = Some(issuer.to_string());
        self
    }

    /// Adds an access token duration in seconds.
    pub fn with_access_token_duration_in_secs(mut self, duration: u64) -> Self {
        self.access_token_duration = Some(Duration::from_secs(duration));
        self
    }

    /// Adds a refresh token duration in seconds.
    #[cfg(feature = "biome-credentials")]
    pub fn with_refresh_token_duration_in_secs(mut self, duration: u64) -> Self {
        self.refresh_token_duration = Some(Duration::from_secs(duration));
        self
    }

    /// Adds a password encryption cost. Accepts the following strings
    /// "low", "medium", or "high".
    #[cfg(feature = "biome-credentials")]
    pub fn with_password_encryption_cost(mut self, cost: &str) -> Self {
        self.password_encryption_cost = Some(cost.to_string());
        self
    }

    /// Creates a new BiomeRestConfig.
    pub fn build(self) -> Result<BiomeRestConfig, BiomeRestConfigBuilderError> {
        let issuer = self.issuer.unwrap_or_else(|| {
            debug!("Using default value for issuer");
            DEFAULT_ISSUER.to_string()
        });

        let access_token_duration = self.access_token_duration.unwrap_or_else(|| {
            debug!("Using default value for access_token_duration");
            Duration::from_secs(DEFAULT_DURATION)
        });
        #[cfg(feature = "biome-credentials")]
        let refresh_token_duration = self
            .refresh_token_duration
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_REFRESH_DURATION));

        #[cfg(feature = "biome-credentials")]
        let password_encryption_cost: PasswordEncryptionCost = self
            .password_encryption_cost
            .unwrap_or_else(|| "high".to_string())
            .parse()
            .map_err(BiomeRestConfigBuilderError::InvalidValue)?;

        Ok(BiomeRestConfig {
            issuer,
            access_token_duration,
            #[cfg(feature = "biome-credentials")]
            refresh_token_duration,
            #[cfg(feature = "biome-credentials")]
            password_encryption_cost,
        })
    }
}
