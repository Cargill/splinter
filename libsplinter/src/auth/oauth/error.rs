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

//! Errors that can occur when using OAuth2

use std::error::Error;
use std::fmt;

/// An error that can occur when configuring a provider
#[derive(Debug)]
pub enum ProviderConfigurationError {
    /// The specified authorization URL for the provider was invalid
    InvalidAuthUrl(String),
    /// The specified token URL for the provider was invalid
    InvalidTokenUrl(String),
}

impl fmt::Display for ProviderConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::InvalidAuthUrl(msg) => {
                write!(f, "provided authorization URL is invalid: {}", msg)
            }
            Self::InvalidTokenUrl(msg) => write!(f, "provided token URL is invalid: {}", msg),
        }
    }
}

impl Error for ProviderConfigurationError {}
