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

use std::collections::HashMap;

use crate::actix_web::{error::BlockingError, web, Error, HttpRequest, HttpResponse};
use crate::futures::{future::IntoFuture, stream::Stream, Future};
use crate::node_registry::{
    error::{InvalidNodeError, NodeRegistryError},
    rest_api::resources::nodes::ListNodesResponse,
    MetadataPredicate, Node, NodeRegistryReader, NodeRegistryWriter,
};
use crate::protocol;
use crate::rest_api::{
    paging::{get_response_paging_info, DEFAULT_LIMIT, DEFAULT_OFFSET},
    percent_encode_filter_query, Method, ProtocolVersionRangeGuard, Resource,
};

type Filter = HashMap<String, (String, String)>;

pub fn make_nodes_resource<N>(registry: N) -> Resource
where
    N: NodeRegistryReader + NodeRegistryWriter + Clone + 'static,
{
    let registry1 = registry.clone();
    Resource::build("/admin/nodes")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_LIST_NODES_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |r, _| {
            list_nodes(r, web::Data::new(registry.clone()))
        })
        .add_method(Method::Post, move |_, p| {
            add_node(p, web::Data::new(registry1.clone()))
        })
}

fn list_nodes<NR>(
    req: HttpRequest,
    registry: web::Data<NR>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>>
where
    NR: NodeRegistryReader + 'static,
{
    let query: web::Query<HashMap<String, String>> =
        if let Ok(q) = web::Query::from_query(req.query_string()) {
            q
        } else {
            return Box::new(
                HttpResponse::BadRequest()
                    .json(json!({
                        "message": "Invalid query"
                    }))
                    .into_future(),
            );
        };

    let offset = match query.get("offset") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(format!(
                            "Invalid offset value passed: {}. Error: {}",
                            value, err
                        ))
                        .into_future(),
                )
            }
        },
        None => DEFAULT_OFFSET,
    };

    let limit = match query.get("limit") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(format!(
                            "Invalid limit value passed: {}. Error: {}",
                            value, err
                        ))
                        .into_future(),
                )
            }
        },
        None => DEFAULT_LIMIT,
    };

    let mut link = format!("{}?", req.uri().path());

    let filters = match query.get("filter") {
        Some(value) => match serde_json::from_str(value) {
            Ok(val) => {
                link.push_str(&format!("filter={}&", percent_encode_filter_query(value)));
                Some(val)
            }
            Err(err) => {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(format!(
                            "Invalid filter value passed: {}. Error: {}",
                            value, err
                        ))
                        .into_future(),
                )
            }
        },
        None => None,
    };

    let predicates = match to_predicates(filters) {
        Ok(predicates) => predicates,
        Err(err) => return Box::new(HttpResponse::BadRequest().json(err).into_future()),
    };

    Box::new(query_list_nodes(
        registry,
        link,
        predicates,
        Some(offset),
        Some(limit),
    ))
}

fn query_list_nodes<NR>(
    registry: web::Data<NR>,
    link: String,
    filters: Vec<MetadataPredicate>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> impl Future<Item = HttpResponse, Error = Error>
where
    NR: NodeRegistryReader + 'static,
{
    let count_filters = filters.clone();
    web::block(move || match registry.count_nodes(&count_filters) {
        Ok(count) => Ok((registry, count)),
        Err(err) => Err(err),
    })
    .and_then(move |(registry, total_count)| {
        web::block(move || match registry.list_nodes(&filters) {
            Ok(nodes_iter) => Ok(ListNodesResponse {
                data: nodes_iter
                    .skip(offset.as_ref().copied().unwrap_or(0))
                    .take(limit.as_ref().copied().unwrap_or(std::usize::MAX))
                    .collect::<Vec<_>>(),
                paging: get_response_paging_info(limit, offset, &link, total_count as usize),
            }),
            Err(err) => Err(err),
        })
    })
    .then(|res| match res {
        Ok(list_res) => Ok(HttpResponse::Ok().json(list_res)),
        Err(err) => {
            error!("Unable to list nodes: {}", err);
            Ok(HttpResponse::InternalServerError().into())
        }
    })
}

fn to_predicates(filters: Option<Filter>) -> Result<Vec<MetadataPredicate>, String> {
    match filters {
        Some(filters) => filters
            .into_iter()
            .map(|(key, (operator, value))| match operator.as_str() {
                "=" => Ok(MetadataPredicate::Eq(key, value)),
                ">" => Ok(MetadataPredicate::Gt(key, value)),
                "<" => Ok(MetadataPredicate::Lt(key, value)),
                ">=" => Ok(MetadataPredicate::Ge(key, value)),
                "<=" => Ok(MetadataPredicate::Le(key, value)),
                "!=" => Ok(MetadataPredicate::Ne(key, value)),
                _ => Err(format!("{} is not a valid operator", operator)),
            })
            .collect(),
        None => Ok(vec![]),
    }
}

fn add_node<NW>(
    payload: web::Payload,
    registry: web::Data<NW>,
) -> Box<dyn Future<Item = HttpResponse, Error = Error>>
where
    NW: NodeRegistryReader + NodeRegistryWriter + 'static,
{
    Box::new(
        payload
            .from_err::<Error>()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, Error>(body)
            })
            .into_future()
            .and_then(move |body| match serde_json::from_slice::<Node>(&body) {
                Ok(node) => Box::new(
                    web::block(move || {
                        if registry.has_node(&node.identity)? {
                            Err(NodeRegistryError::InvalidNode(
                                InvalidNodeError::DuplicateIdentity(node.identity),
                            ))
                        } else {
                            registry.insert_node(node)
                        }
                    })
                    .then(|res| {
                        Ok(match res {
                            Ok(_) => HttpResponse::Ok().finish(),
                            Err(err) => match err {
                                BlockingError::Error(err) => match err {
                                    NodeRegistryError::InvalidNode(err) => {
                                        HttpResponse::Forbidden()
                                            .json(format!("node is invalid: {}", err))
                                    }
                                    _ => {
                                        HttpResponse::InternalServerError().json(format!("{}", err))
                                    }
                                },
                                _ => HttpResponse::InternalServerError().json(format!("{}", err)),
                            },
                        })
                    }),
                )
                    as Box<dyn Future<Item = HttpResponse, Error = Error>>,
                Err(err) => Box::new(
                    HttpResponse::BadRequest()
                        .json(format!("invalid node: {}", err))
                        .into_future(),
                ),
            }),
    )
}
