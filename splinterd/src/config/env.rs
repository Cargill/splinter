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

//! `PartialConfig` builder using values from environment variables.

use std::env;
use std::fs;
use std::path::Path;

use crate::config::{ConfigError, ConfigSource, PartialConfig, PartialConfigBuilder};

const CONFIG_DIR_ENV: &str = "SPLINTER_CONFIG_DIR";
const STATE_DIR_ENV: &str = "SPLINTER_STATE_DIR";
const CERT_DIR_ENV: &str = "SPLINTER_CERT_DIR";
const SPLINTER_HOME_ENV: &str = "SPLINTER_HOME";

pub struct EnvPartialConfigBuilder;

impl EnvPartialConfigBuilder {
    pub fn new() -> Self {
        EnvPartialConfigBuilder {}
    }
}

/// Implementatiion of the `PartialConfigBuilder` trait to create a `PartialConfig` object from the
/// environment variable config options.
impl PartialConfigBuilder for EnvPartialConfigBuilder {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let config_dir_env = match (
            env::var(CONFIG_DIR_ENV).ok(),
            env::var(SPLINTER_HOME_ENV).ok(),
        ) {
            (Some(config_dir), _) => Some(config_dir),
            (None, Some(splinter_home)) => {
                let opt_path = Path::new(&splinter_home).join("etc");
                if !opt_path.is_dir() {
                    fs::create_dir_all(&opt_path).map_err(ConfigError::StdError)?;
                }
                opt_path.to_str().map(ToOwned::to_owned)
            }
            _ => None,
        };
        let tls_cert_dir_env = match (
            env::var(CERT_DIR_ENV).ok(),
            env::var(SPLINTER_HOME_ENV).ok(),
        ) {
            (Some(tls_cert_dir), _) => Some(tls_cert_dir),
            (None, Some(splinter_home)) => {
                let opt_path = Path::new(&splinter_home).join("certs");
                if !opt_path.is_dir() {
                    fs::create_dir_all(&opt_path).map_err(ConfigError::StdError)?;
                }
                opt_path.to_str().map(ToOwned::to_owned)
            }
            _ => None,
        };
        let state_dir_env = match (
            env::var(STATE_DIR_ENV).ok(),
            env::var(SPLINTER_HOME_ENV).ok(),
        ) {
            (Some(state_dir), _) => Some(state_dir),
            (None, Some(splinter_home)) => {
                let opt_path = Path::new(&splinter_home).join("data");
                if !opt_path.is_dir() {
                    fs::create_dir_all(&opt_path).map_err(ConfigError::StdError)?;
                }
                opt_path.to_str().map(ToOwned::to_owned)
            }
            _ => None,
        };
        Ok(PartialConfig::new(ConfigSource::Environment)
            .with_config_dir(config_dir_env)
            .with_tls_cert_dir(tls_cert_dir_env)
            .with_state_dir(state_dir_env))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// This test verifies that a `PartialConfig` object, constructed from the
    /// `EnvPartialConfigBuilder` module, contains the correct values using the following steps:
    ///
    /// 1. Remove any existing environment variables which may be set.
    /// 2. A new `EnvPartialConfigBuilder` object is created.
    /// 3. The `EnvPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    /// 4. Set the environment variables for both the state and cert directories.
    /// 5. A new `EnvPartialConfigBuilder` object is created.
    /// 6. The `EnvPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    ///
    /// This test verifies each `PartialConfig` object built from the `EnvPartialConfigBuilder` module
    /// by asserting each expected value. As the environment variables were initially unset, the
    /// first `PartialConfig` should not contain any values. After the environment variables were
    /// set, the new `PartialConfig` configuration values should reflect those values.
    fn test_environment_var_set_config() {
        // Remove any existing environment variables.
        env::remove_var(STATE_DIR_ENV);
        env::remove_var(CERT_DIR_ENV);

        // Create a new EnvPartialConfigBuilder object.
        let env_var_config = EnvPartialConfigBuilder::new();
        // Build a `PartialConfig` from the `EnvPartialConfigBuilder` object created.
        let unset_config = env_var_config
            .build()
            .expect("Unable to build EnvPartialConfigBuilder");
        assert_eq!(unset_config.source(), ConfigSource::Environment);
        // Compare the generated `PartialConfig` object against the expected values.
        assert_eq!(unset_config.state_dir(), None);
        assert_eq!(unset_config.tls_cert_dir(), None);

        // Set the environment variables.
        env::set_var(STATE_DIR_ENV, "state/test/config");
        env::set_var(CERT_DIR_ENV, "cert/test/config");
        // Create a new EnvPartialConfigBuilder object.
        let env_var_config = EnvPartialConfigBuilder::new();
        // Build a `PartialConfig` from the `EnvPartialConfigBuilder` object created.
        let set_config = env_var_config
            .build()
            .expect("Unable to build EnvPartialConfigBuilder");
        assert_eq!(set_config.source(), ConfigSource::Environment);
        // Compare the generated `PartialConfig` object against the expected values.
        assert_eq!(
            set_config.state_dir(),
            Some(String::from("state/test/config"))
        );
        assert_eq!(
            set_config.tls_cert_dir(),
            Some(String::from("cert/test/config"))
        );
    }
}
