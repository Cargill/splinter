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
use std::sync::Arc;

use actix_web::{web, HttpResponse};
use futures::IntoFuture;

use scabbard::protocol;
use scabbard::service::{
    Scabbard, StateChangeEvent, StateSubscriber, StateSubscriberError, SERVICE_TYPE,
};
use splinter_rest_api_common::response_models::ErrorResponse;
#[cfg(feature = "authorization")]
use splinter_rest_api_common::scabbard::SCABBARD_READ_PERMISSION;

use crate::framework::{
    new_websocket_event_sender, EventSender, Method, ProtocolVersionRangeGuard, Request,
};
use crate::service::ServiceEndpoint;

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
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    );
                }
            };

            let mut query =
                match web::Query::<HashMap<String, String>>::from_query(request.query_string()) {
                    Ok(query) => query,
                    Err(_) => {
                        return Box::new(
                            HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("Invalid query"))
                                .into_future(),
                        )
                    }
                };

            let last_seen_event_id = query.remove("last_seen_event");

            match last_seen_event_id {
                Some(ref id) if id.trim().is_empty() => {
                    return Box::new(
                        HttpResponse::BadRequest()
                            .json(ErrorResponse::bad_request(
                                "last_seen_event must not be empty",
                            ))
                            .into_future(),
                    );
                }
                Some(ref id) => debug!("Getting all state-delta events since {}", id),
                None => debug!("Getting all state-delta events"),
            }

            let unseen_events = match scabbard.get_events_since(last_seen_event_id) {
                Ok(events) => events,
                Err(err) => {
                    error!("Unable to load unseen scabbard events: {}", err);
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    );
                }
            };

            let request = Request::from((request, payload));
            match new_websocket_event_sender(request, Box::new(unseen_events)) {
                Ok((sender, res)) => {
                    if let Err(err) =
                        scabbard.add_state_subscriber(Box::new(WsStateSubscriber { sender }))
                    {
                        error!("Unable to add scabbard event sender: {}", err);
                        return Box::new(
                            HttpResponse::InternalServerError()
                                .json(ErrorResponse::internal_error())
                                .into_future(),
                        );
                    }
                    Box::new(res.into_future())
                }
                Err(err) => {
                    error!("Failed to create websocket: {:?}", err);
                    Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    )
                }
            }
        }),
        request_guards: vec![Arc::new(ProtocolVersionRangeGuard::new(
            splinter_rest_api_common::scabbard::SCABBARD_SUBSCRIBE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
        #[cfg(feature = "authorization")]
        permission: SCABBARD_READ_PERMISSION,
    }
}
