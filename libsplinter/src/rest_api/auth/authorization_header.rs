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

use crate::rest_api::auth::{BearerToken, InvalidArgumentError};

/// The possible outcomes of attempting to authorize a client

/// A parsed authorization header
#[derive(PartialEq, Eq)]
pub enum AuthorizationHeader {
    Bearer(BearerToken),
    Custom(String),
}

/// Parses an authorization string. This implementation will attempt to parse the string in the
/// format "<scheme> <value>" to a known scheme. If the string does not match this format or the
/// scheme is unknown, the `AuthorizationHeader::Custom` variant will be returned with the whole
/// authorization string.
impl FromStr for AuthorizationHeader {
    type Err = InvalidArgumentError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let mut parts = str.splitn(2, ' ');
        match (parts.next(), parts.next()) {
            (Some(auth_scheme), Some(value)) => match auth_scheme {
                "Bearer" => Ok(AuthorizationHeader::Bearer(value.parse()?)),
                _ => Ok(AuthorizationHeader::Custom(str.to_string())),
            },
            (Some(_), None) => Ok(AuthorizationHeader::Custom(str.to_string())),
            _ => unreachable!(), // splitn always returns at least one item
        }
    }
}
