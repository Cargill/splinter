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

use futures::StreamExt;
use transact::protocol::batch::BatchPair;
use transact::protos::FromBytes;

use crate::actix_web::{web, HttpResponse};
use crate::futures::executor::block_on;
use crate::protocol;
use crate::rest_api::{
    new_websocket_event_sender, EventSender, Method, ProtocolVersionRangeGuard, Request,
};
use crate::service::rest_api::ServiceEndpoint;

use super::error::StateSubscriberError;
use super::state::{StateChangeEvent, StateSubscriber};
use super::{Scabbard, SERVICE_TYPE};

const DEFAULT_BATCH_STATUS_WAIT_SECS: u64 = 300;

struct WsStateSubscriber {
    sender: EventSender<StateChangeEvent>,
}

impl StateSubscriber for WsStateSubscriber {
    fn handle_event(&self, event: StateChangeEvent) -> Result<(), StateSubscriberError> {
        self.sender.send(event).map_err(|_| {
            debug!(
                "Dropping scabbard state change event and unsubscribing due to websocket being
                 closed"
            );
            StateSubscriberError::Unsubscribe
        })
    }
}

pub fn make_subscribe_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/ws/subscribe".into(),
        method: Method::Get,
        handler: Arc::new(move |request, payload, service| {
            let scabbard = match service.as_any().downcast_ref::<Scabbard>() {
                Some(s) => s,
                None => {
                    error!("Failed to downcast to scabbard service");
                    return Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })));
                }
            };

            let mut query =
                match web::Query::<HashMap<String, String>>::from_query(request.query_string()) {
                    Ok(query) => query,
                    Err(_) => {
                        return Ok(HttpResponse::BadRequest().json(json!({
                            "message": "Invalid query"
                        })))
                    }
                };

            let last_seen_event_id = query.remove("last_seen_event");

            match last_seen_event_id {
                Some(ref id) if id.trim().is_empty() => {
                    return Ok(HttpResponse::BadRequest().json(json!({
                        "message": "last_seen_event must not be empty",
                    })));
                }
                Some(ref id) => debug!("Getting all state-delta events since {}", id),
                None => debug!("Getting all state-delta events"),
            }

            let unseen_events = match scabbard.get_events_since(last_seen_event_id) {
                Ok(events) => events,
                Err(err) => {
                    error!("Unable to load unseen scabbard events: {}", err);
                    return Ok(HttpResponse::InternalServerError()
                        .json(json!({ "message": "An internal error occurred" })));
                }
            };

            let request = Request::from((request, payload));
            match new_websocket_event_sender(request, Box::new(unseen_events)) {
                Ok((sender, res)) => {
                    if let Err(err) =
                        scabbard.add_state_subscriber(Box::new(WsStateSubscriber { sender }))
                    {
                        error!("Unable to add scabbard event sender: {}", err);
                        return Ok(HttpResponse::InternalServerError()
                            .json(json!({ "message": "An internal error occurred" })));
                    }
                    Ok(res.into())
                }
                Err(err) => {
                    error!("Failed to create websocket: {:?}", err);
                    Ok(HttpResponse::InternalServerError()
                        .json(json!({ "message": "An internal error occurred" })))
                }
            }
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_SUBSCRIBE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}

pub fn make_add_batches_to_queue_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/batches".into(),
        method: Method::Post,
        handler: Arc::new(move |_, mut payload, service| {
            let scabbard = match service.as_any().downcast_ref::<Scabbard>() {
                Some(s) => s,
                None => {
                    error!("Failed to downcast to scabbard service");
                    return Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })));
                }
            }
            .clone();
            let bytes = block_on(async {
                let mut bytes = web::BytesMut::new();
                while let Some(body) = payload.next().await {
                    bytes.extend_from_slice(&body.unwrap());
                }
                bytes
            });
            let batches: Vec<BatchPair> = match Vec::from_bytes(&bytes) {
                Ok(b) => b,
                Err(_) => {
                    return Ok(HttpResponse::BadRequest().json(json!({
                        "message": "invalid body: not a valid list of batches"
                    })))
                }
            };

            match scabbard.add_batches(batches) {
                Ok(Some(link)) => Ok(HttpResponse::Accepted().json(link)),
                Ok(None) => Ok(HttpResponse::BadRequest().json(json!({
                    "message": "no valid batches provided"
                }))),
                Err(err) => {
                    error!("Failed to add batches: {}", err);
                    Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })))
                }
            }
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_ADD_BATCHES_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}

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
                    return Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })));
                }
            }
            .clone();
            let query: web::Query<HashMap<String, String>> =
                if let Ok(q) = web::Query::from_query(req.query_string()) {
                    q
                } else {
                    return Ok(HttpResponse::BadRequest().json(json!({
                        "message": "Invalid query"
                    })));
                };

            let ids = if let Some(ids) = query.get("ids") {
                ids.split(',').map(String::from).collect()
            } else {
                return Ok(HttpResponse::BadRequest().json(json!({
                    "message": "No batch IDs specified"
                })));
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
                    return Ok(HttpResponse::InternalServerError()
                        .json(json!({ "message": "An internal error occurred" })));
                }
            };

            match batch_info_iter.collect::<Result<Vec<_>, _>>() {
                Ok(batch_infos) => Ok(HttpResponse::Ok().json(batch_infos)),
                Err(err) => Ok(HttpResponse::RequestTimeout().json(json!({
                    "message": format!("Failed to get batch statuses before timeout: {}", err)
                }))),
            }
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_BATCH_STATUSES_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}

#[cfg(feature = "scabbard-get-state")]
pub fn make_get_state_at_address_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/state/{address}".into(),
        method: Method::Get,
        handler: Arc::new(move |request, _, service| {
            let scabbard = match service.as_any().downcast_ref::<Scabbard>() {
                Some(s) => s,
                None => {
                    error!("Failed to downcast to scabbard service");
                    return Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })));
                }
            };

            let address = request
                .match_info()
                .get("address")
                .expect("address should not be none");

            Ok(match scabbard.get_state_at_address(address) {
                Ok(Some(value)) => HttpResponse::Ok().json(value),
                Ok(None) => HttpResponse::NotFound().json(json!({
                    "message": "address not set"
                })),
                Err(err) => {
                    error!("Failed to get state at adddress: {}", err);
                    HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    }))
                }
            })
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_GET_STATE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}

#[cfg(feature = "scabbard-get-state")]
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
                    return Ok(HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    })));
                }
            };

            let query: web::Query<HashMap<String, String>> =
                if let Ok(q) = web::Query::from_query(request.query_string()) {
                    q
                } else {
                    return Ok(HttpResponse::BadRequest().json(json!({
                        "message": "Invalid query"
                    })));
                };

            let prefix = query.get("prefix").map(String::as_str);

            Ok(match scabbard.get_state_with_prefix(prefix) {
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
                        Ok(entries) => HttpResponse::Ok().json(entries),
                        Err(err) => {
                            error!("Failed to convert state iterator: {}", err);
                            HttpResponse::InternalServerError().json(json!({
                                "message": "An internal error occurred"
                            }))
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to get state with prefix: {}", err);
                    HttpResponse::InternalServerError().json(json!({
                        "message": "An internal error occurred"
                    }))
                }
            })
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_LIST_STATE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}
