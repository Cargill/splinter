// Copyright 2018-2022 Cargill Incorporated
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
const SPLINTER_STRICT_REF_COUNT_ENV: &str = "SPLINTER_STRICT_REF_COUNT";
#[cfg(feature = "oauth")]
const OAUTH_PROVIDER_ENV: &str = "OAUTH_PROVIDER";
#[cfg(feature = "oauth")]
const OAUTH_CLIENT_ID_ENV: &str = "OAUTH_CLIENT_ID";
#[cfg(feature = "oauth")]
const OAUTH_CLIENT_SECRET_ENV: &str = "OAUTH_CLIENT_SECRET";
#[cfg(feature = "oauth")]
const OAUTH_REDIRECT_URL_ENV: &str = "OAUTH_REDIRECT_URL";
#[cfg(feature = "oauth")]
const OAUTH_OPENID_URL_ENV: &str = "OAUTH_OPENID_URL";
#[cfg(feature = "tap")]
const METRICS_DB_ENV: &str = "SPLINTER_INFLUX_DB";
#[cfg(feature = "tap")]
const METRICS_URL_ENV: &str = "SPLINTER_INFLUX_URL";
#[cfg(feature = "tap")]
const METRICS_USERNAME_ENV: &str = "SPLINTER_INFLUX_USERNAME";
#[cfg(feature = "tap")]
const METRICS_PASSWORD_ENV: &str = "SPLINTER_INFLUX_PASSWORD";

/// Trait that outlines a basic read-only environment variable store
pub trait EnvStore {
    /// Returns an environment variable for the given key
    ///
    /// # Arguments
    ///
    /// * `key` - A string slice that holds the name of the environment variable
    ///
    fn get(&self, key: &str) -> Option<String>;
}

/// Implementation of `EnvStore` for operating system environment variables
pub struct OsEnvStore;

impl EnvStore for OsEnvStore {
    fn get(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

pub struct EnvPartialConfigBuilder<K: EnvStore> {
    store: K,
}

/// Implementation of the `PartialConfigBuilder` trait to create a `PartialConfig` object from the
/// environment variable config options.
impl EnvPartialConfigBuilder<OsEnvStore> {
    pub fn new() -> Self {
        EnvPartialConfigBuilder {
            store: OsEnvStore {},
        }
    }
}

impl<K: EnvStore> EnvPartialConfigBuilder<K> {
    /// Returns an `EnvPartialConfigBuilder` that will fetch data from the given store.
    ///
    /// # Arguments
    ///
    /// * `store` - An instance of `EnvStore`
    ///
    #[cfg(test)]
    pub fn from_store(store: K) -> Self {
        EnvPartialConfigBuilder { store }
    }
}

impl<K: EnvStore> PartialConfigBuilder for EnvPartialConfigBuilder<K> {
    fn build(self) -> Result<PartialConfig, ConfigError> {
        let mut config = PartialConfig::new(ConfigSource::Environment);

        let config_dir_env = match (
            self.store.get(CONFIG_DIR_ENV),
            self.store.get(SPLINTER_HOME_ENV),
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
            self.store.get(CERT_DIR_ENV),
            self.store.get(SPLINTER_HOME_ENV),
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
            self.store.get(STATE_DIR_ENV),
            self.store.get(SPLINTER_HOME_ENV),
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

        let strict_ref_counts = match self.store.get(SPLINTER_STRICT_REF_COUNT_ENV) {
            Some(value) => {
                let t: bool = value.parse().unwrap_or(false);
                Some(t)
            }
            None => Some(false),
        };

        config = config
            .with_config_dir(config_dir_env)
            .with_tls_cert_dir(tls_cert_dir_env)
            .with_state_dir(state_dir_env)
            .with_strict_ref_counts(strict_ref_counts);

        #[cfg(feature = "oauth")]
        {
            config = config
                .with_oauth_provider(self.store.get(OAUTH_PROVIDER_ENV))
                .with_oauth_client_id(self.store.get(OAUTH_CLIENT_ID_ENV))
                .with_oauth_client_secret(self.store.get(OAUTH_CLIENT_SECRET_ENV))
                .with_oauth_redirect_url(self.store.get(OAUTH_REDIRECT_URL_ENV))
                .with_oauth_openid_url(self.store.get(OAUTH_OPENID_URL_ENV));
        }

        #[cfg(feature = "tap")]
        {
            config = config
                .with_influx_db(self.store.get(METRICS_DB_ENV))
                .with_influx_url(self.store.get(METRICS_URL_ENV))
                .with_influx_username(self.store.get(METRICS_USERNAME_ENV))
                .with_influx_password(self.store.get(METRICS_PASSWORD_ENV))
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Implementation of `EnvStore` that supports arbitrary hashmaps
    pub(crate) struct HashmapEnvStore {
        hashmap: HashMap<String, String>,
    }

    impl HashmapEnvStore {
        /// Returns an `EnvStore` that will fetch an environment variable using the given hashmap.
        ///
        /// # Arguments
        ///
        /// * `hashmap` - The hashmap to obtain values from
        ///
        /// # Examples
        ///
        /// ```
        /// use crate::config::env;
        /// use std::collections::HashMap;
        /// let mut hashmap: HashMap<String, String> = new HashMap();
        /// hashmap.insert("MY_ENV_VAR".to_string(), "Success!".to_string());
        /// let store = HashmapEnvStore::new(hashmap);
        /// ```
        pub fn new(hashmap: HashMap<String, String>) -> HashmapEnvStore {
            HashmapEnvStore { hashmap }
        }
    }

    impl EnvStore for HashmapEnvStore {
        fn get(&self, key: &str) -> Option<String> {
            self.hashmap.get(key).map(ToOwned::to_owned)
        }
    }

    #[test]
    /// This test verifies that a `PartialConfig` object, constructed from the
    /// `EnvPartialConfigBuilder` module, contains the correct values using the following steps:
    ///
    /// 1. A new `EnvPartialConfigBuilder` object is created mimicking an empty environment
    /// 2. The `EnvPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    /// 3. A new `EnvPartialConfigBuilder` object is created mimicking state and cert directories.
    /// 4. The `EnvPartialConfigBuilder` object is transformed to a `PartialConfig` object using
    ///    `build`.
    ///
    /// This test verifies each `PartialConfig` object built from the `EnvPartialConfigBuilder` module
    /// by asserting each expected value. As the environment variables were initially unset, the
    /// first `PartialConfig` should not contain any values. After the environment variables were
    /// set, the new `PartialConfig` configuration values should reflect those values.
    fn test_environment_var_set_config() {
        // Create a new EnvPartialConfigBuilder object.
        let hashmap = HashMap::new();
        let store = HashmapEnvStore::new(hashmap);
        let env_var_config = EnvPartialConfigBuilder::from_store(store);

        // Build a `PartialConfig` from the `EnvPartialConfigBuilder` object created.
        let unset_config = env_var_config
            .build()
            .expect("Unable to build EnvPartialConfigBuilder");
        assert_eq!(unset_config.source(), ConfigSource::Environment);
        // Compare the generated `PartialConfig` object against the expected values.
        assert_eq!(unset_config.state_dir(), None);
        assert_eq!(unset_config.tls_cert_dir(), None);

        // Create a new EnvPartialConfigBuilder object.
        let mut hashmap: HashMap<String, String> = HashMap::new();
        hashmap.insert(STATE_DIR_ENV.to_string(), "state/test/config".to_string());
        hashmap.insert(CERT_DIR_ENV.to_string(), "cert/test/config".to_string());
        let store = HashmapEnvStore::new(hashmap);
        let env_var_config = EnvPartialConfigBuilder::from_store(store);
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
