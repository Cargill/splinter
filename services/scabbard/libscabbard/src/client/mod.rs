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

//! A convenient client for interacting with scabbard services on a Splinter node.

mod error;
#[cfg(feature = "reqwest")]
mod reqwest;

pub use self::error::ScabbardClientError;
#[cfg(feature = "reqwest")]
pub use self::reqwest::ReqwestScabbardClient as ScabbardClient;
#[cfg(feature = "reqwest")]
pub use self::reqwest::ReqwestScabbardClientBuilder as ScabbardClientBuilder;

/// A fully-qualified service ID (circuit and service ID)
pub struct ServiceId {
    circuit: String,
    service_id: String,
}

impl ServiceId {
    /// Create a new `ServiceId` from separate `circuit` and `service_id` strings.
    pub fn new(circuit: &str, service_id: &str) -> Self {
        Self {
            circuit: circuit.into(),
            service_id: service_id.into(),
        }
    }

    /// Parse a fully-qualified service ID string (in the form "circuit::service_id").
    pub fn from_string(full_id: &str) -> Result<Self, ScabbardClientError> {
        let ids = full_id.splitn(2, "::").collect::<Vec<_>>();

        let circuit = (*ids
            .get(0)
            .ok_or_else(|| ScabbardClientError::new("service ID invalid: cannot be empty"))?)
        .to_string();
        if circuit.is_empty() {
            return Err(ScabbardClientError::new(
                "service ID invalid: circuit ID cannot be empty",
            ));
        }

        let service_id = (*ids.get(1).ok_or_else(|| {
            ScabbardClientError::new(
                "service ID invalid: must be of the form 'circuit_id::service_id'",
            )
        })?)
        .to_string();
        if service_id.is_empty() {
            return Err(ScabbardClientError::new(
                "service ID invalid: service ID cannot be empty",
            ));
        }

        Ok(Self {
            circuit,
            service_id,
        })
    }

    /// Get the circuit ID.
    pub fn circuit(&self) -> &str {
        &self.circuit
    }

    /// Get the service ID.
    pub fn service_id(&self) -> &str {
        &self.service_id
    }
}

/// Represents an entry in a Scabbard service's state.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct StateEntry {
    address: String,
    value: Vec<u8>,
}

impl StateEntry {
    /// Get the address of the entry.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Get the value of the entry.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}
