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

use std::convert::TryFrom;

/// Specifies the version of scabbard to use.
#[derive(Clone, Copy, PartialEq)]
pub enum ScabbardVersion {
    V1,
    V2,
}

impl TryFrom<Option<&str>> for ScabbardVersion {
    type Error = String;

    fn try_from(str_opt: Option<&str>) -> Result<Self, Self::Error> {
        match str_opt {
            Some("1") => Ok(Self::V1),
            Some("2") => Ok(Self::V2),
            Some(v) => Err(format!("Unsupported scabbard version: {}", v)),
            None => Ok(Self::V1),
        }
    }
}
