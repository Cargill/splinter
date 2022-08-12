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

use core::str::FromStr;

use crate::rest_api::auth::InvalidArgumentError;

/// A bearer token of a specific type
#[derive(PartialEq, Eq)]
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
            // Allowing lint in case none of `biome-credentials`, `cylinder-jwt`, or `oauth` are
            // used
            #[allow(unused_variables, clippy::match_single_binding)]
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
