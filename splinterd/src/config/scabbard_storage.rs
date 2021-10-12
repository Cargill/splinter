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

use std::convert::From;
use std::fmt::Display;
use std::str::FromStr;

use super::toml::ScabbardStorageToml;

#[derive(Debug, Copy, Clone)]
pub enum ScabbardStorage {
    Database,
    Lmdb,
}

pub enum ScabbardStorageError {
    ParseError(String),
}

impl Display for ScabbardStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScabbardStorageError::ParseError(msg) => {
                write!(f, "got {}, expected 'lmdb' or 'database'", msg)
            }
        }
    }
}

impl FromStr for ScabbardStorage {
    type Err = ScabbardStorageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "database" => Ok(Self::Database),
            "lmdb" => Ok(Self::Lmdb),
            _ => Err(ScabbardStorageError::ParseError(s.to_string())),
        }
    }
}

impl From<ScabbardStorageToml> for ScabbardStorage {
    fn from(other: ScabbardStorageToml) -> Self {
        match other {
            ScabbardStorageToml::Lmdb => ScabbardStorage::Lmdb,
            ScabbardStorageToml::Database => ScabbardStorage::Database,
        }
    }
}
