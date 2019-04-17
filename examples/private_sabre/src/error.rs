// Copyright 2019 Cargill Incorporated
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

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ServiceError {
    LoggingInitializationError(Box<dyn Error>),
    ConfigurationError(Box<dyn Error>),
}

impl Error for ServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ServiceError::LoggingInitializationError(err) => Some(&**err),
            ServiceError::ConfigurationError(err) => Some(&**err),
        }
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServiceError::LoggingInitializationError(err) => {
                write!(f, "Unable to initialize logging: {}", err)
            }
            ServiceError::ConfigurationError(err) => write!(f, "Configuration Error: {}", err),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ConfigurationError {
    MissingValue(String),
    EmptyValue(String),
    InvalidValue {
        config_field_name: String,
        message: String,
    },
}

impl Error for ConfigurationError {}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigurationError::MissingValue(config_field_name) => {
                write!(f, "Missing configuration for {}", config_field_name)
            }
            ConfigurationError::EmptyValue(config_field_name) => {
                write!(f, "{} must not be empty", config_field_name)
            }
            ConfigurationError::InvalidValue {
                config_field_name,
                message,
            } => write!(f, "Invalid value for {}: {}", config_field_name, message,),
        }
    }
}

impl From<ConfigurationError> for ServiceError {
    fn from(err: ConfigurationError) -> Self {
        ServiceError::ConfigurationError(Box::new(err))
    }
}

impl From<log::SetLoggerError> for ServiceError {
    fn from(err: log::SetLoggerError) -> Self {
        ServiceError::LoggingInitializationError(Box::new(err))
    }
}
