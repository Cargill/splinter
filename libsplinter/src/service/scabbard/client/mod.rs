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

//! A convenient client for interacting with scabbard services on a Splinter node.

mod error;

use std::time::{Duration, Instant};

use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Url,
};
use transact::{protocol::batch::Batch, protos::IntoBytes};

use crate::hex::parse_hex;
use crate::protocol::SCABBARD_PROTOCOL_VERSION;

use super::{BatchInfo, BatchStatus, SERVICE_TYPE};

pub use error::ScabbardClientError;

/// A client that can be used to interact with scabbard services on a Splinter node.
pub struct ScabbardClient {
    url: String,
}

impl ScabbardClient {
    /// Create a new `ScabbardClient` with the given base `url`. The URL should be the bind endpoint
    /// of the Splinter REST API; it should not include the path to the scabbard service itself.
    pub fn new(url: &str) -> Self {
        Self { url: url.into() }
    }

    /// Submit the given `batches` to the scabbard service with the given `service_id`. If a `wait`
    /// time is specified, wait the given amount of time for the batches to commit.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * The client's URL was invalid
    /// * A REST API request failed
    /// * An internal server error occurred in the scabbard service
    /// * One or more batches were invalid (if `wait` provided)
    /// * The `wait` time has elapsed and the batches have not been committed (if `wait` provided)
    pub fn submit(
        &self,
        service_id: &ServiceId,
        batches: Vec<Batch>,
        wait: Option<Duration>,
    ) -> Result<(), ScabbardClientError> {
        let url = parse_http_url(&format!(
            "{}/{}/{}/{}/batches",
            self.url,
            SERVICE_TYPE,
            service_id.circuit(),
            service_id.service_id()
        ))?;

        let body = batches.into_bytes()?;

        debug!("Submitting batches via {}", url);
        let request = Client::new().post(url).body(body);
        let response = perform_request(request)?;

        let batch_link: Link = response.json().map_err(|err| {
            ScabbardClientError::new_with_source(
                "failed to parse response as batch link",
                err.into(),
            )
        })?;

        if let Some(wait) = wait {
            wait_for_batches(&self.url, &batch_link.link, wait)
        } else {
            Ok(())
        }
    }

    /// Get the value at the given `address` in state for the Scabbard instance with the given
    /// `service_id`. Returns `None` if there is no entry at the given address.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * The client's URL was invalid
    /// * The given address is not a valid hex address
    /// * The REST API request failed
    /// * An internal server error occurred in the scabbard service
    pub fn get_state_at_address(
        &self,
        service_id: &ServiceId,
        address: &str,
    ) -> Result<Option<Vec<u8>>, ScabbardClientError> {
        parse_hex(address)
            .map_err(|err| ScabbardClientError::new_with_source("invalid address", err.into()))?;

        let url = Url::parse(&format!(
            "{}/{}/{}/{}/state/{}",
            &self.url,
            SERVICE_TYPE,
            service_id.circuit(),
            service_id.service_id(),
            address
        ))
        .map_err(|err| ScabbardClientError::new_with_source("invalid URL", err.into()))?;

        let request = Client::new().get(url);
        let response = request
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .send()
            .map_err(|err| ScabbardClientError::new_with_source("request failed", err.into()))?;

        if response.status().is_success() {
            Ok(Some(response.json().map_err(|err| {
                ScabbardClientError::new_with_source(
                    "failed to deserialize response body",
                    err.into(),
                )
            })?))
        } else if response.status().as_u16() == 404 {
            Ok(None)
        } else {
            let status = response.status();
            let msg: ErrorResponse = response.json().map_err(|err| {
                ScabbardClientError::new_with_source(
                    "failed to deserialize error response body",
                    err.into(),
                )
            })?;
            Err(ScabbardClientError::new(&format!(
                "failed to get state at address: {}: {}",
                status, msg
            )))
        }
    }

    /// Get all entries under the given address `prefix` in state for the Scabbard instance with
    /// the given `service_id`.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * The client's URL was invalid
    /// * The given `prefix` is not a valid hex address prefix
    /// * The REST API request failed
    /// * An internal server error occurred in the scabbard service
    pub fn get_state_with_prefix(
        &self,
        service_id: &ServiceId,
        prefix: Option<&str>,
    ) -> Result<Vec<StateEntry>, ScabbardClientError> {
        let mut url = Url::parse(&format!(
            "{}/{}/{}/{}/state",
            &self.url,
            SERVICE_TYPE,
            service_id.circuit(),
            service_id.service_id()
        ))
        .map_err(|err| ScabbardClientError::new_with_source("invalid URL", err.into()))?;
        if let Some(prefix) = prefix {
            parse_hex(prefix).map_err(|err| {
                ScabbardClientError::new_with_source("invalid prefix", err.into())
            })?;
            if prefix.len() > 70 {
                return Err(ScabbardClientError::new(
                    "prefix must be less than 70 characters",
                ));
            }
            url.set_query(Some(&format!("prefix={}", prefix)))
        }

        let request = Client::new().get(url);
        let response = request
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .send()
            .map_err(|err| ScabbardClientError::new_with_source("request failed", err.into()))?;

        if response.status().is_success() {
            response.json().map_err(|err| {
                ScabbardClientError::new_with_source(
                    "failed to deserialize response body",
                    err.into(),
                )
            })
        } else {
            let status = response.status();
            let msg: ErrorResponse = response.json().map_err(|err| {
                ScabbardClientError::new_with_source(
                    "failed to deserialize error response body",
                    err.into(),
                )
            })?;
            Err(ScabbardClientError::new(&format!(
                "failed to get state with prefix: {}: {}",
                status, msg
            )))
        }
    }
}

/// Using the given `base_url` and `batch_link` to check batch statuses, `wait` the given duration
/// for the batches (encoded in `batch_link`) to commit.
///
/// # Errors
///
/// Returns an error in any of the following cases:
/// * A batch status request failed
/// * An internal server error occurred in the scabbard service
/// * One or more batches were invalid
/// * The `wait` time has elapsed and the batches have not been committed
fn wait_for_batches(
    base_url: &str,
    batch_link: &str,
    wait: Duration,
) -> Result<(), ScabbardClientError> {
    let url = if batch_link.starts_with("http") || batch_link.starts_with("https") {
        parse_http_url(batch_link)
    } else {
        parse_http_url(&format!("{}{}", base_url, batch_link))
    }?;

    let end_time = Instant::now()
        .checked_add(wait)
        .ok_or_else(|| ScabbardClientError::new("failed to schedule timeout"))?;

    loop {
        let wait_query = format!("wait={}", wait.as_secs());
        let query_string = if let Some(existing_query) = url.query() {
            format!("{}&{}", existing_query, wait_query)
        } else {
            wait_query
        };

        let mut url_with_query = url.clone();
        url_with_query.set_query(Some(&query_string));

        debug!("Checking batches via {}", url);
        let request = Client::new().get(url.clone());
        let response = perform_request(request)?;

        let batch_infos: Vec<BatchInfo> = response.json().map_err(|err| {
            ScabbardClientError::new_with_source(
                "failed to parse response as batch statuses",
                err.into(),
            )
        })?;

        let any_pending_batches = batch_infos.iter().any(|info| {
            match info.status {
                // `Valid` is still technically pending until it's `Committed`
                BatchStatus::Pending | BatchStatus::Valid(_) => true,
                _ => false,
            }
        });

        if any_pending_batches {
            if Instant::now() < end_time {
                continue;
            } else {
                return Err(ScabbardClientError::new(&format!(
                    "one or more batches are still pending after timeout: {:?}",
                    batch_infos
                )));
            }
        } else {
            let any_invalid_batches = batch_infos.iter().any(|info| {
                if let BatchStatus::Invalid(_) = info.status {
                    true
                } else {
                    false
                }
            });

            if any_invalid_batches {
                return Err(ScabbardClientError::new(&format!(
                    "one or more batches were invalid: {:?}",
                    batch_infos
                )));
            } else {
                return Ok(());
            }
        }
    }
}

/// Parses the given `url`, returning an error if it is invalid.
fn parse_http_url(url: &str) -> Result<Url, ScabbardClientError> {
    let url = Url::parse(url)
        .map_err(|err| ScabbardClientError::new_with_source("invalid URL", err.into()))?;
    if url.scheme() != "http" {
        Err(ScabbardClientError::new(&format!(
            "unsupported scheme ({}) in URL: {}",
            url.scheme(),
            url
        )))
    } else {
        Ok(url)
    }
}

/// Performs the given `request`, returning an error if the request fails or an error status code
/// is received.
fn perform_request(request: RequestBuilder) -> Result<Response, ScabbardClientError> {
    request
        .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
        .send()
        .map_err(|err| ScabbardClientError::new_with_source("request failed", err.into()))?
        .error_for_status()
        .map_err(|err| {
            ScabbardClientError::new_with_source("received error status code", err.into())
        })
}

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
#[derive(Deserialize, Debug)]
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

/// Used for deserializing the batch link provided by the Scabbard REST API.
#[derive(Deserialize, Debug)]
struct Link {
    link: String,
}

impl std::fmt::Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{\"link\": {}}}", self.link)
    }
}

/// Used for deserializing error responses from the Scabbard REST API.
#[derive(Deserialize, Debug)]
struct ErrorResponse {
    message: String,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that a `ServiceId` can be correctly parsed from a fully-qualified service ID string.
    #[test]
    fn service_id_from_string() {
        assert!(ServiceId::from_string("").is_err());
        assert!(ServiceId::from_string("circuit").is_err());
        assert!(ServiceId::from_string("::").is_err());
        assert!(ServiceId::from_string("circuit::").is_err());
        assert!(ServiceId::from_string("::service_id").is_err());

        let service_id = ServiceId::from_string("circuit::service_id").expect("failed to parse");
        assert_eq!(service_id.circuit(), "circuit");
        assert_eq!(service_id.service_id(), "service_id");
    }
}
