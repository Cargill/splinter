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

mod error;
mod scar;
mod submit;

use reqwest::{blocking::Client, Url};
use sawtooth_sdk::messages::batch::BatchList;

use crate::hex::parse_hex;
use crate::protocol::SCABBARD_PROTOCOL_VERSION;

use super::SERVICE_TYPE;

pub use error::Error;
pub use scar::{SabreSmartContractDefinition, SabreSmartContractMetadata};
use submit::{submit_batches, wait_for_batches};

/// A client that can be used to submit transactions to scabbard services on a Splinter node.
pub struct ScabbardClient {
    url: String,
}

impl ScabbardClient {
    /// Create a new `ScabbardClient` with the given base `url`. The `url` should be the endpoint
    /// of the Splinter node; it should not include the endpoint of the scabbard service itself.
    pub fn new(url: &str) -> Self {
        Self { url: url.into() }
    }

    /// Submit the given batches to the scabbard service specified by the circuit and service IDs.
    /// Optionally wait the given number of seconds for batches to commit.
    pub fn submit(
        &self,
        service_id: &ServiceId,
        batches: BatchList,
        wait: Option<u64>,
    ) -> Result<(), Error> {
        let batch_link = submit_batches(
            &self.url,
            service_id.circuit(),
            service_id.service_id(),
            batches,
        )?;
        if let Some(wait_secs) = wait {
            wait_for_batches(&self.url, &batch_link, wait_secs)
        } else {
            Ok(())
        }
    }

    pub fn get_state_at_address(
        &self,
        service_id: &ServiceId,
        address: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
        parse_hex(address).map_err(|err| Error::new_with_source("invalid address", err.into()))?;

        let url = Url::parse(&format!(
            "{}/{}/{}/{}/state/{}",
            &self.url,
            SERVICE_TYPE,
            service_id.circuit(),
            service_id.service_id(),
            address
        ))
        .map_err(|err| Error::new_with_source("invalid URL", err.into()))?;

        let request = Client::new().get(url);
        let response = request
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .send()
            .map_err(|err| Error::new_with_source("request failed", err.into()))?;

        if response.status().is_success() {
            Ok(Some(response.json().map_err(|err| {
                Error::new_with_source("failed to deserialize response body", err.into())
            })?))
        } else if response.status().as_u16() == 404 {
            Ok(None)
        } else {
            let status = response.status();
            let msg: ErrorResponse = response.json().map_err(|err| {
                Error::new_with_source("failed to deserialize error response body", err.into())
            })?;
            Err(Error::new(&format!(
                "failed to get state at address: {}: {}",
                status, msg
            )))
        }
    }

    pub fn get_state_with_prefix(
        &self,
        service_id: &ServiceId,
        prefix: Option<&str>,
    ) -> Result<Vec<StateEntry>, Error> {
        let mut url = Url::parse(&format!(
            "{}/{}/{}/{}/state",
            &self.url,
            SERVICE_TYPE,
            service_id.circuit(),
            service_id.service_id()
        ))
        .map_err(|err| Error::new_with_source("invalid URL", err.into()))?;
        if let Some(prefix) = prefix {
            parse_hex(prefix)
                .map_err(|err| Error::new_with_source("invalid prefix", err.into()))?;
            if prefix.len() > 70 {
                return Err(Error::new("prefix must be less than 70 characters"));
            }
            url.set_query(Some(&format!("prefix={}", prefix)))
        }

        let request = Client::new().get(url);
        let response = request
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .send()
            .map_err(|err| Error::new_with_source("request failed", err.into()))?;

        if response.status().is_success() {
            response.json().map_err(|err| {
                Error::new_with_source("failed to deserialize response body", err.into())
            })
        } else {
            let status = response.status();
            let msg: ErrorResponse = response.json().map_err(|err| {
                Error::new_with_source("failed to deserialize error response body", err.into())
            })?;
            Err(Error::new(&format!(
                "failed to get state with prefix: {}: {}",
                status, msg
            )))
        }
    }
}

/// A fully-qualified service ID (circuit and service ID)
pub struct ServiceId {
    circuit: String,
    service_id: String,
}

impl ServiceId {
    /// Parse a fully-qualified service ID string ("circuit::service_id").
    pub fn new(full_id: &str) -> Result<Self, Error> {
        let ids = full_id.splitn(2, "::").collect::<Vec<_>>();
        let circuit = (*ids
            .get(0)
            .ok_or_else(|| Error::new("service ID invalid: cannot be empty"))?)
        .to_string();
        let service_id = (*ids.get(1).ok_or_else(|| {
            Error::new("service ID invalid: must be of the form 'circuit_id::service_id'")
        })?)
        .to_string();

        Ok(Self {
            circuit,
            service_id,
        })
    }

    pub fn circuit(&self) -> &str {
        &self.circuit
    }

    pub fn service_id(&self) -> &str {
        &self.service_id
    }
}

#[derive(Deserialize, Debug)]
pub struct StateEntry {
    address: String,
    value: Vec<u8>,
}

impl StateEntry {
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    message: String,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
