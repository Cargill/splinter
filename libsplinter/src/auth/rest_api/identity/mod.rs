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

//! Tools for identifying clients and users

#[cfg(feature = "biome-credentials")]
pub mod biome;
mod error;
#[cfg(feature = "oauth-github")]
pub mod github;

use std::str::FromStr;

use crate::error::InternalError;

pub use error::{AuthorizationParseError, IdentityProviderError};

/// A service that fetches identities from a backing provider
pub trait IdentityProvider: Send + Sync {
    /// Attempts to get the identity that corresponds to the given authorization
    fn get_identity(&self, authorization: &Authorization) -> Result<String, IdentityProviderError>;

    /// Clones implementation for `IdentityProvider`. The implementation of the `Clone` trait for
    /// `Box<dyn IdentityProvider>` calls this method.
    ///
    /// # Example
    ///
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn IdentityProvider> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn IdentityProvider>;
}

impl Clone for Box<dyn IdentityProvider> {
    fn clone(&self) -> Box<dyn IdentityProvider> {
        self.clone_box()
    }
}

/// A trait that fetches a value based on an Authorization.
pub trait GetByAuthorization<T> {
    /// Return a value based on the given authorization value.
    fn get(&self, authorization: &Authorization) -> Result<Option<T>, InternalError>;
}

/// The authorization that is passed to an `IdentityProvider`
pub enum Authorization {
    Bearer(BearerToken),
}

/// Parses an authorization string, which must be in the format "<scheme> <value>"
impl FromStr for Authorization {
    type Err = AuthorizationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ' ');
        match (parts.next(), parts.next()) {
            (Some(auth_scheme), Some(token)) => match auth_scheme {
                "Bearer" => Ok(Authorization::Bearer(token.parse()?)),
                other_scheme => Err(AuthorizationParseError::new(format!(
                    "unsupported authorization scheme: {}",
                    other_scheme
                ))),
            },
            (Some(_), None) => Err(AuthorizationParseError::new(
                "malformed authorization".into(),
            )),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}

/// A bearer token of a specific type
#[non_exhaustive]
pub enum BearerToken {
    #[cfg(feature = "biome-credentials")]
    /// Contains a Biome JWT
    Biome(String),
    #[cfg(feature = "oauth")]
    /// Contains an OAuth2 token
    OAuth2(String),
}

/// Parses a bearer token string, which must be in the format "<type>:<value>"
impl FromStr for BearerToken {
    type Err = AuthorizationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(token_type), Some(token)) => match token_type {
                #[cfg(feature = "biome-credentials")]
                "Biome" => Ok(BearerToken::Biome(token.to_string())),
                #[cfg(feature = "oauth")]
                "OAuth2" => Ok(BearerToken::OAuth2(token.to_string())),
                other_type => Err(AuthorizationParseError::new(format!(
                    "unsupported token type: {}",
                    other_type
                ))),
            },
            (Some(_), None) => Err(AuthorizationParseError::new("malformed token".into())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}
