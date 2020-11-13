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

use actix_web::{client::Client, error, http::StatusCode, web, Error, HttpResponse};
use gameroom_database::{
    helpers,
    models::{Gameroom, GameroomMember as DbGameroomMember},
    ConnectionPool,
};
use openssl::hash::{hash, MessageDigest};
use protobuf::Message;
use splinter::admin::messages::{CreateCircuit, CreateCircuitBuilder, SplinterNode};
use splinter::circuit::template::CircuitCreateTemplate;
use splinter::protocol;
use splinter::protos::admin::{
    CircuitManagementPayload, CircuitManagementPayload_Action as Action,
    CircuitManagementPayload_Header as Header,
};

use crate::config::NodeInfo;
use crate::rest_api::{GameroomdData, RestApiResponseError};

use super::{
    get_response_paging_info, validate_limit, ErrorResponse, SuccessResponse, DEFAULT_LIMIT,
    DEFAULT_OFFSET,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGameroomForm {
    alias: String,
    members: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ApiGameroom {
    circuit_id: String,
    authorization_type: String,
    persistence: String,
    routes: String,
    circuit_management_type: String,
    members: Vec<ApiGameroomMember>,
    alias: String,
    status: String,
}

impl ApiGameroom {
    fn from(db_gameroom: Gameroom, db_members: Vec<DbGameroomMember>) -> Self {
        Self {
            circuit_id: db_gameroom.circuit_id.to_string(),
            authorization_type: db_gameroom.authorization_type.to_string(),
            persistence: db_gameroom.persistence.to_string(),
            routes: db_gameroom.routes.to_string(),
            circuit_management_type: db_gameroom.circuit_management_type.to_string(),
            members: db_members
                .into_iter()
                .map(ApiGameroomMember::from)
                .collect(),
            alias: db_gameroom.alias.to_string(),
            status: db_gameroom.status,
        }
    }
}

#[derive(Debug, Serialize)]
struct ApiGameroomMember {
    node_id: String,
    endpoints: Vec<String>,
}

impl ApiGameroomMember {
    fn from(db_circuit_member: DbGameroomMember) -> Self {
        ApiGameroomMember {
            node_id: db_circuit_member.node_id.to_string(),
            endpoints: db_circuit_member.endpoints,
        }
    }
}

pub async fn propose_gameroom(
    pool: web::Data<ConnectionPool>,
    create_gameroom: web::Json<CreateGameroomForm>,
    node_info: web::Data<NodeInfo>,
    client: web::Data<Client>,
    gameroomd_data: web::Data<GameroomdData>,
) -> HttpResponse {
    let mut template = match CircuitCreateTemplate::from_yaml_file("gameroom.yaml") {
        Ok(template) => template,
        Err(err) => {
            error!("Failed to load Gameroom template: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    let response = fetch_node_information(
        &create_gameroom.members,
        &gameroomd_data.splinterd_url,
        &gameroomd_data.authorization,
        client,
    )
    .await;

    let nodes = match response {
        Ok(nodes) => nodes,
        Err(err) => match err {
            RestApiResponseError::BadRequest(message) => {
                return HttpResponse::BadRequest().json(ErrorResponse::bad_request(&message));
            }
            _ => {
                debug!("Failed to fetch node information: {}", err);
                return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
            }
        },
    };

    let mut members = nodes
        .iter()
        .map(|node| SplinterNode {
            node_id: node.identity.to_string(),
            endpoints: node.endpoints.to_vec(),
        })
        .collect::<Vec<SplinterNode>>();

    members.push(SplinterNode {
        node_id: node_info.identity.to_string(),
        endpoints: node_info.endpoints.to_vec(),
    });

    let node_ids = nodes
        .iter()
        .map(|node| node.identity.to_string())
        .collect::<Vec<String>>()
        .join(",");
    let node_argument_value = format!("{},{}", node_info.identity, node_ids);

    let alias = match check_alias_uniqueness(pool, &create_gameroom.alias) {
        Ok(()) => &create_gameroom.alias,
        Err(err) => {
            return HttpResponse::BadRequest().json(ErrorResponse::bad_request(&err.to_string()));
        }
    };

    match template.set_argument_value("nodes", &node_argument_value) {
        Ok(template) => template,
        Err(err) => {
            error!("Failed to set 'nodes' arg value: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };
    match template.set_argument_value("signer_pub_key", &gameroomd_data.get_ref().public_key) {
        Ok(template) => template,
        Err(err) => {
            error!("Failed to set 'signer_pub_key' arg value: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    match template.set_argument_value("gameroom_name", alias) {
        Ok(template) => template,
        Err(err) => {
            error!("Failed to set 'gameroom_name' arg value: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    let mut create_circuit_builder = CreateCircuitBuilder::new();

    create_circuit_builder = match template.apply_to_builder(create_circuit_builder) {
        Ok(builder) => builder,
        Err(err) => {
            error!(
                "Unable to apply circuit template to CreateCircuitBuilder: {}",
                err
            );
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    let create_request = match create_circuit_builder.with_members(&members).build() {
        Ok(create_request) => create_request,
        Err(err) => {
            error!("Failed to build CreateCircuit: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    let payload_bytes = match make_payload(create_request, node_info.identity.to_string()) {
        Ok(bytes) => bytes,
        Err(err) => {
            debug!("Failed to make circuit management payload: {}", err);
            return HttpResponse::InternalServerError().json(ErrorResponse::internal_error());
        }
    };

    HttpResponse::Ok().json(SuccessResponse::new(json!({
        "payload_bytes": payload_bytes
    })))
}

async fn fetch_node_information(
    node_ids: &[String],
    splinterd_url: &str,
    authorization: &str,
    client: web::Data<Client>,
) -> Result<Vec<NodeResponse>, RestApiResponseError> {
    let node_ids = node_ids.to_owned();
    let mut response = client
        .get(&format!(
            "{}/registry/nodes?limit={}",
            splinterd_url,
            std::i64::MAX
        ))
        .header("Authorization", authorization)
        .header(
            "SplinterProtocolVersion",
            protocol::ADMIN_PROTOCOL_VERSION.to_string(),
        )
        .send()
        .await
        .map_err(|err| {
            RestApiResponseError::InternalError(format!("Failed to send request {}", err))
        })?;

    let body = response.body().await.map_err(|err| {
        RestApiResponseError::InternalError(format!("Failed to receive response body {}", err))
    })?;

    match response.status() {
        StatusCode::OK => {
            let list_reponse: SuccessResponse<Vec<NodeResponse>> = serde_json::from_slice(&body)
                .map_err(|err| {
                    RestApiResponseError::InternalError(format!(
                        "Failed to parse response body {}",
                        err
                    ))
                })?;
            let nodes = node_ids.into_iter().try_fold(vec![], |mut acc, node_id| {
                if let Some(node) = list_reponse
                    .data
                    .iter()
                    .find(|node| node.identity == node_id)
                {
                    acc.push(node.clone());
                    Ok(acc)
                } else {
                    Err(RestApiResponseError::BadRequest(format!(
                        "Could not find node with id {}",
                        node_id
                    )))
                }
            })?;

            Ok(nodes)
        }
        StatusCode::BAD_REQUEST => {
            let message: String = serde_json::from_slice(&body).map_err(|err| {
                RestApiResponseError::InternalError(format!(
                    "Failed to parse response body {}",
                    err
                ))
            })?;
            Err(RestApiResponseError::BadRequest(message))
        }
        _ => {
            let message: String = serde_json::from_slice(&body).map_err(|err| {
                RestApiResponseError::InternalError(format!(
                    "Failed to parse response body {}",
                    err
                ))
            })?;

            Err(RestApiResponseError::InternalError(message))
        }
    }
}

/// Represents a node as presented by the Splinter REST API.
#[derive(Clone, Deserialize, Serialize)]
struct NodeResponse {
    identity: String,
    endpoints: Vec<String>,
}

fn check_alias_uniqueness(
    pool: web::Data<ConnectionPool>,
    alias: &str,
) -> Result<(), RestApiResponseError> {
    if let Some(gameroom) = helpers::fetch_gameroom_by_alias(&*pool.get()?, alias)? {
        return Err(RestApiResponseError::BadRequest(format!(
            "Gameroom with alias {} already exists",
            gameroom.alias
        )));
    }
    Ok(())
}

fn make_payload(
    create_request: CreateCircuit,
    local_node: String,
) -> Result<Vec<u8>, RestApiResponseError> {
    let circuit_proto = create_request.into_proto()?;
    let circuit_bytes = circuit_proto.write_to_bytes()?;
    let hashed_bytes = hash(MessageDigest::sha512(), &circuit_bytes)?;

    let mut header = Header::new();
    header.set_action(Action::CIRCUIT_CREATE_REQUEST);
    header.set_payload_sha512(hashed_bytes.to_vec());
    header.set_requester_node_id(local_node);
    let header_bytes = header.write_to_bytes()?;

    let mut circuit_management_payload = CircuitManagementPayload::new();
    circuit_management_payload.set_header(header_bytes);
    circuit_management_payload.set_circuit_create_request(circuit_proto);
    let payload_bytes = circuit_management_payload.write_to_bytes()?;
    Ok(payload_bytes)
}

pub async fn list_gamerooms(
    pool: web::Data<ConnectionPool>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let mut base_link = "api/gamerooms?".to_string();
    let offset: usize = query
        .get("offset")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_OFFSET.to_string())
        .parse()
        .unwrap_or_else(|_| DEFAULT_OFFSET);

    let limit: usize = query
        .get("limit")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| DEFAULT_LIMIT.to_string())
        .parse()
        .unwrap_or_else(|_| DEFAULT_LIMIT);

    let status_optional = query.get("status").map(ToOwned::to_owned);

    if let Some(status) = status_optional.clone() {
        base_link.push_str(format!("status={}?", status).as_str());
    }

    match web::block(move || list_gamerooms_from_db(pool, status_optional, limit, offset)).await {
        Ok((gamerooms, query_count)) => {
            let paging_info =
                get_response_paging_info(limit, offset, "api/gamerooms?", query_count as usize);
            Ok(HttpResponse::Ok().json(SuccessResponse::list(gamerooms, paging_info)))
        }
        Err(err) => {
            debug!("Internal Server Error: {}", err);
            Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
        }
    }
}

fn list_gamerooms_from_db(
    pool: web::Data<ConnectionPool>,
    status_optional: Option<String>,
    limit: usize,
    offset: usize,
) -> Result<(Vec<ApiGameroom>, i64), RestApiResponseError> {
    let db_limit = validate_limit(limit);
    let db_offset = offset as i64;

    if let Some(status) = status_optional {
        let gamerooms = helpers::list_gamerooms_with_paging_and_status(
            &*pool.get()?,
            &status,
            db_limit,
            db_offset,
        )?
        .into_iter()
        .map(|gameroom| {
            let circuit_id = gameroom.circuit_id.to_string();
            let members = helpers::fetch_gameroom_members_by_circuit_id_and_status(
                &*pool.get()?,
                &circuit_id,
                &gameroom.status,
            )?;
            Ok(ApiGameroom::from(gameroom, members))
        })
        .collect::<Result<Vec<ApiGameroom>, RestApiResponseError>>()?;
        Ok((gamerooms, helpers::get_gameroom_count(&*pool.get()?)?))
    } else {
        let gamerooms = helpers::list_gamerooms_with_paging(&*pool.get()?, db_limit, db_offset)?
            .into_iter()
            .map(|gameroom| {
                let circuit_id = gameroom.circuit_id.to_string();
                let members = helpers::fetch_gameroom_members_by_circuit_id_and_status(
                    &*pool.get()?,
                    &circuit_id,
                    &gameroom.status,
                )?;
                Ok(ApiGameroom::from(gameroom, members))
            })
            .collect::<Result<Vec<ApiGameroom>, RestApiResponseError>>()?;
        Ok((gamerooms, helpers::get_gameroom_count(&*pool.get()?)?))
    }
}

pub async fn fetch_gameroom(
    pool: web::Data<ConnectionPool>,
    circuit_id: web::Path<String>,
) -> Result<HttpResponse, Error> {
    match web::block(move || fetch_gameroom_from_db(pool, &circuit_id)).await {
        Ok(gameroom) => Ok(HttpResponse::Ok().json(gameroom)),
        Err(err) => {
            match err {
                error::BlockingError::Error(err) => match err {
                    RestApiResponseError::NotFound(err) => {
                        Ok(HttpResponse::NotFound().json(ErrorResponse::not_found(&err)))
                    }
                    _ => Ok(HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request(&err.to_string()))),
                },
                error::BlockingError::Canceled => {
                    debug!("Internal Server Error: {}", err);
                    Ok(HttpResponse::InternalServerError().json(ErrorResponse::internal_error()))
                }
            }
        }
    }
}

fn fetch_gameroom_from_db(
    pool: web::Data<ConnectionPool>,
    circuit_id: &str,
) -> Result<ApiGameroom, RestApiResponseError> {
    if let Some(gameroom) = helpers::fetch_gameroom(&*pool.get()?, circuit_id)? {
        let members = helpers::fetch_gameroom_members_by_circuit_id_and_status(
            &*pool.get()?,
            &gameroom.circuit_id,
            &gameroom.status,
        )?;
        return Ok(ApiGameroom::from(gameroom, members));
    }
    Err(RestApiResponseError::NotFound(format!(
        "Gameroom with id {} not found",
        circuit_id
    )))
}
