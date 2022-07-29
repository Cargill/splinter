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

pub mod batch_statuses;
pub mod batches;
pub mod state;
pub mod state_address;
pub mod state_root;
#[cfg(feature = "websocket")]
pub mod ws_subscribe;

use crate::service::{ServiceEndpoint, ServiceEndpointProvider};

pub struct ScabbardServiceEndpointProvider {
    endpoints: Vec<ServiceEndpoint>,
}

impl ServiceEndpointProvider for ScabbardServiceEndpointProvider {
    fn endpoints(&self) -> Vec<ServiceEndpoint> {
        self.endpoints.clone()
    }
}

impl ScabbardServiceEndpointProvider {
    fn new(endpoints: Vec<ServiceEndpoint>) -> Self {
        Self { endpoints }
    }
}

impl Default for ScabbardServiceEndpointProvider {
    fn default() -> Self {
        let endpoints = vec![
            batches::make_add_batches_to_queue_endpoint(),
            #[cfg(feature = "websocket")]
            ws_subscribe::make_subscribe_endpoint(),
            batch_statuses::make_get_batch_status_endpoint(),
            state_address::make_get_state_at_address_endpoint(),
            state::make_get_state_with_prefix_endpoint(),
            state_root::make_get_state_root_endpoint(),
        ];
        Self::new(endpoints)
    }
}

#[cfg(feature = "CALEB")]
mod tests {

    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::{Duration, SystemTime};

    use actix_web::web;
    use actix_web::HttpResponse;
    use futures::future::IntoFuture;
    use scabbard::client::ReqwestScabbardClientBuilder;
    use scabbard::protocol::SCABBARD_PROTOCOL_VERSION;
    use scabbard::service::{BatchInfo, BatchStatus};
    use splinter::error::InternalError;
    use splinter::service::ServiceId;
    #[cfg(feature = "authorization")]
    use splinter_rest_api_common::auth::{
        AuthorizationHandler, AuthorizationHandlerResult, Permission,
    };
    use splinter_rest_api_common::auth::{AuthorizationHeader, Identity, IdentityProvider};
    use splinter_rest_api_common::error::RestApiServerError;
    use splinter_rest_api_common::response_models::ErrorResponse;

    use crate::framework::{
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
}
