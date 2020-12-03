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
#[cfg(feature = "cylinder-jwt")]
pub mod cylinder;
#[cfg(feature = "oauth-github")]
pub mod github;
#[cfg(feature = "oauth-openid")]
pub mod openid;

use std::str::FromStr;

use crate::error::{InternalError, InvalidArgumentError};

/// A service that fetches identities from a backing provider
pub trait IdentityProvider: Send + Sync {
    /// Attempts to get the identity that corresponds to the given authorization. This method will
    /// return `Ok(None)` if the identity provider was not able to resolve the authorization to an
    /// identity.
    fn get_identity(&self, authorization: &Authorization) -> Result<Option<String>, InternalError>;

    /// Clone implementation for `IdentityProvider`. The implementation of the `Clone` trait for
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
pub trait AuthorizationMapping<T> {
    /// Return a value based on the given authorization value.
    fn get(&self, authorization: &Authorization) -> Result<Option<T>, InternalError>;
}

/// The authorization that is passed to an `IdentityProvider`
#[derive(PartialEq)]
pub enum Authorization {
    Bearer(BearerToken),
    Custom(String),
}

/// Parses an authorization string. This implementation will attempt to parse the string in the
/// format "<scheme> <value>" to a known scheme. If the string does not match this format or the
/// scheme is unknown, the `Authorization::Custom` variant will be returned with the whole
/// authorization string.
impl FromStr for Authorization {
    type Err = InvalidArgumentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let mut parts = str.splitn(2, ' ');
        match (parts.next(), parts.next()) {
            (Some(auth_scheme), Some(value)) => match auth_scheme {
                "Bearer" => Ok(Authorization::Bearer(value.parse()?)),
                _ => Ok(Authorization::Custom(str.to_string())),
            },
            (Some(_), None) => Ok(Authorization::Custom(str.to_string())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}

/// A bearer token of a specific type
#[derive(PartialEq)]
pub enum BearerToken {
    #[cfg(feature = "biome-credentials")]
    /// Contains a Biome JWT
    Biome(String),
    /// Contains a custom token, which is any bearer token that does not match one of the other
    /// variants of this enum
    Custom(String),
    #[cfg(feature = "cylinder-jwt")]
    /// Contains a Cylinder JWT
    Cylinder(String),
    #[cfg(feature = "oauth")]
    /// Contains an OAuth2 token
    OAuth2(String),
}

/// Parses a bearer token string. This implementation will attempt to parse the token in the format
/// "<type>:<value>" to a know type. If the token does not match this format or the type is unknown,
/// the `BearerToken::Custom` variant will be returned with the whole token value.
impl FromStr for BearerToken {
    type Err = InvalidArgumentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let mut parts = str.splitn(2, ':');
        match (parts.next(), parts.next()) {
            (Some(token_type), Some(token)) => match token_type {
                #[cfg(feature = "biome-credentials")]
                "Biome" => Ok(BearerToken::Biome(token.to_string())),
                #[cfg(feature = "cylinder-jwt")]
                "Cylinder" => Ok(BearerToken::Cylinder(token.to_string())),
                #[cfg(feature = "oauth")]
                "OAuth2" => Ok(BearerToken::OAuth2(token.to_string())),
                _ => Ok(BearerToken::Custom(str.to_string())),
            },
            (Some(_), None) => Ok(BearerToken::Custom(str.to_string())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verfifies that the `Authorization` enum is correctly parsed from strings
    #[test]
    fn parse_authorization() {
        assert!(matches!(
            "Bearer token".parse(),
            Ok(Authorization::Bearer(token)) if token == "token".parse().unwrap()
        ));

        assert!(matches!(
            "Unknown token".parse(),
            Ok(Authorization::Custom(auth_str)) if auth_str == "Unknown token"
        ));

        assert!(matches!(
            "test".parse(),
            Ok(Authorization::Custom(auth_str)) if auth_str == "test"
        ));
    }

    /// Verfifies that the `BearerToken` enum is correctly parsed from strings
    #[test]
    fn parse_bearer_token() {
        #[cfg(feature = "biome-credentials")]
        assert!(matches!(
            "Biome:test".parse(),
            Ok(BearerToken::Biome(token)) if token == "test"
        ));

        #[cfg(feature = "cylinder-jwt")]
        assert!(matches!(
            "Cylinder:test".parse(),
            Ok(BearerToken::Cylinder(token)) if token == "test"
        ));

        #[cfg(feature = "oauth")]
        assert!(matches!(
            "OAuth2:test".parse(),
            Ok(BearerToken::OAuth2(token)) if token == "test"
        ));

        assert!(matches!(
            "Unknown:test".parse(),
            Ok(BearerToken::Custom(token)) if token == "Unknown:test"
        ));

        assert!(matches!(
            "test".parse(),
            Ok(BearerToken::Custom(token)) if token == "test"
        ));
    }
}
