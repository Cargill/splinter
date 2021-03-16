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

use std::str::FromStr;
use std::time::Duration;

use transact::protocol::batch::Batch;

pub use self::error::ScabbardClientError;
#[cfg(feature = "reqwest")]
pub use self::reqwest::ReqwestScabbardClient;
#[cfg(feature = "reqwest")]
pub use self::reqwest::ReqwestScabbardClientBuilder;

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

impl FromStr for ServiceId {
    type Err = ScabbardClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_string(s)
    }
}

/// Represents an entry in a Scabbard service's state.
#[derive(Debug, PartialEq)]
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

pub trait ScabbardClient {
    /// Submit the given `batches` to the scabbard service with the given `service_id`. If a `wait`
    /// time is specified, wait the given amount of time for the batches to commit.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * One or more batches were invalid (if `wait` provided)
    /// * The `wait` time has elapsed and the batches have not been committed (if `wait` provided)
    /// * An internal error based on the underlying implementation
    fn submit(
        &self,
        service_id: &ServiceId,
        batches: Vec<Batch>,
        wait: Option<Duration>,
    ) -> Result<(), ScabbardClientError>;

    /// Get the value at the given `address` in state for the scabbard instance with the given
    /// `service_id`. Returns `None` if there is no entry at the given address.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * The given address is not a valid hex address
    /// * An internal server error occurred in the scabbard service
    /// * An internal error based on the underlying implementation
    fn get_state_at_address(
        &self,
        service_id: &ServiceId,
        address: &str,
    ) -> Result<Option<Vec<u8>>, ScabbardClientError>;

    /// Get all entries under the given address `prefix` in state for the scabbard instance with
    /// the given `service_id`.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * The given `prefix` is not a valid hex address prefix
    /// * An internal server error occurred in the scabbard service
    /// * An internal error based on the underlying implementation
    fn get_state_with_prefix(
        &self,
        service_id: &ServiceId,
        prefix: Option<&str>,
    ) -> Result<Vec<StateEntry>, ScabbardClientError>;

    /// Get the current state root hash of the scabbard instance with the given `service_id`.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * An internal server error occurred in the scabbard service
    /// * An internal error based on the underlying implementation
    fn get_current_state_root(&self, service_id: &ServiceId)
        -> Result<String, ScabbardClientError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test covers parsing ServiceId values from strings. It is tested via str::parse, which
    /// is provided by the FromStr implementation.
    ///
    /// 1. Parse a valid id
    /// 2. Return err on missing service id
    /// 3. Return err on only circuit id
    /// 4. Return err on empty str
    #[test]
    fn test_parse_service_id() {
        let service_id: ServiceId = "circuit::service".parse().expect("Unable to parse");
        assert_eq!("circuit", service_id.circuit());
        assert_eq!("service", service_id.service_id());

        let parse_res: Result<ServiceId, _> = "circuit::".parse();
        assert!(matches!(parse_res, Err(ScabbardClientError { .. })));

        let parse_res: Result<ServiceId, _> = "circuit".parse();
        assert!(matches!(parse_res, Err(ScabbardClientError { .. })));

        let parse_res: Result<ServiceId, _> = "".parse();
        assert!(matches!(parse_res, Err(ScabbardClientError { .. })));
    }
}
