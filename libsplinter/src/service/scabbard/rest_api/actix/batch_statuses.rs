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
use std::sync::Arc;
use std::time::Duration;

use crate::actix_web::{web, HttpResponse};
use crate::futures::IntoFuture;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard};
use crate::service::rest_api::ServiceEndpoint;
use crate::service::scabbard::{Scabbard, SERVICE_TYPE};

const DEFAULT_BATCH_STATUS_WAIT_SECS: u64 = 300;

pub fn make_get_batch_status_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/batch_statuses".into(),
        method: Method::Get,
        handler: Arc::new(move |req, _, service| {
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
            }
            .clone();
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

            let ids = if let Some(ids) = query.get("ids") {
                ids.split(',').map(String::from).collect()
            } else {
                return Box::new(
                    HttpResponse::BadRequest()
                        .json(ErrorResponse::bad_request("No batch IDs specified"))
                        .into_future(),
                );
            };

            let wait = query
                .get("wait")
                .and_then(|wait_str| {
                    if wait_str.as_str() == "false" {
                        None
                    } else {
                        wait_str
                            .parse()
                            .ok()
                            .or(Some(DEFAULT_BATCH_STATUS_WAIT_SECS))
                    }
                })
                .map(Duration::from_secs);

            let batch_info_iter = match scabbard.get_batch_info(ids, wait) {
                Ok(iter) => iter,
                Err(err) => {
                    error!("Failed to get batch statuses iterator: {}", err);
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    );
                }
            };

            match batch_info_iter.collect::<Result<Vec<_>, _>>() {
                Ok(batch_infos) => Box::new(HttpResponse::Ok().json(batch_infos).into_future()),
                Err(err) => Box::new(
                    HttpResponse::RequestTimeout()
                        .json(ErrorResponse::request_timeout(&format!(
                            "Failed to get batch statuses before timeout: {}",
                            err
                        )))
                        .into_future(),
                ),
            }
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_BATCH_STATUSES_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}
