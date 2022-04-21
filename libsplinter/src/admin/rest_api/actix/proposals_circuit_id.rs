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

//! Provides the `GET /admin/proposals/{circuit_id} endpoint for fetching circuit proposals by
//! circuit ID.

use std::convert::TryFrom;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::Future;

use crate::admin::rest_api::error::ProposalFetchError;
#[cfg(feature = "authorization")]
use crate::admin::rest_api::CIRCUIT_READ_PERMISSION;
use crate::admin::service::proposal_store::ProposalStoreFactory;
use crate::rest_api::{
    actix_web_1::{Method, ProtocolVersionRangeGuard, Resource},
    ErrorResponse, SPLINTER_PROTOCOL_VERSION,
};

use super::super::resources;

const ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN: u32 = 1;

pub fn make_fetch_proposal_resource<PSF: ProposalStoreFactory + 'static>(
    proposal_store_factory: PSF,
) -> Resource {
    let resource = Resource::build("admin/proposals/{circuit_id}").add_request_guard(
        ProtocolVersionRangeGuard::new(
            ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN,
            SPLINTER_PROTOCOL_VERSION,
        ),
    );

    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Get, CIRCUIT_READ_PERMISSION, move |r, _| {
            fetch_proposal(r, web::Data::new(proposal_store_factory.clone()))
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |r, _| {
            fetch_proposal(r, web::Data::new(proposal_store_factory.clone()))
        })
    }
}

fn fetch_proposal<PSF: ProposalStoreFactory + 'static>(
    request: HttpRequest,
    proposal_store_factory: web::Data<PSF>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let circuit_id = request
        .match_info()
        .get("circuit_id")
        .unwrap_or("")
        .to_string();

    let protocol_version = match request.headers().get("SplinterProtocolVersion") {
        Some(header_value) => match header_value.to_str() {
            Ok(protocol_version) => Ok(protocol_version.to_string()),
            Err(_) => Err(ProposalFetchError::BadRequest(
                "Unable to get SplinterProtocolVersion".to_string(),
            )),
        },
        None => Ok(format!("{}", SPLINTER_PROTOCOL_VERSION)),
    };

    Box::new(
        web::block(move || {
            let proposal = proposal_store_factory
                .new_proposal_store()
                .proposal(&circuit_id)
                .map_err(|err| ProposalFetchError::InternalError(err.to_string()))?
                .ok_or_else(|| {
                    ProposalFetchError::NotFound(format!("Unable to find proposal: {}", circuit_id))
                })?;

            Ok((proposal, protocol_version?))
        })
        .then(|res| match res {
            Ok((proposal, protocol_version)) => match protocol_version.as_str() {
                "1" => Ok(HttpResponse::Ok().json(
                    resources::v1::proposals_circuit_id::ProposalResponse::from(&proposal),
                )),
                // Handles 2
                "2" => {
                    match resources::v2::proposals_circuit_id::ProposalResponse::try_from(&proposal)
                    {
                        Ok(proposal_response) => Ok(HttpResponse::Ok().json(proposal_response)),
                        Err(err) => {
                            error!("{}", err);
                            Ok(HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error()))
                        }
                    }
                }
                _ => Ok(
                    HttpResponse::BadRequest().json(ErrorResponse::bad_request(&format!(
                        "Unsupported SplinterProtocolVersion: {}",
                        protocol_version
                    ))),
                ),
            },
            Err(err) => match err {
                BlockingError::Error(err) => match err {
                    ProposalFetchError::InternalError(_) => {
                        error!("{}", err);
                        Ok(HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error()))
                    }
                    ProposalFetchError::NotFound(err) => {
                        Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(&err)))
                    }
                    ProposalFetchError::BadRequest(err) => {
                        Ok(HttpResponse::BadRequest().json(ErrorResponse::not_found(&err)))
                    }
                },
                _ => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            },
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};
    use serde_json::{to_value, Value as JsonValue};

    use crate::admin::{
        messages::{
            AuthorizationType, CircuitProposal, CircuitStatus, CreateCircuit, DurabilityType,
            PersistenceType, ProposalType, RouteType,
        },
        service::proposal_store::{
            error::ProposalStoreError, proposal_iter::ProposalIter, ProposalStore,
        },
        store::CircuitPredicate,
    };
    use crate::error::InternalError;
    use crate::rest_api::actix_web_1::AuthConfig;
    use crate::rest_api::actix_web_1::{RestApiBuilder, RestApiShutdownHandle};
    use crate::rest_api::auth::authorization::{AuthorizationHandler, AuthorizationHandlerResult};
    use crate::rest_api::auth::identity::{Identity, IdentityProvider};
    use crate::rest_api::auth::AuthorizationHeader;

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request returns the expected proposal.
    fn test_fetch_proposal_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStoreFactory)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/{}",
            bind_url,
            get_proposal().circuit_id
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposal: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposal,
            to_value(
                resources::v2::proposals_circuit_id::ProposalResponse::try_from(&get_proposal())
                    .expect("Unable to get ProposalResponse")
            )
            .expect("failed to convert expected data")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request with protocol 1 returns the expected
    /// proposal. This test is for backwards compatibility.
    fn test_fetch_proposal_ok_v1() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStoreFactory)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/{}",
            bind_url,
            get_proposal().circuit_id
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", "1");
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposal: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposal,
            to_value(resources::v1::proposals_circuit_id::ProposalResponse::from(
                &get_proposal()
            ))
            .expect("failed to convert expected data")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request returns NotFound when an invalid
    /// circuit_id is passed.
    fn test_fetch_proposal_not_found() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStoreFactory)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/Circuit-not-valid",
            bind_url,
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("Authorization", "custom")
            .header("SplinterProtocolVersion", SPLINTER_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[derive(Clone)]
    struct MockProposalStoreFactory;

    impl ProposalStoreFactory for MockProposalStoreFactory {
        fn new_proposal_store<'a>(&'a self) -> Box<dyn ProposalStore + 'a> {
            Box::new(MockProposalStore {})
        }
    }

    #[derive(Clone)]
    struct MockProposalStore;

    impl ProposalStore for MockProposalStore {
        fn proposals(
            &self,
            _filters: Vec<CircuitPredicate>,
        ) -> Result<ProposalIter, ProposalStoreError> {
            unimplemented!()
        }

        fn proposal(
            &self,
            circuit_id: &str,
        ) -> Result<Option<CircuitProposal>, ProposalStoreError> {
            Ok(if circuit_id == &get_proposal().circuit_id {
                Some(get_proposal())
            } else {
                None
            })
        }
    }

    fn get_proposal() -> CircuitProposal {
        CircuitProposal {
            proposal_type: ProposalType::Create,
            circuit_id: "circuit1".into(),
            circuit_hash: "012345".into(),
            circuit: CreateCircuit {
                circuit_id: "circuit1".into(),
                roster: vec![],
                members: vec![],
                authorization_type: AuthorizationType::Trust,
                persistence: PersistenceType::Any,
                durability: DurabilityType::NoDurability,
                routes: RouteType::Any,
                circuit_management_type: "mgmt_type".into(),
                application_metadata: vec![],
                comments: Some("mock circuit".into()),
                display_name: Some("test_circuit".into()),
                circuit_version: 1,
                circuit_status: CircuitStatus::Active,
            },
            votes: vec![],
            requester: vec![],
            requester_node_id: "node_id".into(),
        }
    }

    fn run_rest_api_on_open_port(
        resources: Vec<Resource>,
    ) -> (RestApiShutdownHandle, std::thread::JoinHandle<()>, String) {
        #[cfg(not(feature = "https-bind"))]
        let bind = "127.0.0.1:0";
        #[cfg(feature = "https-bind")]
        let bind = crate::rest_api::BindConfig::Http("127.0.0.1:0".into());
        let identity_provider = MockIdentityProvider::default().clone_box();
        let auth_config = AuthConfig::Custom {
            resources: Vec::new(),
            identity_provider,
        };
        let authorization_handlers = vec![MockAuthorizationHandler::default().clone_box()];

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .push_auth_config(auth_config)
            .with_authorization_handlers(authorization_handlers)
            .build()
            .expect("Failed to build REST API")
            .run();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }

    #[derive(Clone, Default)]
    struct MockIdentityProvider {}

    impl IdentityProvider for MockIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(Some(Identity::Custom("custom".to_string())))
        }
        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }

    #[derive(Clone, Default)]
    struct MockAuthorizationHandler {}

    impl AuthorizationHandler for MockAuthorizationHandler {
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
