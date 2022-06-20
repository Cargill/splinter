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

use std::sync::Arc;

use actix_web::HttpResponse;
use futures::IntoFuture;
use splinter::{
    rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard},
    service::rest_api::ServiceEndpoint,
};

use crate::protocol;
#[cfg(feature = "authorization")]
use crate::service::rest_api::SCABBARD_READ_PERMISSION;
use crate::service::{Scabbard, SERVICE_TYPE};

pub fn make_get_state_root_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/state_root".into(),
        method: Method::Get,
        handler: Arc::new(move |_, _, service| {
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

            Box::new(match scabbard.get_current_state_root() {
                Ok(state_root) => HttpResponse::Ok().json(state_root).into_future(),
                Err(err) => {
                    error!("Failed to get current state root: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_STATE_ROOT_PROTOCOL_MIN,
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
    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use reqwest::{blocking::Client, StatusCode, Url};
    use sawtooth::migrations::run_sqlite_migrations;
    use sawtooth::receipt::store::diesel::DieselReceiptStore;
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

    /// Verify that the `GET /state_root` endpoint works properly.
    ///
    /// 1. Initialize a temporary instance of `ScabbardState`, set some values in state, and get
    ///    the resulting state root hash.
    /// 2. Initialize an instance of the `Scabbard` service that's backed by the same underlying
    ///    state that was set in the previous step.
    /// 3. Setup the REST API with the `GET /state_root` endpoint exposed.
    /// 3. Make a request to the endpoint, verify that the response code is 200, and check that the
    ///    body of the response contains the same state root hash that was reported in step (1).
    #[test]
    fn state_root() {
        let (merkle_state, commit_hash_store) = create_merkle_state_and_commit_hash_store();

        let receipt_store = Arc::new(DieselReceiptStore::new(
            create_connection_pool_and_migrate(":memory:".to_string()),
            None,
        ));

        // Initialize a temporary scabbard state and set some values to pre-populate the DBs, then
        // get the resulting state root hash.
        let expected_state_root = {
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
                    BytesEntry::new("abcdef".into(), b"value1".to_vec()),
                    BytesEntry::new("012345".into(), b"value2".to_vec()),
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

            state.current_state_root().to_string()
        };

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
                make_get_state_root_endpoint(),
                Arc::new(Mutex::new(scabbard)),
            )]);

        // Verify that a request is successful and the correct state root hash is returned
        let url =
            Url::parse(&format!("http://{}/state_root", bind_url)).expect("Failed to parse URL");
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
        let response_state_root: String = resp.json().expect("Failed to deserialize body");
        assert_eq!(response_state_root, expected_state_root);

        shutdown_handle
            .shutdown()
            .expect("Unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn resource_from_service_endpoint(
        service_endpoint: ServiceEndpoint,
        service: Arc<Mutex<dyn ServiceInstance>>,
    ) -> Resource {
        let mut resource = Resource::build(&service_endpoint.route);
        for request_guard in service_endpoint.request_guards.into_iter() {
            resource = resource.add_request_guard(request_guard);
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

    struct NoOpScabbardStatePurgeHandlerHandler;

    impl ScabbardStatePurgeHandler for NoOpScabbardStatePurgeHandlerHandler {
        fn purge_state(&self) -> Result<(), InternalError> {
            Ok(())
        }
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
    ) -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(connection_string);
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
