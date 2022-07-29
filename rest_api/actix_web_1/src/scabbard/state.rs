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

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{web, HttpResponse};
use futures::IntoFuture;

use scabbard::protocol;
use scabbard::service::{Scabbard, SERVICE_TYPE};
use splinter_rest_api_common::response_models::ErrorResponse;
use splinter_rest_api_common::scabbard::state::StateEntryResponse;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::scabbard::SCABBARD_READ_PERMISSION;

use crate::framework::{Method, ProtocolVersionRangeGuard};
use crate::service::ServiceEndpoint;

pub fn make_get_state_with_prefix_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/state".into(),
        method: Method::Get,
        handler: Arc::new(move |request, _, service| {
            let scabbard = match service.as_any().downcast_ref::<Scabbard>() {
                Some(s) => s,
                None => {
                    error!("Failed to downcast to scabbard service");
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    );
                }
            };

            let query: web::Query<HashMap<String, String>> =
                if let Ok(q) = web::Query::from_query(request.query_string()) {
                    q
                } else {
                    return Box::new(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request("Invalid query"))
                            .into_future(),
                    );
                };

            let prefix = query.get("prefix").map(String::as_str);

            Box::new(match scabbard.get_state_with_prefix(prefix) {
                Ok(state_iter) => {
                    let res = state_iter.collect::<Result<Vec<_>, _>>();
                    match res {
                        Ok(entries) => HttpResponse::Ok()
                            .json(
                                entries
                                    .iter()
                                    .map(StateEntryResponse::from)
                                    .collect::<Vec<_>>(),
                            )
                            .into_future(),
                        Err(err) => {
                            error!("Failed to consume state iterator: {}", err);
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future()
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to get state with prefix: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        }),
        request_guards: vec![Arc::new(ProtocolVersionRangeGuard::new(
            splinter_rest_api_common::scabbard::SCABBARD_LIST_STATE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
        #[cfg(feature = "authorization")]
        permission: SCABBARD_READ_PERMISSION,
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use diesel::r2d2::{ConnectionManager, Pool};
    use reqwest::{blocking::Client, StatusCode, Url};
    use sawtooth::migrations::run_sqlite_migrations;
    use sawtooth::receipt::store::diesel::DieselReceiptStore;
    use serde_json::{to_value, Value as JsonValue};
    use transact::{
        database::{btree::BTreeDatabase, Database},
        state::merkle::INDEXES,
    };
    use transact::{
        families::command::CommandTransactionBuilder,
        protocol::command::{BytesEntry, Command, SetState},
    };

    #[cfg(feature = "authorization")]
    use splinter::rest_api::auth::authorization::{
        AuthorizationHandler, AuthorizationHandlerResult,
    };
    use splinter::{
        error::InternalError,
        rest_api::{
            auth::{
                identity::{Identity, IdentityProvider},
                AuthorizationHeader,
            },
            AuthConfig, Resource, RestApiBuilder, RestApiServerError, RestApiShutdownHandle,
        },
        service::instance::ServiceInstance,
    };

    use crate::service::state::merkle_state::{MerkleState, MerkleStateConfig};
    use crate::service::{
        state::ScabbardState, Scabbard, ScabbardStatePurgeHandler, ScabbardVersion,
    };
    use crate::store::{
        transact::{TransactCommitHashStore, CURRENT_STATE_ROOT_INDEX},
        CommitHashStore,
    };

    const MOCK_CIRCUIT_ID: &str = "abcde-01234";
    const MOCK_SERVICE_ID: &str = "ABCD";

    /// Verify that the `GET /state` endpoint works properly.
    ///
    /// 1. Initialize a temporary instance of `ScabbardState` and set some values in state; 2 with
    ///    a shared prefix, and 1 without.
    /// 2. Initialize an instance of the `Scabbard` service that's backed by the same underlying
    ///    state that was set in the previous step.
    /// 3. Setup the REST API with the `GET /state` endpoint exposed.
    /// 3. Make a request to the endpoint with no prefix, verify that the response code is 200, and
    ///    check that all the entries that were set are included in the response (there may be other
    ///    entries because the `ScabbardState` contstructor sets some state).
    /// 4. Make a request to the endpoint with the shared prefix, verify that the response code is
    ///    200, and check that the response contains only the 2 entries under that prefix.
    /// 5. Make a request to the endpoint with a prefix under which no addresses are set, verify
    ///    that the response code is 200, and check that there are no entries in the response.
    #[test]
    fn state_with_prefix() {
        let (merkle_state, commit_hash_store) = create_merkle_state_and_commit_hash_store();

        let receipt_store = Arc::new(DieselReceiptStore::new(
            create_connection_pool_and_migrate(":memory:".to_string()),
            None,
        ));

        // Initialize a temporary scabbard state and set some values; this will pre-populate the DBs
        let prefix = "abcdef".to_string();
        let address1 = format!("{}01", prefix);
        let value1 = b"value1".to_vec();
        let address2 = format!("{}02", prefix);
        let value2 = b"value2".to_vec();
        let address3 = "0123456789".to_string();
        let value3 = b"value3".to_vec();
        {
            let mut state = ScabbardState::new(
                merkle_state.clone(),
                false,
                commit_hash_store.clone(),
                receipt_store.clone(),
                #[cfg(feature = "metrics")]
                "svc0".to_string(),
                #[cfg(feature = "metrics")]
                "vzrQS-rvwf4".to_string(),
                vec![],
            )
            .expect("Failed to initialize state");

            state.start_executor().expect("Failed to start executor");

            let signing_context = Secp256k1Context::new();
            let signer = signing_context.new_signer(signing_context.new_random_private_key());
            let batch = CommandTransactionBuilder::new()
                .with_commands(vec![Command::SetState(SetState::new(vec![
                    BytesEntry::new(address1.clone(), value1.clone()),
                    BytesEntry::new(address2.clone(), value2.clone()),
                    BytesEntry::new(address3.clone(), value3.clone()),
                ]))])
                .into_transaction_builder()
                .expect("failed to convert to transaction builder")
                .into_batch_builder(&*signer)
                .expect("failed to build transaction")
                .build_pair(&*signer)
                .expect("Failed to build batch");
            state
                .prepare_change(batch)
                .expect("Failed to prepare change");
            state.commit().expect("Failed to commit change");

            state.stop_executor();
        }

        // Initialize scabbard
        let scabbard = Scabbard::new(
            MOCK_SERVICE_ID.into(),
            MOCK_CIRCUIT_ID,
            ScabbardVersion::V1,
            Default::default(),
            merkle_state,
            false,
            commit_hash_store,
            receipt_store,
            Box::new(NoOpScabbardStatePurgeHandlerHandler),
            Secp256k1Context::new().new_verifier(),
            vec![],
            None,
        )
        .expect("Failed to create scabbard");

        // Setup the REST API
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![resource_from_service_endpoint(
                make_get_state_with_prefix_endpoint(),
                Arc::new(Mutex::new(scabbard.clone())),
            )]);

        let base_url = format!("http://{}/state", bind_url);

        // Verify that a request for all state entries results in the correct entries being returned
        let url = Url::parse(&base_url).expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::SCABBARD_PROTOCOL_VERSION,
            )
            .header("Authorization", "test")
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        let entries = resp
            .json::<JsonValue>()
            .expect("Failed to deserialize body")
            .as_array()
            .expect("Response is not a JSON array")
            .to_vec();

        assert!(entries.len() >= 3);
        assert!(entries.contains(
            &to_value(StateEntryResponse::from(&(
                address1.clone(),
                value1.clone()
            )))
            .expect("Failed to convert entry1 to JsonValue")
        ));
        assert!(entries.contains(
            &to_value(StateEntryResponse::from(&(
                address2.clone(),
                value2.clone()
            )))
            .expect("Failed to convert entry2 to JsonValue")
        ));
        assert!(entries.contains(
            &to_value(StateEntryResponse::from(&(
                address3.clone(),
                value3.clone()
            )))
            .expect("Failed to convert entry3 to JsonValue")
        ));

        // Verify that a request for state entries under the shared prefix results in the correct
        // entries being returned
        let url =
            Url::parse(&format!("{}?prefix={}", base_url, prefix)).expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::SCABBARD_PROTOCOL_VERSION,
            )
            .header("Authorization", "test")
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        let entries = resp
            .json::<JsonValue>()
            .expect("Failed to deserialize body")
            .as_array()
            .expect("Response is not a JSON array")
            .to_vec();

        assert_eq!(entries.len(), 2);
        assert!(entries.contains(
            &to_value(StateEntryResponse::from(&(
                address1.clone(),
                value1.clone()
            )))
            .expect("Failed to convert entry1 to JsonValue")
        ));
        assert!(entries.contains(
            &to_value(StateEntryResponse::from(&(
                address2.clone(),
                value2.clone()
            )))
            .expect("Failed to convert entry2 to JsonValue")
        ));

        // Verify that a request for state entries under a prefix with no set addresses results in
        // no entries being returned
        let url = Url::parse(&format!("{}?prefix=abcdef0123456789", base_url))
            .expect("Failed to parse URL");
        let resp = Client::new()
            .get(url)
            .header(
                "SplinterProtocolVersion",
                protocol::SCABBARD_PROTOCOL_VERSION,
            )
            .header("Authorization", "test")
            .send()
            .expect("Failed to perform request");
        assert_eq!(resp.status(), StatusCode::OK);
        let entries = resp
            .json::<JsonValue>()
            .expect("Failed to deserialize body")
            .as_array()
            .expect("Response is not a JSON array")
            .to_vec();

        assert!(entries.is_empty());

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    struct NoOpScabbardStatePurgeHandlerHandler;

    impl ScabbardStatePurgeHandler for NoOpScabbardStatePurgeHandlerHandler {
        fn purge_state(&self) -> Result<(), InternalError> {
            Ok(())
        }
    }

    fn resource_from_service_endpoint(
        service_endpoint: ServiceEndpoint,
        service: Arc<Mutex<dyn ServiceInstance>>,
    ) -> Resource {
        let mut resource = Resource::build(&service_endpoint.route);
        for request_guard in service_endpoint.request_guards.into_iter() {
            resource = resource.add_service_request_guard(request_guard);
        }
        let handler = service_endpoint.handler;
        #[cfg(feature = "authorization")]
        {
            resource.add_method(
                service_endpoint.method,
                service_endpoint.permission,
                move |request, payload| {
                    (handler)(
                        request,
                        payload,
                        &*service.lock().expect("Service lock poisoned"),
                    )
                },
            )
        }
        #[cfg(not(feature = "authorization"))]
        {
            resource.add_method(service_endpoint.method, move |request, payload| {
                (handler)(
                    request,
                    payload,
                    &*service.lock().expect("Service lock poisoned"),
                )
            })
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

    fn create_connection_pool_and_migrate(
        connection_string: String,
    ) -> Pool<ConnectionManager<diesel::SqliteConnection>> {
        let connection_manager =
            ConnectionManager::<diesel::SqliteConnection>::new(connection_string);
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }

    fn create_merkle_state_and_commit_hash_store(
    ) -> (MerkleState, Arc<dyn CommitHashStore + Sync + Send>) {
        let mut indexes = INDEXES.to_vec();
        indexes.push(CURRENT_STATE_ROOT_INDEX);
        let db = BTreeDatabase::new(&indexes);
        let merkle_state = MerkleState::new(MerkleStateConfig::key_value(db.clone_box()))
            .expect("Unable to create merkle state");
        let commit_hash_store = TransactCommitHashStore::new(db);
        (merkle_state, Arc::new(commit_hash_store))
    }
}
