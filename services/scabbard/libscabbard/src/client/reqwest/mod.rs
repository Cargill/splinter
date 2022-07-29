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

//! A ScabbardClient instance backed by the reqwest library.

mod builder;

use std::time::{Duration, Instant, SystemTime};

use reqwest::{
    blocking::{Client, RequestBuilder, Response},
    Url,
};
use serde::{Deserialize, Serialize};
use transact::{protocol::batch::Batch, protos::IntoBytes};

use crate::hex::parse_hex;
use crate::protocol::SCABBARD_PROTOCOL_VERSION;

use super::error::ScabbardClientError;
use super::ScabbardClient;
use super::{ServiceId, StateEntry};

pub use builder::ReqwestScabbardClientBuilder;

/// A client that can be used to interact with scabbard services on a Splinter node.
pub struct ReqwestScabbardClient {
    url: String,
    auth: String,
}

impl ScabbardClient for ReqwestScabbardClient {
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
    fn submit(
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
        let request = Client::new()
            .post(url)
            .body(body)
            .header("Authorization", &self.auth);
        let response = perform_request(request)?;

        let batch_link: Link = response.json().map_err(|err| {
            ScabbardClientError::new_with_source(
                "failed to parse response as batch link",
                err.into(),
            )
        })?;

        if let Some(wait) = wait {
            wait_for_batches(&self.url, &batch_link.link, wait, &self.auth)
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
    fn get_state_at_address(
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

        let response = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
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
    fn get_state_with_prefix(
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

        let response = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
            .send()
            .map_err(|err| ScabbardClientError::new_with_source("request failed", err.into()))?;

        if response.status().is_success() {
            response
                .json::<Vec<JsonStateEntry>>()
                .map(|entries| entries.into_iter().map(StateEntry::from).collect())
                .map_err(|err| {
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
    fn get_current_state_root(
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

        let response = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", SCABBARD_PROTOCOL_VERSION)
            .header("Authorization", &self.auth)
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
    auth: &str,
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
        let request = Client::new()
            .get(url_with_query.clone())
            .header("Authorization", auth.to_string());
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
            let any_invalid_or_unknown = batch_infos
                .iter()
                .any(|info| matches!(info.status, BatchStatus::Invalid(_) | BatchStatus::Unknown));

            if any_invalid_or_unknown {
                return Err(ScabbardClientError::new(&format!(
                    "one or more batches are invalid or unknown: {:?}",
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
    match url.scheme() {
        "http" => Ok(url),
        #[cfg(feature = "https")]
        "https" => Ok(url),
        scheme => Err(ScabbardClientError::new(&format!(
            "unsupported scheme ({}) in URL: {}",
            scheme, url
        ))),
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

#[derive(Serialize, Deserialize)]
struct JsonStateEntry {
    address: String,
    value: Vec<u8>,
}

impl From<JsonStateEntry> for StateEntry {
    fn from(json: JsonStateEntry) -> Self {
        let JsonStateEntry { address, value } = json;
        Self { address, value }
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

#[cfg(all(test, feature = "rest-api-actix-web-1"))]
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
    use splinter::error::InternalError;
    #[cfg(feature = "authorization")]
    use splinter_rest_api_common::auth::{
        AuthorizationHandler, AuthorizationHandlerResult, AuthorizationHeader, Identity,
        IdentityProvider, Permission,
    };
    use splinter_rest_api_common::error::RestApiServerError;

    use splinter_rest_api_actix_web_1::framework::{
        AuthConfig, Method, ProtocolVersionRangeGuard, Resource, RestApiBuilder,
        RestApiShutdownHandle,
    };

    const SCABBARD_ADD_BATCHES_PROTOCOL_MIN: u32 = 1;
    const SCABBARD_BATCH_STATUSES_PROTOCOL_MIN: u32 = 1;
    const SCABBARD_GET_STATE_PROTOCOL_MIN: u32 = 1;
    const SCABBARD_LIST_STATE_PROTOCOL_MIN: u32 = 1;
    const SCABBARD_STATE_ROOT_PROTOCOL_MIN: u32 = 1;

    const MOCK_CIRCUIT_ID: &str = "01234-abcde";
    const MOCK_SERVICE_ID: &str = "ABCD";
    const MOCK_BATCH_ID: &str = "batch_id";
    const MOCK_STATE_ROOT_HASH: &str = "abcd";

    const MOCK_AUTH: &str = "Bearer Cylinder:eyJhbGciOiJzZWNwMjU2azEiLCJ0eXAiOiJjeWxpbmRlcitqd3QifQ==.\
    eyJpc3MiOiIwMjA5MWEwNmNjNDZjNWUwZDg4ZTg5Mjg0OTM2ZWRiMTY4MDBiMDNiNTZhOGYxYjdlYzI5MmYyMzJiN2M4Mzg1YTIifQ==.\
    tOMakxmebss0WGWcvKCQhYo2AAo3aaMDPS28y9nfVnMXiYq98Be08CdxB0gXCY5qYHZSw53+kjuIG+8gPhXLBA==";

    // These have to be redefined here because the `scabbard::service::rest_api` module where these
    // are originally defined is private
    #[cfg(feature = "authorization")]
    const SCABBARD_READ_PERMISSION: Permission = Permission::Check {
        permission_id: "scabbard.read",
        permission_display_name: "Scabbard read",
        permission_description:
            "Allows the client to read scabbard services' state and batch statuses",
    };
    #[cfg(feature = "authorization")]
    const SCABBARD_WRITE_PERMISSION: Permission = Permission::Check {
        permission_id: "scabbard.write",
        permission_display_name: "Scabbard write",
        permission_description: "Allows the client to submit batches to scabbard services",
    };

    /// Verify the `ScabbardClient::submit` method works properly.
    #[test]
    fn submit() {
        let mut resource_manager = ResourceManager::new();
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(resource_manager.resources());

        let client = ReqwestScabbardClientBuilder::new()
            .with_url(&format!("http://{}", bind_url))
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");

        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a basic batch submission with no wait time is successful
        client
            .submit(&service_id, vec![], None)
            .expect("Failed to submit batches without wait");

        // Verify that basic batch submission with a wait time is successful
        client
            .submit(&service_id, vec![], Some(Duration::from_secs(1)))
            .expect("Failed to submit batches with wait");

        // Verify that an invalid URL results in an error being returned
        let client = ReqwestScabbardClientBuilder::new()
            .with_url("not a valid URL")
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
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

        let client = ReqwestScabbardClientBuilder::new()
            .with_url(&format!("http://{}", bind_url))
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
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
        let client = ReqwestScabbardClientBuilder::new()
            .with_url("not a valid URL")
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
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

        let client = ReqwestScabbardClientBuilder::new()
            .with_url(&format!("http://{}", bind_url))
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
        let service_id = ServiceId::new(MOCK_CIRCUIT_ID, MOCK_SERVICE_ID);

        // Verify that a request with no prefix is successful and returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, None)
            .expect("Failed to get all entries");
        assert_eq!(entries, vec![mock_state_entry().into()]);

        // Verify that a request with a prefix that contains an existing entry is successful and
        // returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, Some(&mock_state_entry().address[..2]))
            .expect("Failed to get entries under prefix with existing entry");
        assert_eq!(entries, vec![mock_state_entry().into()]);

        // Verify that a request with a prefix that does not contain any existing entries is
        // successful and returns the right value
        let entries = client
            .get_state_with_prefix(&service_id, Some("01"))
            .expect("Failed to get entries under prefix with existing entry");
        assert_eq!(entries, vec![]);

        // Verify that an invalid URL results in an error being returned
        let client = ReqwestScabbardClientBuilder::new()
            .with_url("not a valid URL")
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
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

        let client = ReqwestScabbardClientBuilder::new()
            .with_url(&format!("http://{}", bind_url))
            .with_auth(MOCK_AUTH)
            .build()
            .expect("unable to build client");
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
            let mut batches = Resource::build(&format!("{}/batches", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_ADD_BATCHES_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ));
            #[cfg(feature = "authorization")]
            {
                batches =
                    batches.add_method(Method::Post, SCABBARD_WRITE_PERMISSION, move |_, _| {
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
            }
            #[cfg(not(feature = "authorization"))]
            {
                batches = batches.add_method(Method::Post, move |_, _| {
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
            }
            resources.push(batches);

            let internal_server_error_clone = internal_server_error.clone();
            let invalid_batch_clone = invalid_batch.clone();
            let dont_commit_clone = dont_commit.clone();
            let mut batch_statuses = Resource::build(&format!("{}/batch_statuses", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_BATCH_STATUSES_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ));
            #[cfg(feature = "authorization")]
            {
                batch_statuses = batch_statuses.add_method(
                    Method::Get,
                    SCABBARD_READ_PERMISSION,
                    move |_, _| {
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
                    },
                );
            }
            #[cfg(not(feature = "authorization"))]
            {
                batch_statuses = batch_statuses.add_method(Method::Get, move |_, _| {
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
            }
            resources.push(batch_statuses);

            let internal_server_error_clone = internal_server_error.clone();
            let mut state_address =
                Resource::build(&format!("{}/state/{{address}}", scabbard_base)).add_request_guard(
                    ProtocolVersionRangeGuard::new(
                        SCABBARD_GET_STATE_PROTOCOL_MIN,
                        SCABBARD_PROTOCOL_VERSION,
                    ),
                );
            #[cfg(feature = "authorization")]
            {
                state_address = state_address.add_method(
                    Method::Get,
                    SCABBARD_READ_PERMISSION,
                    move |request, _| {
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
                    },
                );
            }
            #[cfg(not(feature = "authorization"))]
            {
                state_address = state_address.add_method(Method::Get, move |request, _| {
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
            }
            resources.push(state_address);

            let internal_server_error_clone = internal_server_error.clone();
            let mut state = Resource::build(&format!("{}/state", scabbard_base)).add_request_guard(
                ProtocolVersionRangeGuard::new(
                    SCABBARD_LIST_STATE_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ),
            );
            #[cfg(feature = "authorization")]
            {
                state =
                    state.add_method(Method::Get, SCABBARD_READ_PERMISSION, move |request, _| {
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
            }
            #[cfg(not(feature = "authorization"))]
            {
                state = state.add_method(Method::Get, move |request, _| {
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
            }
            resources.push(state);

            let internal_server_error_clone = internal_server_error.clone();
            let mut state_root = Resource::build(&format!("{}/state_root", scabbard_base))
                .add_request_guard(ProtocolVersionRangeGuard::new(
                    SCABBARD_STATE_ROOT_PROTOCOL_MIN,
                    SCABBARD_PROTOCOL_VERSION,
                ));
            #[cfg(feature = "authorization")]
            {
                state_root =
                    state_root.add_method(Method::Get, SCABBARD_READ_PERMISSION, move |_, _| {
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
            }
            #[cfg(not(feature = "authorization"))]
            {
                state_root = state_root.add_method(Method::Get, move |_, _| {
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
            }
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

    fn mock_state_entry() -> JsonStateEntry {
        JsonStateEntry {
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
                let rest_api_builder = RestApiBuilder::new()
                    .with_bind(&bind_url)
                    .add_resources(resources.clone())
                    .with_auth_configs(vec![AuthConfig::Custom {
                        resources: vec![],
                        identity_provider: Box::new(AlwaysAcceptIdentityProvider),
                    }]);
                #[cfg(feature = "authorization")]
                let rest_api_builder = rest_api_builder
                    .with_authorization_handlers(vec![Box::new(AlwaysAllowAuthorizationHandler)]);
                let result = rest_api_builder
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

    /// An identity provider that always returns `Ok(Some(_))`
    #[derive(Clone)]
    struct AlwaysAcceptIdentityProvider;

    impl IdentityProvider for AlwaysAcceptIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(Some(Identity::Custom("identity".into())))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    /// An authorization handler that always returns `Ok(AuthorizationHandlerResult::Allow)`
    #[cfg(feature = "authorization")]
    #[derive(Clone)]
    struct AlwaysAllowAuthorizationHandler;

    #[cfg(feature = "authorization")]
    impl AuthorizationHandler for AlwaysAllowAuthorizationHandler {
        fn has_permission(
            &self,
            _identity: &Identity,
            _permission_id: &str,
        ) -> Result<AuthorizationHandlerResult, InternalError> {
            Ok(AuthorizationHandlerResult::Allow)
        }

        fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn parse_http_url_handles_http() {
        let parsed = Url::parse("http://some.domain");
        match parse_http_url("http://some.domain") {
            Ok(url) => assert_eq!(Ok(url), parsed),
            Err(err) => panic!("expected Ok({:?}), got Err({})", parsed, err),
        }
    }

    #[cfg(feature = "https")]
    #[test]
    fn parse_http_url_handles_https() {
        let parsed = Url::parse("https://some.domain");
        match parse_http_url("https://some.domain") {
            Ok(url) => assert_eq!(Ok(url), parsed),
            Err(err) => panic!("expected Ok({:?}), got Err({})", parsed, err),
        }
    }

    #[cfg(not(feature = "https"))]
    #[test]
    fn parse_http_url_throws_on_https() {
        match parse_http_url("https://some.domain") {
            Err(err) => assert!(err
                .to_string()
                .starts_with("unsupported scheme (https) in URL: https://some.domain")),
            Ok(url) => panic!("expected Err(_), got Ok({})", url),
        }
    }

    #[test]
    fn parse_http_url_throws_on_bad_schema() {
        match parse_http_url("badschema://some.domain") {
            Err(err) => assert!(err
                .to_string()
                .starts_with("unsupported scheme (badschema) in URL: badschema://some.domain")),
            Ok(url) => panic!("expected Err(_), got Ok({})", url),
        }
    }
}
