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

//! Routes for key registry operations

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use serde::Serializer;

use crate::actix_web::{error::Error as ActixError, web, HttpRequest, HttpResponse};
use crate::futures::executor::block_on;
use crate::protocol;
use crate::rest_api::{
    paging::{get_response_paging_info, Paging, DEFAULT_LIMIT, DEFAULT_OFFSET},
    Method, ProtocolVersionRangeGuard, Resource, RestResourceProvider,
};

use super::{KeyInfo, KeyRegistry, KeyRegistryError};

#[derive(Debug, Serialize, Clone, PartialEq)]
struct ListKeyInfoResponse {
    data: Vec<KeyInfoResponse>,
    paging: Paging,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
struct KeyInfoResponse {
    #[serde(serialize_with = "as_hex")]
    public_key: Vec<u8>,
    node_id: String,

    metadata: BTreeMap<String, String>,
}

impl KeyInfoResponse {
    fn new(key_info: &KeyInfo) -> Self {
        Self {
            public_key: key_info.public_key().to_vec(),
            node_id: key_info.associated_node_id().into(),
            metadata: key_info
                .metadata()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

pub struct KeyRegistryManager {
    key_registry: Box<dyn KeyRegistry>,
}

impl KeyRegistryManager {
    pub fn new(key_registry: Box<dyn KeyRegistry>) -> Self {
        Self { key_registry }
    }
}

impl RestResourceProvider for KeyRegistryManager {
    fn resources(&self) -> Vec<Resource> {
        vec![
            make_list_key_resources(self.key_registry.clone()),
            make_fetch_key_resource(self.key_registry.clone()),
        ]
    }
}

fn make_fetch_key_resource(key_registry: Box<dyn KeyRegistry>) -> Resource {
    Resource::build("/admin/keys/{public_key}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_FETCH_KEY_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |req, _| {
            block_on(make_fetch_key_method(req, key_registry.clone()))
        })
}

async fn make_fetch_key_method(
    req: HttpRequest,
    key_registry: Box<dyn KeyRegistry>,
) -> Result<HttpResponse, ActixError> {
    let public_key = match parse_hex(req.match_info().get("public_key").unwrap_or("")) {
        Ok(public_key) => public_key,
        Err(err_msg) => return Ok(HttpResponse::BadRequest().json(json!({ "message": err_msg }))),
    };

    let registry = web::Data::new(key_registry.clone());
    match web::block(move || registry.get_key(&public_key)).await {
        Ok(Some(key_info)) => {
            Ok(HttpResponse::Ok().json(json!({ "data": KeyInfoResponse::new(&key_info) })))
        }
        Ok(None) => Ok(HttpResponse::NotFound().into()),
        Err(err) => {
            error!("Unable to read key info: {}", err);
            Ok(HttpResponse::InternalServerError().into())
        }
    }
}

fn make_list_key_resources(key_registry: Box<dyn KeyRegistry>) -> Resource {
    Resource::build("/admin/keys")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_LIST_KEYS_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |req, _| {
            block_on(make_list_key_method(req, key_registry.clone()))
        })
}

async fn make_list_key_method(
    req: HttpRequest,
    key_registry: Box<dyn KeyRegistry>,
) -> Result<HttpResponse, ActixError> {
    let query: web::Query<HashMap<String, String>> =
        if let Ok(q) = web::Query::from_query(req.query_string()) {
            q
        } else {
            return Ok(HttpResponse::BadRequest().json(json!({
                "message": "Invalid query"
            })));
        };

    let offset = match query.get("offset") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::BadRequest().json(format!(
                    "Invalid offset value passed: {}. Error: {}",
                    value, err
                )))
            }
        },
        None => DEFAULT_OFFSET,
    };

    let limit = match query.get("limit") {
        Some(value) => match value.parse::<usize>() {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::BadRequest().json(format!(
                    "Invalid limit value passed: {}. Error: {}",
                    value, err
                )))
            }
        },
        None => DEFAULT_LIMIT,
    };

    let link = format!("{}?", req.uri().path());
    let registry = web::Data::new(key_registry.clone());

    match web::block(move || {
        let res: Result<(Vec<_>, usize), KeyRegistryError> = Ok((
            registry
                .keys()?
                .skip(offset)
                .take(limit)
                .map(|key_info| KeyInfoResponse::new(&key_info))
                .collect::<Vec<_>>(),
            registry.count()?,
        ));
        res
    })
    .await
    {
        Ok((data, total_count)) => Ok(HttpResponse::Ok().json(json!(ListKeyInfoResponse {
            data: data,
            paging: get_response_paging_info(Some(limit), Some(offset), &link, total_count)
        }))),
        Err(err) => {
            error!("unable to list key info: {}", err);
            Ok(HttpResponse::InternalServerError().into())
        }
    }
}

fn as_hex<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut buf = String::new();
    for b in data {
        write!(&mut buf, "{:02x}", b).expect("Unable to write to string");
    }

    serializer.serialize_str(&buf)
}

fn parse_hex(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err(format!("{} is not valid hex: odd number of digits", hex));
    }

    let mut res = vec![];
    for i in (0..hex.len()).step_by(2) {
        res.push(
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|_| format!("{} contains invalid hex", hex))?,
        );
    }

    Ok(res)
}
