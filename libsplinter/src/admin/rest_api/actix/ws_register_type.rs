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

use actix_web::{web, HttpResponse};
use futures::IntoFuture;
use std::collections::HashMap;
#[cfg(feature = "admin-service-event-store")]
use std::convert::TryInto;
use std::time;

use crate::admin::messages::AdminServiceEvent;
use crate::admin::service::{
    AdminCommands, AdminServiceEventSubscriber, AdminServiceStatus, AdminSubscriberError,
};
use crate::protocol;
use crate::rest_api::{
    new_websocket_event_sender, EventSender, Method, ProtocolVersionRangeGuard, Request, Resource,
};

pub fn make_application_handler_registration_route<A: AdminCommands + Clone + 'static>(
    admin_commands: A,
) -> Resource {
    Resource::build("/ws/admin/register/{type}")
        .add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_APPLICATION_REGISTRATION_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ))
        .add_method(Method::Get, move |request, payload| {
            let status = if let Ok(status) = admin_commands.admin_service_status() {
                status
            } else {
                return Box::new(HttpResponse::InternalServerError().finish().into_future());
            };

            if status != AdminServiceStatus::Running {
                warn!("Admin service is not running");
                return Box::new(HttpResponse::ServiceUnavailable().finish().into_future());
            }
            let circuit_management_type = if let Some(t) = request.match_info().get("type") {
                t.to_string()
            } else {
                return Box::new(HttpResponse::BadRequest().finish().into_future());
            };
            debug!(
                "Beginning application authorization handler registration for \"{}\"",
                circuit_management_type
            );

            let mut query =
                match web::Query::<HashMap<String, u64>>::from_query(request.query_string()) {
                    Ok(query) => query,
                    Err(_) => return Box::new(HttpResponse::BadRequest().finish().into_future()),
                };

            #[cfg(not(feature = "admin-service-event-store"))]
            let initial_events = {
                let (skip, last_seen_timestamp) = query
                    .remove("last")
                    .map(|since_millis| {
                        // Since this is the last seen event, we will skip it in our since
                        // query
                        debug!("Catching up on events since {}", since_millis);
                        (
                            1usize,
                            time::SystemTime::UNIX_EPOCH
                                + time::Duration::from_millis(since_millis),
                        )
                    })
                    .unwrap_or((0, time::SystemTime::UNIX_EPOCH));

                match admin_commands
                    .get_events_since(&last_seen_timestamp, &circuit_management_type)
                {
                    Ok(events) => events.map(JsonAdminEvent::from).skip(skip),
                    Err(err) => {
                        error!(
                            "Unable to load initial set of admin events for {}: {}",
                            &circuit_management_type, err
                        );
                        return Box::new(
                            HttpResponse::InternalServerError().finish().into_future(),
                        );
                    }
                }
            };

            #[cfg(feature = "admin-service-event-store")]
            let initial_events = {
                let (skip, last_seen_event_id) = query
                    .remove("last")
                    .map(|since_evt_id| {
                        // Since this is the last seen event, we will skip it in our since
                        // query
                        let id: i64 = since_evt_id.try_into().unwrap_or(0);
                        debug!("Catching up on events since {}", id);
                        (1usize, id)
                    })
                    .unwrap_or((0, 0));

                match admin_commands.get_events_since(&last_seen_event_id, &circuit_management_type)
                {
                    Ok(events) => events.map(JsonAdminEvent::from).skip(skip),
                    Err(err) => {
                        error!(
                            "Unable to load initial set of admin events for {}: {}",
                            &circuit_management_type, err
                        );
                        return Box::new(
                            HttpResponse::InternalServerError().finish().into_future(),
                        );
                    }
                }
            };

            let request = Request::from((request, payload));
            match new_websocket_event_sender(request, Box::new(initial_events)) {
                Ok((sender, res)) => {
                    if let Err(err) = admin_commands.add_event_subscriber(
                        &circuit_management_type,
                        Box::new(WsAdminServiceEventSubscriber { sender }),
                    ) {
                        error!("Unable to add admin event subscriber: {}", err);
                        return Box::new(
                            HttpResponse::InternalServerError().finish().into_future(),
                        );
                    }
                    debug!("Websocket response: {:?}", res);
                    Box::new(res.into_future())
                }
                Err(err) => {
                    debug!("Failed to create websocket: {:?}", err);
                    Box::new(HttpResponse::InternalServerError().finish().into_future())
                }
            }
        })
}

struct WsAdminServiceEventSubscriber {
    sender: EventSender<JsonAdminEvent>,
}

impl AdminServiceEventSubscriber for WsAdminServiceEventSubscriber {
    #[cfg(not(feature = "admin-service-event-store"))]
    fn handle_event(
        &self,
        event: &AdminServiceEvent,
        timestamp: &time::SystemTime,
    ) -> Result<(), AdminSubscriberError> {
        let json_event = JsonAdminEvent {
            timestamp: *timestamp,
            event: event.clone(),
            event_id: None,
        };
        self.sender.send(json_event).map_err(|_| {
            debug!("Dropping admin service event and unsubscribing due to websocket being closed");
            AdminSubscriberError::Unsubscribe
        })
    }

    #[cfg(feature = "admin-service-event-store")]
    fn handle_event(
        &self,
        event: &AdminServiceEvent,
        event_id: &i64,
    ) -> Result<(), AdminSubscriberError> {
        let json_event = JsonAdminEvent {
            timestamp: time::SystemTime::now(),
            event: event.clone(),
            event_id: Some(*event_id),
        };
        self.sender.send(json_event).map_err(|_| {
            debug!("Dropping admin service event and unsubscribing due to websocket being closed");
            AdminSubscriberError::Unsubscribe
        })
    }
}

#[derive(Debug, Serialize, Clone)]
struct JsonAdminEvent {
    #[serde(serialize_with = "st_as_millis")]
    timestamp: time::SystemTime,

    #[serde(flatten)]
    event: AdminServiceEvent,

    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<i64>,
}

// Conversion for the `JsonAdminEvent` sent to subscribers when the `Mailbox` is used to store
// admin events.
#[cfg(not(feature = "admin-service-event-store"))]
impl From<(time::SystemTime, AdminServiceEvent)> for JsonAdminEvent {
    fn from(raw_evt: (time::SystemTime, AdminServiceEvent)) -> Self {
        Self {
            timestamp: raw_evt.0,
            event: raw_evt.1,
            event_id: None,
        }
    }
}

// Conversion for the `JsonAdminEvent` sent to subscribers when the `AdminServiceEventStore` is
// used to store admin events.
// `timestamp` is set to the `UNIX_EPOCH` value to allow for backward-compatibility, as the
// `timestamp` is not used by the `AdminServiceEventStore`.
#[cfg(feature = "admin-service-event-store")]
impl From<(i64, AdminServiceEvent)> for JsonAdminEvent {
    fn from(raw_evt: (i64, AdminServiceEvent)) -> Self {
        Self {
            timestamp: time::SystemTime::now(),
            event: raw_evt.1,
            event_id: Some(raw_evt.0),
        }
    }
}

fn st_as_millis<S>(data: &time::SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let since_the_epoch = data
        .duration_since(time::UNIX_EPOCH)
        .expect("Time went backwards");

    serializer.serialize_u128(since_the_epoch.as_millis())
}
