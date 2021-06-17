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

#[cfg(feature = "log-config")]
use log4rs::config::runtime::ConfigErrors;
use std::error::Error;
use std::fmt;
use std::io;

use toml::de::Error as TomlError;

#[derive(Debug)]
/// General error type used during `Config` contruction.
pub enum ConfigError {
    ReadError {
        file: String,
        err: io::Error,
    },
    TomlParseError(TomlError),
    InvalidArgument(String),
    MissingValue(String),
    InvalidVersion(String),
    StdError(io::Error),
    #[cfg(feature = "log-config")]
    LoggingError(ConfigErrors),
}

impl From<TomlError> for ConfigError {
    fn from(e: TomlError) -> Self {
        ConfigError::TomlParseError(e)
    }
}

impl From<clap::Error> for ConfigError {
    fn from(e: clap::Error) -> Self {
        ConfigError::InvalidArgument(e.to_string())
    }
}

#[cfg(feature = "log-config")]
impl From<ConfigErrors> for ConfigError {
    fn from(e: ConfigErrors) -> Self {
        ConfigError::LoggingError(e)
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::ReadError { err, .. } => Some(err),
            ConfigError::TomlParseError(source) => Some(source),
            ConfigError::InvalidArgument(_) => None,
            ConfigError::MissingValue(_) => None,
            ConfigError::InvalidVersion(_) => None,
            ConfigError::StdError(source) => Some(source),
            #[cfg(feature = "log-config")]
            ConfigError::LoggingError(err) => Some(err),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::ReadError { file, err } => write!(f, "{}: {}", err, file),
            ConfigError::TomlParseError(source) => write!(f, "Invalid File Format: {}", source),
            ConfigError::InvalidArgument(msg) => {
                write!(f, "Unable to parse command line argument: {}", msg)
            }
            ConfigError::MissingValue(msg) => write!(f, "Configuration value must be set: {}", msg),
            ConfigError::InvalidVersion(msg) => write!(f, "{}", msg),
            ConfigError::StdError(source) => write!(f, "{}", source),
            #[cfg(feature = "log-config")]
            ConfigError::LoggingError(msg) => write!(f, "{}", msg),
        }
    }
}
