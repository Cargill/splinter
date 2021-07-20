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

//! Splinter specific representation of a public key

use std::cmp::Ordering;

/// Local representation of a public ket
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicKey {
    bytes: Vec<u8>,
}

impl PublicKey {
    /// Create a `PublicKey` from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        PublicKey { bytes }
    }

    /// Consumes the public key and returns it as bytes
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns the public key as bytes
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
