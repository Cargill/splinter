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

use crate::actix_web::{web, HttpResponse};
use crate::futures::IntoFuture;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard};
use crate::service::rest_api::ServiceEndpoint;
use crate::service::scabbard::{Scabbard, SERVICE_TYPE};

pub fn make_get_state_with_prefix_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/state".into(),
        method: Method::Get,
        handler: Arc::new(move |request, _, service| {
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

            let query: web::Query<HashMap<String, String>> =
                if let Ok(q) = web::Query::from_query(request.query_string()) {
                    q
                } else {
                    return Box::new(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request("Invalid query"))
                            .into_future(),
                    );
                };

            let prefix = query.get("prefix").map(String::as_str);

            Box::new(match scabbard.get_state_with_prefix(prefix) {
                Ok(state_iter) => {
                    let res = state_iter
                        .map(|res| {
                            res.map(|(address, value)| {
                                json!({
                                    "address": address,
                                    "value": value,
                                })
                            })
                        })
                        .collect::<Result<Vec<_>, _>>();
                    match res {
                        Ok(entries) => HttpResponse::Ok().json(entries).into_future(),
                        Err(err) => {
                            error!("Failed to convert state iterator: {}", err);
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future()
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to get state with prefix: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_LIST_STATE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}
