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

use actix_web::{error::BlockingError, web, Error, HttpResponse};
use futures::Future;

use crate::admin::rest_api::error::ProposalFetchError;
use crate::admin::service::proposal_store::ProposalStore;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard, Request, Resource};

use super::super::resources::proposals_circuit_id::ProposalResponse;

pub fn make_fetch_proposal_resource<PS: ProposalStore + 'static>(proposal_store: PS) -> Resource {
    Resource::build("admin/proposals/{circuit_id}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_PROPOSALS_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r| {
            fetch_proposal(r, web::Data::new(proposal_store.clone()))
        })
}

fn fetch_proposal<PS: ProposalStore + 'static>(
    request: Request,
    proposal_store: web::Data<PS>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let circuit_id = request
        .path_parameter("circuit_id")
        .unwrap_or("")
        .to_string();
    Box::new(
        web::block(move || {
            proposal_store
                .proposal(&circuit_id)
                .map_err(|err| ProposalFetchError::InternalError(err.to_string()))?
                .ok_or_else(|| {
                    ProposalFetchError::NotFound(format!("Unable to find proposal: {}", circuit_id))
                })
        })
        .then(|res| match res {
            Ok(proposal) => Ok(HttpResponse::Ok().json(ProposalResponse::from(&proposal))),
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
        service::proposal_store::{ProposalFilter, ProposalIter, ProposalStoreError},
    };
    use crate::rest_api::{RestApiBuilder, RestApiServerError, RestApiShutdownHandle};

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request returns the expected proposal.
    fn test_fetch_proposal_ok() {
        let (_shutdown_handle, _join_handle, bind_url) =
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
            to_value(ProposalResponse::from(&get_proposal()))
                .expect("failed to convert expected data")
        );
    }

    #[test]
    /// Tests a GET /admin/proposals/{circuit_id} request returns NotFound when an invalid
    /// circuit_id is passed.
    fn test_fetch_proposal_not_found() {
        let (_shutdown_handle, _join_handle, bind_url) =
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
    }

    #[derive(Clone)]
    struct MockProposalStore;

    impl ProposalStore for MockProposalStore {
        fn proposals(
            &self,
            _filters: Vec<ProposalFilter>,
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
                comments: "mock circuit".into(),
            },
            votes: vec![],
            requester: vec![],
            requester_node_id: "node_id".into(),
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
