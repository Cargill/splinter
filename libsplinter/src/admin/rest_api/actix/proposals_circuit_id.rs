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

//! Provides the `GET /admin/proposals/{circuit_id} endpoint for fetching circuit proposals by
//! circuit ID.

use std::convert::TryFrom;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::Future;

use crate::admin::rest_api::error::ProposalFetchError;
use crate::admin::service::proposal_store::ProposalStore;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Resource};

use super::super::resources;

pub fn make_fetch_proposal_resource<PS: ProposalStore + 'static>(proposal_store: PS) -> Resource {
    Resource::build("admin/proposals/{circuit_id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            fetch_proposal(r, web::Data::new(proposal_store.clone()))
        })
}

fn fetch_proposal<PS: ProposalStore + 'static>(
    request: HttpRequest,
    proposal_store: web::Data<PS>,
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
        None => Ok(format!("{}", protocol::ADMIN_PROTOCOL_VERSION)),
    };

    Box::new(
        web::block(move || {
            let proposal = proposal_store
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
                // Handles 2 (and catch all)
                _ => {
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
            AuthorizationType, CircuitProposal, CreateCircuit, DurabilityType, PersistenceType,
            ProposalType, RouteType,
        },
        service::proposal_store::{ProposalIter, ProposalStoreError},
        store::CircuitPredicate,
    };
    use crate::rest_api::{RestApiBuilder, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request returns the expected proposal.
    fn test_fetch_proposal_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/{}",
            bind_url,
            get_proposal().circuit_id
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
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
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/{}",
            bind_url,
            get_proposal().circuit_id
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
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
            run_rest_api_on_open_port(vec![make_fetch_proposal_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals/Circuit-not-valid",
            bind_url,
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
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
        let bind = crate::rest_api::RestApiBind::Insecure("127.0.0.1:0".into());

        let result = RestApiBuilder::new()
            .with_bind(bind)
            .add_resources(resources.clone())
            .build_insecure()
            .expect("Failed to build REST API")
            .run_insecure();
        match result {
            Ok((shutdown_handle, join_handle)) => {
                let port = shutdown_handle.port_numbers()[0];
                (shutdown_handle, join_handle, format!("127.0.0.1:{}", port))
            }
            Err(err) => panic!("Failed to run REST API: {}", err),
        }
    }
}
