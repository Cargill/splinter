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

//! Provides the `GET /admin/proposals` endpoint for listing circuit proposals.

use std::collections::HashMap;
use std::convert::TryFrom;

use actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use futures::{future::IntoFuture, Future};

#[cfg(feature = "authorization")]
use crate::admin::rest_api::CIRCUIT_READ_PERMISSION;
use crate::admin::service::proposal_store::ProposalStore;
use crate::admin::store::CircuitPredicate;
use crate::protocol;
use crate::rest_api::{
    actix_web_1::{get_paging_query, Method, ProtocolVersionRangeGuard, Resource},
    paging::{get_response_paging_info, PagingQuery},
    ErrorResponse,
};

use super::super::error::ProposalListError;
use super::super::resources;

pub fn make_list_proposals_resource<PS: ProposalStore + 'static>(proposal_store: PS) -> Resource {
    let resource =
        Resource::build("admin/proposals").add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_LIST_PROPOSALS_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ));

    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Get, CIRCUIT_READ_PERMISSION, move |r, _| {
            list_proposals(r, web::Data::new(proposal_store.clone()))
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |r, _| {
            list_proposals(r, web::Data::new(proposal_store.clone()))
        })
    }
}

fn list_proposals<PS: ProposalStore + 'static>(
    req: HttpRequest,
    proposal_store: web::Data<PS>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>> {
    let query: web::Query<HashMap<String, String>> =
        if let Ok(q) = web::Query::from_query(req.query_string()) {
            q
        } else {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(ErrorResponse::bad_request("Invalid query"))
                    .into_future(),
            );
        };

    let paging_query = match get_paging_query(&query) {
        Ok(paging_query) => paging_query,
        Err(err) => return err.into_future(),
    };

    let mut new_queries = vec![];
    let management_type_filter = query.get("management_type").map(|management_type| {
        new_queries.push(format!("management_type={}", management_type));
        management_type.to_string()
    });
    let member_filter = query.get("member").map(|member| {
        new_queries.push(format!("member={}", member));
        member.to_string()
    });

    let mut link = req.uri().path().to_string();
    if !new_queries.is_empty() {
        link.push_str(&format!("?{}&", new_queries.join("&")));
    }

    let protocol_version = match req.headers().get("SplinterProtocolVersion") {
        Some(header_value) => match header_value.to_str() {
            Ok(protocol_version) => protocol_version.to_string(),
            Err(_) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(
                            "Unable to get SplinterProtocolVersion",
                        ))
                        .into_future(),
                )
            }
        },
        None => format!("{}", protocol::ADMIN_PROTOCOL_VERSION),
    };

    Box::new(query_list_proposals(
        proposal_store,
        link,
        management_type_filter,
        member_filter,
        paging_query,
        protocol_version,
    ))
}

fn query_list_proposals<PS: ProposalStore + 'static>(
    proposal_store: web::Data<PS>,
    link: String,
    management_type_filter: Option<String>,
    member_filter: Option<String>,
    paging_query: PagingQuery,
    protocol_version: String,
) -> impl Future<Item = HttpResponse, Error = Error> {
    web::block(move || {
        let mut filters = vec![];
        if let Some(management_type) = management_type_filter {
            filters.push(CircuitPredicate::ManagementTypeEq(management_type));
        }
        if let Some(member) = member_filter {
            filters.push(CircuitPredicate::MembersInclude(vec![member]));
        }

        let proposals = proposal_store
            .proposals(filters)
            .map_err(|err| ProposalListError::InternalError(err.to_string()))?;
        let total = proposals.total() as usize;

        let proposals = proposals
            .skip(paging_query.offset)
            .take(paging_query.limit)
            .collect::<Vec<_>>();

        Ok((proposals, link, paging_query, total, protocol_version))
    })
    .then(|res| match res {
        Ok((proposals, link, paging_query, total_count, protocol_version)) => {
            match protocol_version.as_str() {
                "1" => Ok(HttpResponse::Ok().json(
                    resources::v1::proposals::ListProposalsResponse {
                        data: proposals
                            .iter()
                            .map(resources::v1::proposals::ProposalResponse::from)
                            .collect(),
                        paging: get_response_paging_info(paging_query, &link, total_count),
                    },
                )),
                // Handles 2 (and catch all)
                _ => {
                    let proposal_responses = match proposals
                        .iter()
                        .map(resources::v2::proposals::ProposalResponse::try_from)
                        .collect::<Result<
                            Vec<resources::v2::proposals::ProposalResponse>, &'static str>>()
                    {
                        Ok(proposal) => proposal,
                        Err(err) => {
                            error!("{}", err);
                            return Ok(HttpResponse::InternalServerError().into());
                        }
                    };
                    Ok(
                        HttpResponse::Ok().json(resources::v2::proposals::ListProposalsResponse {
                            data: proposal_responses,
                            paging: get_response_paging_info(paging_query, &link, total_count),
                        }),
                    )
                }
            }
        }
        Err(err) => match err {
            BlockingError::Error(err) => match err {
                ProposalListError::InternalError(_) => {
                    error!("{}", err);
                    Ok(HttpResponse::InternalServerError().into())
                }
            },
            _ => Ok(HttpResponse::InternalServerError().into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use reqwest::{blocking::Client, StatusCode, Url};
    use serde_json::{to_value, Value as JsonValue};

    use crate::admin::{
        messages::{
            AuthorizationType, CircuitProposal, CreateCircuit, DurabilityType, PersistenceType,
            ProposalType, RouteType, SplinterNode,
        },
        service::proposal_store::{ProposalIter, ProposalStoreError},
        store::{
            self, CircuitPredicate, CircuitProposal as StoreProposal, CircuitProposalBuilder,
            ProposedCircuitBuilder, ProposedNodeBuilder,
        },
    };
    use crate::rest_api::{
        actix_web_1::{RestApiBuilder, RestApiShutdownHandle},
        paging::Paging,
    };

    #[test]
    /// Tests a GET /admin/proposals request with no filters returns the expected proposals.
    fn test_list_proposals_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!("http://{}/admin/proposals", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v2::proposals::ProposalResponse::try_from(&CircuitProposal::from(
                    get_proposal_1()
                ))
                .expect("Unable to get ProposalResponse"),
                resources::v2::proposals::ProposalResponse::try_from(&CircuitProposal::from(
                    get_proposal_2()
                ))
                .expect("Unable to get ProposalResponse"),
                resources::v2::proposals::ProposalResponse::try_from(&CircuitProposal::from(
                    get_proposal_3()
                ))
                .expect("Unable to get ProposalResponse"),
            ])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                3,
                "/admin/proposals?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals request with protocol 1 and no filters returns the expected
    /// proposals. This test is for backwards compatibility.
    fn test_list_proposals_ok_v1() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!("http://{}/admin/proposals", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", "1");
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v1::proposals::ProposalResponse::from(&CircuitProposal::from(
                    get_proposal_1()
                )),
                resources::v1::proposals::ProposalResponse::from(&CircuitProposal::from(
                    get_proposal_2()
                )),
                resources::v1::proposals::ProposalResponse::from(&CircuitProposal::from(
                    get_proposal_3()
                )),
            ])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                3,
                "/admin/proposals?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals request with the `management_type` filter returns the expected
    /// proposal.
    fn test_list_proposals_with_management_type_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals?management_type=mgmt_type_1",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::proposals::ProposalResponse::try_from(
                &get_proposal_1()
            )
            .expect("Unable to get ProposalResponse")])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                1,
                &format!("/admin/proposals?management_type=mgmt_type_1&")
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals request with the `member` filter returns the expected
    /// proposals.
    fn test_list_proposals_with_member_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals?member=node_id",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v2::proposals::ProposalResponse::try_from(&get_proposal_1())
                    .expect("Unable to get ProposalResponse"),
                resources::v2::proposals::ProposalResponse::try_from(&get_proposal_3())
                    .expect("Unable to get ProposalResponse")
            ])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                2,
                &format!("/admin/proposals?member=node_id&")
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals request with both the `management_type` and `member` filters returns
    /// the expected proposal.
    fn test_list_proposals_with_management_type_and_member_ok() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!(
            "http://{}/admin/proposals?management_type=mgmt_type_2&member=node_id",
            bind_url
        ))
        .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::proposals::ProposalResponse::try_from(
                &get_proposal_3()
            )
            .expect("Unable to get ProposalResponse")])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                100,
                0,
                0,
                0,
                1,
                &format!("/admin/proposals?management_type=mgmt_type_2&member=node_id&")
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals?limit=1 request returns the expected proposal.
    fn test_list_proposal_with_limit() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!("http://{}/admin/proposals?limit=1", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![resources::v2::proposals::ProposalResponse::try_from(
                &get_proposal_1()
            )
            .expect("Unable to get ProposalResponse")])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                0,
                1,
                1,
                0,
                2,
                3,
                "/admin/proposals?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    #[test]
    /// Tests a GET /admin/proposals?offset=1 request returns the expected proposals.
    fn test_list_proposal_with_offset() {
        let (shutdown_handle, join_handle, bind_url) =
            run_rest_api_on_open_port(vec![make_list_proposals_resource(MockProposalStore)]);

        let url = Url::parse(&format!("http://{}/admin/proposals?offset=1", bind_url))
            .expect("Failed to parse URL");
        let req = Client::new()
            .get(url)
            .header("SplinterProtocolVersion", protocol::ADMIN_PROTOCOL_VERSION);
        let resp = req.send().expect("Failed to perform request");

        assert_eq!(resp.status(), StatusCode::OK);
        let proposals: JsonValue = resp.json().expect("Failed to deserialize body");

        assert_eq!(
            proposals.get("data").expect("no data field in response"),
            &to_value(vec![
                resources::v2::proposals::ProposalResponse::try_from(&get_proposal_2())
                    .expect("Unable to get ProposalResponse"),
                resources::v2::proposals::ProposalResponse::try_from(&get_proposal_3())
                    .expect("Unable to get ProposalResponse")
            ])
            .expect("failed to convert expected data"),
        );

        assert_eq!(
            proposals
                .get("paging")
                .expect("no paging field in response"),
            &to_value(create_test_paging_response(
                1,
                100,
                0,
                0,
                0,
                3,
                "/admin/proposals?"
            ))
            .expect("failed to convert expected paging")
        );

        shutdown_handle
            .shutdown()
            .expect("unable to shutdown rest api");
        join_handle.join().expect("Unable to join rest api thread");
    }

    fn create_test_paging_response(
        offset: usize,
        limit: usize,
        next_offset: usize,
        previous_offset: usize,
        last_offset: usize,
        total: usize,
        link: &str,
    ) -> Paging {
        let base_link = format!("{}limit={}&", link, limit);
        let current_link = format!("{}offset={}", base_link, offset);
        let first_link = format!("{}offset=0", base_link);
        let next_link = format!("{}offset={}", base_link, next_offset);
        let previous_link = format!("{}offset={}", base_link, previous_offset);
        let last_link = format!("{}offset={}", base_link, last_offset);

        Paging {
            current: current_link,
            offset,
            limit,
            total,
            first: first_link,
            prev: previous_link,
            next: next_link,
            last: last_link,
        }
    }

    #[derive(Clone)]
    struct MockProposalStore;

    impl ProposalStore for MockProposalStore {
        fn proposals(
            &self,
            filters: Vec<CircuitPredicate>,
        ) -> Result<ProposalIter, ProposalStoreError> {
            let mut proposals = get_proposal_list();

            proposals.retain(|proposal| {
                filters
                    .iter()
                    .all(|predicate| predicate.apply_to_proposals(proposal))
            });

            Ok(ProposalIter::new(Box::new(proposals.into_iter())))
        }

        fn proposal(
            &self,
            _circuit_id: &str,
        ) -> Result<Option<CircuitProposal>, ProposalStoreError> {
            unimplemented!()
        }
    }

    fn get_proposal_list() -> Vec<StoreProposal> {
        vec![
            CircuitProposalBuilder::new()
                .with_proposal_type(&store::ProposalType::Create)
                .with_circuit_id("abcDE-00000")
                .with_circuit_hash("012345")
                .with_circuit(
                    &ProposedCircuitBuilder::new()
                        .with_circuit_id("abcDE-00000")
                        .with_roster(&[])
                        .with_members(&[ProposedNodeBuilder::new()
                            .with_node_id("node_id")
                            .with_endpoints(&["".into()])
                            .build()
                            .expect("Unable to build circuit node")])
                        .with_authorization_type(&store::AuthorizationType::Trust)
                        .with_persistence(&store::PersistenceType::Any)
                        .with_durability(&store::DurabilityType::NoDurability)
                        .with_routes(&store::RouteType::Any)
                        .with_circuit_management_type("mgmt_type_1")
                        .with_comments("mock circuit 1")
                        .with_display_name("circuit_1")
                        .build()
                        .expect("Unable to create proposed circuit"),
                )
                .with_requester(&[])
                .with_requester_node_id("node_id")
                .build()
                .expect("unable to build proposal"),
            CircuitProposalBuilder::new()
                .with_proposal_type(&store::ProposalType::Create)
                .with_circuit_id("abcDE-00001")
                .with_circuit_hash("abcdef")
                .with_circuit(
                    &ProposedCircuitBuilder::new()
                        .with_circuit_id("abcDE-00001")
                        .with_roster(&[])
                        .with_members(&[])
                        .with_authorization_type(&store::AuthorizationType::Trust)
                        .with_persistence(&store::PersistenceType::Any)
                        .with_durability(&store::DurabilityType::NoDurability)
                        .with_routes(&store::RouteType::Any)
                        .with_circuit_management_type("mgmt_type_2")
                        .with_comments("mock circuit 2")
                        .with_display_name("circuit_2")
                        .with_circuit_version(2)
                        .build()
                        .expect("Unable to create proposed circuit"),
                )
                .with_requester(&[])
                .with_requester_node_id("node_id")
                .build()
                .expect("unable to build proposal"),
            CircuitProposalBuilder::new()
                .with_proposal_type(&store::ProposalType::Create)
                .with_circuit_id("abcDE-00002")
                .with_circuit_hash("678910")
                .with_circuit(
                    &ProposedCircuitBuilder::new()
                        .with_circuit_id("abcDE-00002")
                        .with_roster(&[])
                        .with_members(&[ProposedNodeBuilder::new()
                            .with_node_id("node_id")
                            .with_endpoints(&["".into()])
                            .build()
                            .expect("Unable to build circuit node")])
                        .with_authorization_type(&store::AuthorizationType::Trust)
                        .with_persistence(&store::PersistenceType::Any)
                        .with_durability(&store::DurabilityType::NoDurability)
                        .with_routes(&store::RouteType::Any)
                        .with_circuit_management_type("mgmt_type_2")
                        .with_comments("mock circuit 3")
                        .build()
                        .expect("Unable to create proposed circuit"),
                )
                .with_requester(&[])
                .with_requester_node_id("node_id")
                .build()
                .expect("unable to build proposal"),
        ]
    }

    fn get_proposal_1() -> CircuitProposal {
        CircuitProposal {
            proposal_type: ProposalType::Create,
            circuit_id: "abcDE-00000".into(),
            circuit_hash: "012345".into(),
            circuit: CreateCircuit {
                circuit_id: "abcDE-00000".into(),
                roster: vec![],
                members: vec![SplinterNode {
                    node_id: "node_id".into(),
                    endpoints: vec!["".into()],
                }],
                authorization_type: AuthorizationType::Trust,
                persistence: PersistenceType::Any,
                durability: DurabilityType::NoDurability,
                routes: RouteType::Any,
                circuit_management_type: "mgmt_type_1".into(),
                application_metadata: vec![],
                comments: Some("mock circuit 1".into()),
                display_name: Some("circuit_1".into()),
                circuit_version: 1,
            },
            votes: vec![],
            requester: vec![],
            requester_node_id: "node_id".into(),
        }
    }

    fn get_proposal_2() -> CircuitProposal {
        CircuitProposal {
            proposal_type: ProposalType::Create,
            circuit_id: "abcDE-00001".into(),
            circuit_hash: "abcdef".into(),
            circuit: CreateCircuit {
                circuit_id: "abcDE-00001".into(),
                roster: vec![],
                members: vec![],
                authorization_type: AuthorizationType::Trust,
                persistence: PersistenceType::Any,
                durability: DurabilityType::NoDurability,
                routes: RouteType::Any,
                circuit_management_type: "mgmt_type_2".into(),
                application_metadata: vec![],
                comments: Some("mock circuit 2".into()),
                display_name: Some("circuit_2".into()),
                circuit_version: 2,
            },
            votes: vec![],
            requester: vec![],
            requester_node_id: "node_id".into(),
        }
    }

    fn get_proposal_3() -> CircuitProposal {
        CircuitProposal {
            proposal_type: ProposalType::Create,
            circuit_id: "abcDE-00002".into(),
            circuit_hash: "678910".into(),
            circuit: CreateCircuit {
                circuit_id: "abcDE-00002".into(),
                roster: vec![],
                members: vec![SplinterNode {
                    node_id: "node_id".into(),
                    endpoints: vec!["".into()],
                }],
                authorization_type: AuthorizationType::Trust,
                persistence: PersistenceType::Any,
                durability: DurabilityType::NoDurability,
                routes: RouteType::Any,
                circuit_management_type: "mgmt_type_2".into(),
                application_metadata: vec![],
                comments: Some("mock circuit 3".into()),
                display_name: None,
                circuit_version: 1,
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
