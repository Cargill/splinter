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
use std::convert::TryFrom;
use std::sync::{Arc,Mutex};

use super::resources::{v1, v2};

use actix_web_4::body::BoxBody;
use actix_web_4::web::Query;
use actix_web_4::{HttpRequest, HttpResponse, Responder};

use crate::rest_api::actix_web_4::protocol_version::{
    ProtocolVersion, MAX_PROTOCOL_VERSION, MIN_PROTOCOL_VERSION,
};
use crate::rest_api::paging::{DEFAULT_LIMIT, DEFAULT_OFFSET};
use crate::rest_error::RESTError;
use crate::store::StoreFactory;

pub async fn get_admin_circuits(request: HttpRequest) -> Result<HttpResponse<BoxBody>, RESTError> {
    match ProtocolVersion::try_from(&request) {
        Ok(system_version) => match system_version.into() {
            MIN_PROTOCOL_VERSION..=1 => Ok(v1::get_admin_circuits(v1::Arguments::try_from(
                &request,
            )?)?
            .respond_to(&request)),
            2..=MAX_PROTOCOL_VERSION => Ok(v2::get_admin_circuits(v2::Arguments::try_from(
                &request,
            )?)?
            .respond_to(&request)),
            // this should be unreachable as ProtocolVersion does the check
            _ => Err(RESTError::bad_request(
                "Protocol version does not have a mapped resource version",
            )),
        },
        Err(_) => Ok(HttpResponse::Ok().body("Could not get resource")),
    }
}

impl TryFrom<&HttpRequest> for v1::Arguments {
    type Error = RESTError;
    fn try_from(value: &HttpRequest) -> Result<Self, Self::Error> {
        let store = value
            .app_data::<Arc<Mutex<Box<dyn StoreFactory + Send>>>>()
            .ok_or_else(|| {
                RESTError::internal_error("Could not get StoreFactory from application", None)
            })?
            .lock().unwrap()
            .get_admin_service_store();
        let query = Query::<HashMap<String, String>>::from_query(value.query_string()).unwrap();
        let limit = query
            .get("limit")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|e| RESTError::bad_request(format!("Could not parse limit query: {}", e)))?
            .unwrap_or(DEFAULT_LIMIT);
        let offset = query
            .get("offset")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|e| RESTError::bad_request(format!("Could not parse offset query: {}", e)))?
            .unwrap_or(DEFAULT_OFFSET);
        let status = query.get("status").map(ToString::to_string);
        let member = query.get("member").map(ToString::to_string);
        let link = value.uri().path().to_string();
        Ok(Self {
            store,
            limit,
            offset,
            member,
            link,
            status,
        })
    }
}

impl Responder for v1::Response {
    type Body = BoxBody;
    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}

impl TryFrom<&HttpRequest> for v2::Arguments {
    type Error = RESTError;
    fn try_from(value: &HttpRequest) -> Result<Self, Self::Error> {
        let store = value
            .app_data::<Arc<Mutex<Box<dyn StoreFactory + Send>>>>()
            .ok_or_else(|| {
                RESTError::internal_error("Could not get StoreFactory from application", None)
            })?
            .lock().unwrap()
            .get_admin_service_store();
        let query = Query::<HashMap<String, String>>::from_query(value.query_string()).unwrap();
        let limit = query
            .get("limit")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|e| RESTError::bad_request(format!("Could not parse limit query: {}", e)))?
            .unwrap_or(DEFAULT_LIMIT);
        let offset = query
            .get("offset")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|e| RESTError::bad_request(format!("Could not parse offset query: {}", e)))?
            .unwrap_or(DEFAULT_OFFSET);
        let status = query.get("status").map(ToString::to_string);
        let member = query.get("member").map(ToString::to_string);
        let link = value.uri().path().to_string();
        Ok(Self {
            store,
            limit,
            offset,
            member,
            link,
            status,
        })
    }
}

impl Responder for v2::Response {
    type Body = BoxBody;
    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::Ok().json(self)
    }
}
