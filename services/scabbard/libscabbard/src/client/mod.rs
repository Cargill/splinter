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

mod builder;
mod error;

use std::time::{Duration, Instant, SystemTime};

use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Url,
};
use transact::{protocol::batch::Batch, protos::IntoBytes};

use super::hex::parse_hex;
use super::protocol::SCABBARD_PROTOCOL_VERSION;

pub use builder::ScabbardClientBuilder;
pub use error::ScabbardClientError;

/// A client that can be used to interact with scabbard services on a Splinter node.
pub struct ScabbardClient {
    url: String,
    #[cfg(feature = "client-auth")]
    auth: String,
}

impl ScabbardClient {
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
            "{}/scabbard/{}/{}/batches",
            self.url,
            service_id.circuit(),
            service_id.service_id()
        ))?;

        let body = batches.into_bytes()?;

        debug!("Submitting batches via {}", url);
        // Allowing unused_mut because request must be mutable if experimental feature
        // client-auth is enabled, if feature is removed unused_mut notation can be removed
        #[allow(unused_mut)]
        let mut request = Client::new().post(url).body(body);

        #[cfg(feature = "client-auth")]
        {
            request = request.header("Authorization", &self.auth);
        }
        let response = perform_request(request)?;

        let batch_link: Link = response.json().map_err(|err| {
            ScabbardClientError::new_with_source(
                "failed to parse response as batch link",
                err.into(),
            )
        })?;

        if let Some(wait) = wait {
            wait_for_batches(
                &self.url,
                &batch_link.link,
                wait,
                #[cfg(feature = "client-auth")]
                &self.auth,
            )
        } else {
            Ok(())
        }
    }

    /// Get the value at the given `address` in state for the scabbard instance with the given
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
            "{}/scabbard/{}/{}/state/{}",
            &self.url,
            service_id.circuit(),
            service_id.service_id(),
            address
        ))
        .map_err(|err| ScabbardClientError::new_with_source("invalid URL", err.into()))?;
        // Allowing unused_mut because request must be mutable if experimental feature
        // client-auth is enabled, if feature is removed unused_mut notation can be removed
        #[allow(unused_mut)]
        let mut request = Client::new().get(url);

        #[cfg(feature = "client-auth")]
        {
            request = request.header("Authorization", &self.auth);
        }

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

    /// Get all entries under the given address `prefix` in state for the scabbard instance with
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
            "{}/scabbard/{}/{}/state",
            &self.url,
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
        // Allowing unused_mut because request must be mutable if experimental feature
        // client-auth is enabled, if feature is removed unused_mut notation can be removed
        #[allow(unused_mut)]
        let mut request = Client::new().get(url);

        #[cfg(feature = "client-auth")]
        {
            request = request.header("Authorization", &self.auth);
        }

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

    /// Get the current state root hash of the scabbard instance with the given `service_id`.
    pub fn get_current_state_root(
        &self,
        service_id: &ServiceId,
    ) -> Result<String, ScabbardClientError> {
        let url = Url::parse(&format!(
            "{}/scabbard/{}/{}/state_root",
            &self.url,
            service_id.circuit(),
            service_id.service_id()
        ))
        .map_err(|err| ScabbardClientError::new_with_source("invalid URL", err.into()))?;
        // Allowing unused_mut because request must be mutable if experimental feature
        // client-auth is enabled, if feature is removed unused_mut notation can be removed
        #[allow(unused_mut)]
        let mut request = Client::new().get(url);

        #[cfg(feature = "client-auth")]
        {
            request = request.header("Authorization", &self.auth);
        }

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
                "failed to get current state root: {}: {}",
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
    #[cfg(feature = "client-auth")] auth: &str,
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
        // Allowing unused_mut because request must be mutable if experimental feature
        // client-auth is enabled, if feature is removed unused_mut notation can be removed
        #[allow(unused_mut)]
        let mut request = Client::new().get(url.clone());

        #[cfg(feature = "client-auth")]
        {
            request = request.header("Authorization", auth.to_string());
        }

        let response = perform_request(request)?;

        let batch_infos: Vec<BatchInfo> = response.json().map_err(|err| {
            ScabbardClientError::new_with_source(
                "failed to parse response as batch statuses",
                err.into(),
            )
        })?;

        let any_pending_batches = batch_infos
            .iter()
            .any(|info| matches!(info.status, BatchStatus::Pending | BatchStatus::Valid(_)));

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
            let any_invalid_batches = batch_infos
                .iter()
                .any(|info| matches!(info.status, BatchStatus::Invalid(_)));

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

/// Used for deserializing the batch link provided by the Scabbard REST API.
#[derive(Debug, Serialize, Deserialize)]
struct Link {
    link: String,
}

impl std::fmt::Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{\"link\": {}}}", self.link)
    }
}

/// Used for deserializing `GET /batch_status` responses.
#[derive(Debug, Serialize, Deserialize)]
struct BatchInfo {
    pub id: String,
    pub status: BatchStatus,
    pub timestamp: SystemTime,
}

/// Used by `BatchInfo` for deserializing `GET /batch_status` responses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "statusType", content = "message")]
enum BatchStatus {
    Unknown,
    Pending,
    Invalid(Vec<InvalidTransaction>),
    Valid(Vec<ValidTransaction>),
    Committed(Vec<ValidTransaction>),
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidTransaction {
    pub transaction_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct InvalidTransaction {
    pub transaction_id: String,
    pub error_message: String,
    pub error_data: Vec<u8>,
}

/// Used for deserializing error responses from the Scabbard REST API.
#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    message: String,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(all(test, feature = "rest-api", feature = "rest-api-actix"))]
mod tests {
    use super::*;

    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use actix_web::web;
    use actix_web::HttpResponse;
    use futures::future::IntoFuture;
    use splinter::rest_api::{
        Method, ProtocolVersionRangeGuard, Resource, RestApiBuilder, RestApiServerError,
        RestApiShutdownHandle,
    };

    use crate::protocol::{
        SCABBARD_ADD_BATCHES_PROTOCOL_MIN, SCABBARD_BATCH_STATUSES_PROTOCOL_MIN,
        SCABBARD_GET_STATE_PROTOCOL_MIN, SCABBARD_LIST_STATE_PROTOCOL_MIN,
        SCABBARD_STATE_ROOT_PROTOCOL_MIN,
    };

    const MOCK_CIRCUIT_ID: &str = "01234-abcde";
    const MOCK_SERVICE_ID: &str = "ABCD";
    const MOCK_BATCH_ID: &str = "batch_id";
    const MOCK_STATE_ROOT_HASH: &str = "abcd";

    const MOCK_AUTH: &str = "Bearer Cylinder:eyJhbGciOiJzZWNwMjU2azEiLCJ0eXAiOiJjeWxpbmRlcitqd3QifQ==.\
    eyJpc3MiOiIwMjA5MWEwNmNjNDZjNWUwZDg4ZTg5Mjg0OTM2ZWRiMTY4MDBiMDNiNTZhOGYxYjdlYzI5MmYyMzJiN2M4Mzg1YTIifQ==.\
    tOMakxmebss0WGWcvKCQhYo2AAo3aaMDPS28y9nfVnMXiYq98Be08CdxB0gXCY5qYHZSw53+kjuIG+8gPhXLBA==";

    /// Verify that a `ServiceId` can be correctly parsed from a fully-qualified service ID string.
    #[test]
    fn service_id_from_string() {
        assert!(ServiceId::from_string("").is_err());
        assert!(ServiceId::from_string("01234-abcde").is_err());
        assert!(ServiceId::from_string("::").is_err());
        assert!(ServiceId::from_string("01234-abcde::").is_err());
        assert!(ServiceId::from_string("::ABCD").is_err());

        let service_id = ServiceId::from_string("01234-abcde::ABCD").expect("failed to parse");
        assert_eq!(service_id.circuit(), "01234-abcde");
        assert_eq!(service_id.service_id(), "ABCD");
    }

    /// Verify the `ScabbardClient::submit` method works properly.
    #[test]
    fn submit() {
        let mut resource_manager = ResourceManager::new();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(resource_manager.resources());

        let mut builder = ScabbardClientBuilder::new();

        builder = builder.with_url(&format!("http://{}", bind_url));
        #[cfg(feature = "client-auth")]
        {
            builder = builder.with_auth(MOCK_AUTH);
        }
        let client = builder.build().expect("unable to build client");

        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a basic batch submission with no wait time is successful
        client
            .submit(&service_id, vec![], None)
            .expect("Failed to submit batches without wait");

        // Verify that basic batch submision with a wait time is successful
        client
            .submit(&service_id, vec![], Some(Duration::from_secs(1)))
            .expect("Failed to submit batches with wait");

        // Verify that an invalid URL results in an error being returned
        let mut invalid_builder = ScabbardClientBuilder::new();
        invalid_builder = invalid_builder.with_url("not a valid URL");
        #[cfg(feature = "client-auth")]
        {
            invalid_builder = invalid_builder.with_auth(MOCK_AUTH);
        }
        let client = invalid_builder.build().expect("unable to build client");
        assert!(client.submit(&service_id, vec![], None,).is_err());

        // Verify that an error response code results in an error being returned
        resource_manager.internal_server_error(true);
        assert!(client.submit(&service_id, vec![], None,).is_err());
        resource_manager.internal_server_error(false);

        // Verify that an invalid batch results in an error being returned when `wait` is requested
        resource_manager.invalid_batch(true);
        assert!(client
            .submit(&service_id, vec![], Some(Duration::from_secs(1)),)
            .is_err());
        resource_manager.invalid_batch(false);

        // Verify that a batch not getting committed before the `wait` time elapses results in an
        // error being returned
        resource_manager.dont_commit(true);
        assert!(client
            .submit(&service_id, vec![], Some(Duration::from_secs(1)),)
            .is_err());
        resource_manager.dont_commit(false);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verify that the `ScabbardClient::get_state_at_address` method works properly.
    #[test]
    fn get_state_at_address() {
        let mut resource_manager = ResourceManager::new();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(resource_manager.resources());

        let mut builder = ScabbardClientBuilder::new();

        builder = builder.with_url(&format!("http://{}", bind_url));
        #[cfg(feature = "client-auth")]
        {
            builder = builder.with_auth(MOCK_AUTH);
        }
        let client = builder.build().expect("unable to build client");
        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a request for an existing entry is successful and returns the right value
        let value = client
            .get_state_at_address(&service_id, &mock_state_entry().address)
            .expect("Failed to get state for existing entry");
        assert_eq!(value, Some(mock_state_entry().value));

        // Verify that a request for a non-existent entry is successful and returns `None`
        let value = client
            .get_state_at_address(&service_id, "012345")
            .expect("Failed to get state for non-existent entry");
        assert_eq!(value, None);

        // Verify that an invalid URL results in an error being returned
        let mut invalid_builder = ScabbardClientBuilder::new();
        invalid_builder = invalid_builder.with_url("not a valid URL");
        #[cfg(feature = "client-auth")]
        {
            invalid_builder = invalid_builder.with_auth(MOCK_AUTH);
        }
        let client = invalid_builder.build().expect("unable to build client");
        assert!(client.submit(&service_id, vec![], None,).is_err());

        // Verify that an invalid address results in an error being returned
        assert!(client
            .get_state_at_address(&service_id, "not a valid address")
            .is_err());

        // Verify that an error response code results in an error being returned
        resource_manager.internal_server_error(true);
        assert!(client
            .get_state_at_address(&service_id, &mock_state_entry().address)
            .is_err());
        resource_manager.internal_server_error(false);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verify that the `ScabbardClient::get_state_with_prefix` method works properly.
    #[test]
    fn get_state_with_prefix() {
        let mut resource_manager = ResourceManager::new();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(resource_manager.resources());

        let mut builder = ScabbardClientBuilder::new();

        builder = builder.with_url(&format!("http://{}", bind_url));
        #[cfg(feature = "client-auth")]
        {
            builder = builder.with_auth(MOCK_AUTH);
        }
        let client = builder.build().expect("unable to build client");
        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a request with no prefix is successful and returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, None)
            .expect("Failed to get all entries");
        assert_eq!(entries, vec![mock_state_entry()]);

        // Verify that a request with a prefix that contains an existing entry is successful and
        // returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, Some(&mock_state_entry().address[..2]))
            .expect("Failed to get entries under prefix with existing entry");
        assert_eq!(entries, vec![mock_state_entry()]);

        // Verify that a request with a prefix that does not contain any existing entries is
        // successful and returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, Some("01"))
            .expect("Failed to get entries under prefix with existing entry");
        assert_eq!(entries, vec![]);

        // Verify that an invalid URL results in an error being returned
        let mut invalid_builder = ScabbardClientBuilder::new();
        invalid_builder = invalid_builder.with_url("not a valid URL");
        #[cfg(feature = "client-auth")]
        {
            invalid_builder = invalid_builder.with_auth(MOCK_AUTH);
        }
        let client = invalid_builder.build().expect("unable to build client");
        assert!(client.submit(&service_id, vec![], None,).is_err());

        // Verify that an invalid address prefix results in an error being returned
        assert!(client
            .get_state_with_prefix(&service_id, Some("not a valid address"))
            .is_err());

        // Verify that an error response code results in an error being returned
        resource_manager.internal_server_error(true);
        assert!(client.get_state_with_prefix(&service_id, None).is_err());
        resource_manager.internal_server_error(false);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    /// Verify that the `ScabbardClient::get_current_state_root` method works properly.
    #[test]
    fn get_current_state_root() {
        let mut resource_manager = ResourceManager::new();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(resource_manager.resources());

        let mut builder = ScabbardClientBuilder::new();

        builder = builder.with_url(&format!("http://{}", bind_url));
        #[cfg(feature = "client-auth")]
        {
            builder = builder.with_auth(MOCK_AUTH);
        }
        let client = builder.build().expect("unable to build client");
        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a request returns the right value
        let state_root_hash = client
            .get_current_state_root(&service_id)
            .expect("Failed to get state root hash");
        assert_eq!(&state_root_hash, MOCK_STATE_ROOT_HASH);

        // Verify that an error response code results in an error being returned
        resource_manager.internal_server_error(true);
        assert!(client.get_current_state_root(&service_id).is_err());
        resource_manager.internal_server_error(false);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    struct ResourceManager {
        resources: Vec<Resource>,
        internal_server_error: Arc<AtomicBool>,
        invalid_batch: Arc<AtomicBool>,
        dont_commit: Arc<AtomicBool>,
    }

    impl ResourceManager {
        fn new() -> Self {
            let scabbard_base = format!("/scabbard/{}/{}", MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

            let internal_server_error = Arc::new(AtomicBool::new(false));
            let invalid_batch = Arc::new(AtomicBool::new(false));
            let dont_commit = Arc::new(AtomicBool::new(false));

            let mut resources = vec![];

            let scabbard_base_clone = scabbard_base.clone();
            let internal_server_error_clone = internal_server_error.clone();
            let batches = Resource::build(&format!("{}/batches", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_ADD_BATCHES_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ))
                .add_method(Method::Post, move |_, _| {
                    if internal_server_error_clone.load(Ordering::SeqCst) {
                        let response = ErrorResponse {
                            message: "Request failed".into(),
                        };
                        Box::new(
                            HttpResponse::InternalServerError()
                                .json(response)
                                .into_future(),
                        )
                    } else {
                        let link = Link {
                            link: format!(
                                "{}/batch_statuses?ids={}",
                                scabbard_base_clone, MOCK_BATCH_ID
                            ),
                        };
                        Box::new(HttpResponse::Accepted().json(link).into_future())
                    }
                });
            resources.push(batches);

            let internal_server_error_clone = internal_server_error.clone();
            let invalid_batch_clone = invalid_batch.clone();
            let dont_commit_clone = dont_commit.clone();
            let batch_statuses = Resource::build(&format!("{}/batch_statuses", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_BATCH_STATUSES_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ))
                .add_method(Method::Get, move |_, _| {
                    if internal_server_error_clone.load(Ordering::SeqCst) {
                        let response = ErrorResponse {
                            message: "Request failed".into(),
                        };
                        Box::new(
                            HttpResponse::InternalServerError()
                                .json(response)
                                .into_future(),
                        )
                    } else if invalid_batch_clone.load(Ordering::SeqCst) {
                        Box::new(
                            HttpResponse::Ok()
                                .json(vec![BatchInfo {
                                    id: MOCK_BATCH_ID.into(),
                                    status: BatchStatus::Invalid(vec![]),
                                    timestamp: SystemTime::now(),
                                }])
                                .into_future(),
                        )
                    } else if dont_commit_clone.load(Ordering::SeqCst) {
                        Box::new(
                            HttpResponse::Ok()
                                .json(vec![BatchInfo {
                                    id: MOCK_BATCH_ID.into(),
                                    status: BatchStatus::Pending,
                                    timestamp: SystemTime::now(),
                                }])
                                .into_future(),
                        )
                    } else {
                        Box::new(
                            HttpResponse::Ok()
                                .json(vec![BatchInfo {
                                    id: MOCK_BATCH_ID.into(),
                                    status: BatchStatus::Committed(vec![]),
                                    timestamp: SystemTime::now(),
                                }])
                                .into_future(),
                        )
                    }
                });
            resources.push(batch_statuses);

            let internal_server_error_clone = internal_server_error.clone();
            let state_address = Resource::build(&format!("{}/state/{{address}}", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_GET_STATE_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ))
                .add_method(Method::Get, move |request, _| {
                    let address = request
                        .match_info()
                        .get("address")
                        .expect("address should not be none");

                    if internal_server_error_clone.load(Ordering::SeqCst) {
                        let response = ErrorResponse {
                            message: "Request failed".into(),
                        };
                        Box::new(
                            HttpResponse::InternalServerError()
                                .json(response)
                                .into_future(),
                        )
                    } else if address == mock_state_entry().address {
                        Box::new(
                            HttpResponse::Ok()
                                .json(mock_state_entry().value)
                                .into_future(),
                        )
                    } else {
                        let response = ErrorResponse {
                            message: "Not found".into(),
                        };
                        Box::new(HttpResponse::NotFound().json(response).into_future())
                    }
                });
            resources.push(state_address);

            let internal_server_error_clone = internal_server_error.clone();
            let state = Resource::build(&format!("{}/state", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_LIST_STATE_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ))
                .add_method(Method::Get, move |request, _| {
                    let query: web::Query<HashMap<String, String>> =
                        web::Query::from_query(request.query_string())
                            .expect("Failed to get query string");
                    let prefix = query.get("prefix").map(String::as_str);

                    if internal_server_error_clone.load(Ordering::SeqCst) {
                        let response = ErrorResponse {
                            message: "Request failed".into(),
                        };
                        Box::new(
                            HttpResponse::InternalServerError()
                                .json(response)
                                .into_future(),
                        )
                    } else {
                        let return_entry = match prefix {
                            Some(prefix) => mock_state_entry().address.starts_with(prefix),
                            None => true,
                        };
                        let entries = if return_entry {
                            vec![mock_state_entry()]
                        } else {
                            vec![]
                        };
                        Box::new(HttpResponse::Ok().json(entries).into_future())
                    }
                });
            resources.push(state);

            let internal_server_error_clone = internal_server_error.clone();
            let state_root = Resource::build(&format!("{}/state_root", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_STATE_ROOT_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ))
                .add_method(Method::Get, move |_, _| {
                    if internal_server_error_clone.load(Ordering::SeqCst) {
                        let response = ErrorResponse {
                            message: "Request failed".into(),
                        };
                        Box::new(
                            HttpResponse::InternalServerError()
                                .json(response)
                                .into_future(),
                        )
                    } else {
                        Box::new(HttpResponse::Ok().json(MOCK_STATE_ROOT_HASH).into_future())
                    }
                });
            resources.push(state_root);

            Self {
                resources,
                internal_server_error,
                invalid_batch,
                dont_commit,
            }
        }

        fn resources(&self) -> Vec<Resource> {
            self.resources.clone()
        }

        fn internal_server_error(&mut self, val: bool) {
            self.internal_server_error.store(val, Ordering::SeqCst);
        }

        fn invalid_batch(&mut self, val: bool) {
            self.invalid_batch.store(val, Ordering::SeqCst);
        }

        fn dont_commit(&mut self, val: bool) {
            self.dont_commit.store(val, Ordering::SeqCst);
        }
    }

    fn mock_state_entry() -> StateEntry {
        StateEntry {
            address: "abcdef".into(),
            value: b"value".to_vec(),
        }
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        (10000..20000)
            .find_map(|port| {
                let bind_url = format!("127.0.0.1:{}", port);
                let result = RestApiBuilder::new()
                    .with_bind(&bind_url)
                    .add_resources(resources.clone())
                    .build()
                    .expect("Failed to build REST API")
                    .run();
                match result {
                    Ok((shutdown_handle, join_handle)) => {
                        Some((shutdown_handle, join_handle, bind_url))
                    }
                    Err(RestApiServerError::BindError(_)) => None,
                    Err(err) => panic!("Failed to run REST API: {}", err),
                }
            })
            .expect("No port available")
    }
}
