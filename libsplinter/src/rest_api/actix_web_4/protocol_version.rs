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

use std::convert::TryFrom;
use std::fmt::Display;

use actix_web_4::HttpRequest;

use crate::rest_error::RESTError;

pub const MIN_PROTOCOL_VERSION: usize = 1;
pub const MAX_PROTOCOL_VERSION: usize = 3;

#[non_exhaustive]
#[derive(PartialEq, PartialOrd)]
pub enum ProtocolVersion {
    One,
    Two,
    Three,
}

impl Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            ProtocolVersion::One => "1",
            ProtocolVersion::Two => "2",
            ProtocolVersion::Three => "3",
        };
        write!(f, "{}", val)
    }
}

impl From<ProtocolVersion> for usize {
    fn from(protocol_version: ProtocolVersion) -> Self {
        (&protocol_version).into()
    }
}

impl From<&ProtocolVersion> for usize {
    fn from(protocol_version: &ProtocolVersion) -> Self {
        match protocol_version {
            ProtocolVersion::One => 1,
            ProtocolVersion::Two => 2,
            ProtocolVersion::Three => 3,
        }
    }
}

impl TryFrom<usize> for ProtocolVersion {
    type Error = ();
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(ProtocolVersion::One),
            2 => Ok(ProtocolVersion::Two),
            3 => Ok(ProtocolVersion::Three),
            _ => Err(()),
        }
    }
}

impl TryFrom<&HttpRequest> for ProtocolVersion {
    type Error = RESTError;
    fn try_from(value: &HttpRequest) -> Result<Self, Self::Error> {
        match value.headers().get("SplinterProtocolVersion") {
            Some(header_value) => match header_value.to_str() {
                Ok(protocol_version) => match protocol_version {
                    "1" => Ok(ProtocolVersion::One),
                    "2" => Ok(ProtocolVersion::Two),
                    "3" => Ok(ProtocolVersion::Three),
                    val => Err(RESTError::bad_request(format!(
                        "{}: is not a valid protocol version",
                        val
                    ))),
                },
                Err(e) => Err(RESTError::bad_request(format!(
                    "Could not marshall value to string: {}",
                    e
                ))),
            },
            // In the case there is no header we want the most recent ProtocolVersion
            None => ProtocolVersion::try_from(MAX_PROTOCOL_VERSION)
                .map_err(|_| RESTError::bad_request(format!("The default value of MAX_PROTOCOL_VERSION = {} can't be converted toa ProtocolVersion",MAX_PROTOCOL_VERSION))),
        }
    }
}
