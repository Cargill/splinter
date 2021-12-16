// Copyright 2018-2021 Cargill Incorporated
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
use std::convert::TryFrom;
use std::convert::TryInto;
use std::str::FromStr;
use std::time;

use crate::admin::messages::v1;
use crate::admin::messages::AdminServiceEvent;
#[cfg(feature = "authorization")]
use crate::admin::rest_api::CIRCUIT_READ_PERMISSION;
use crate::admin::service::{
    AdminCommands, AdminServiceEventSubscriber, AdminServiceStatus, AdminSubscriberError,
};
use crate::admin::store;
use crate::error::InvalidStateError;
use crate::rest_api::{
    actix_web_1::{
        new_websocket_event_sender, EventSender, Method, ProtocolVersionRangeGuard, Request,
        Resource,
    },
    ErrorResponse, SPLINTER_PROTOCOL_VERSION,
};

const ADMIN_APPLICATION_REGISTRATION_PROTOCOL_MIN: u32 = 1;

pub fn make_application_handler_registration_route<A: AdminCommands + Clone + 'static>(
    admin_commands: A,
) -> Resource {
    let resource = Resource::build("/ws/admin/register/{type}").add_request_guard(
        ProtocolVersionRangeGuard::new(
            ADMIN_APPLICATION_REGISTRATION_PROTOCOL_MIN,
            SPLINTER_PROTOCOL_VERSION,
        ),
    );

    #[cfg(feature = "authorization")]
    {
        resource.add_method(
            Method::Get,
            CIRCUIT_READ_PERMISSION,
            move |request, payload| {
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

                let protocol_version = match request.headers().get("SplinterProtocolVersion") {
                    Some(header_value) => match header_value.to_str() {
                        Ok(protocol_version) => match u32::from_str(protocol_version) {
                            Ok(protocol_version) => protocol_version,
                            Err(_) => {
                                return Box::new(
                                    HttpResponse::BadRequest()
                                        .json(ErrorResponse::bad_request(
                                            "Unable to parse SplinterProtocolVersion",
                                        ))
                                        .into_future(),
                                )
                            }
                        },
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
                    None => SPLINTER_PROTOCOL_VERSION,
                };

                debug!(
                    "Beginning application authorization handler registration for \"{}\" \
                        with protocol {}",
                    circuit_management_type, protocol_version
                );

                let mut query =
                    match web::Query::<HashMap<String, u64>>::from_query(request.query_string()) {
                        Ok(query) => query,
                        Err(_) => {
                            return Box::new(HttpResponse::BadRequest().finish().into_future())
                        }
                    };

                let initial_events: Vec<JsonAdminEvent> = {
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

                    match admin_commands
                        .get_events_since(&last_seen_event_id, &circuit_management_type)
                    {
                        Ok(events) => {
                            match events
                                .map(|event| JsonAdminEvent::new(&event, protocol_version))
                                .skip(skip)
                                .collect::<Result<Vec<JsonAdminEvent>, InvalidStateError>>()
                            {
                                Ok(events) => events,
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
                        }
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
                match new_websocket_event_sender(request, Box::new(initial_events.into_iter())) {
                    Ok((sender, res)) => {
                        if let Err(err) = admin_commands.add_event_subscriber(
                            &circuit_management_type,
                            Box::new(WsAdminServiceEventSubscriber {
                                sender,
                                protocol_version,
                            }),
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
            },
        )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, move |request, payload| {
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

            let protocol_version = match request.headers().get("SplinterProtocolVersion") {
                Some(header_value) => match header_value.to_str() {
                    Ok(protocol_version) => match u32::from_str(protocol_version) {
                        Ok(protocol_version) => protocol_version,
                        Err(_) => {
                            return Box::new(
                                HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(
                                        "Unable to parse SplinterProtocolVersion",
                                    ))
                                    .into_future(),
                            )
                        }
                    },
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
                None => SPLINTER_PROTOCOL_VERSION,
            };

            debug!(
                "Beginning application authorization handler registration for \"{}\" \
                    with protocol {}",
                circuit_management_type, protocol_version
            );

            let mut query =
                match web::Query::<HashMap<String, u64>>::from_query(request.query_string()) {
                    Ok(query) => query,
                    Err(_) => return Box::new(HttpResponse::BadRequest().finish().into_future()),
                };

            let initial_events: Vec<JsonAdminEvent> = {
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
                    Ok(events) => {
                        match events
                            .map(|event| JsonAdminEvent::new(&event, protocol_version))
                            .skip(skip)
                            .collect::<Result<Vec<JsonAdminEvent>, InvalidStateError>>()
                        {
                            Ok(events) => events,
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
                    }
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
            match new_websocket_event_sender(request, Box::new(initial_events.into_iter())) {
                Ok((sender, res)) => {
                    if let Err(err) = admin_commands.add_event_subscriber(
                        &circuit_management_type,
                        Box::new(WsAdminServiceEventSubscriber {
                            sender,
                            protocol_version,
                        }),
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
}

struct WsAdminServiceEventSubscriber {
    sender: EventSender<JsonAdminEvent>,
    protocol_version: u32,
}

impl AdminServiceEventSubscriber for WsAdminServiceEventSubscriber {
    fn handle_event(&self, event: &store::AdminServiceEvent) -> Result<(), AdminSubscriberError> {
        let json_event = JsonAdminEvent::new(event, self.protocol_version)
            .map_err(|err| AdminSubscriberError::UnableToHandleEvent(err.to_string()))?;
        self.sender.send(json_event).map_err(|_| {
            debug!("Dropping admin service event and unsubscribing due to websocket being closed");
            AdminSubscriberError::Unsubscribe
        })
    }
}

// `timestamp` is set to the current time to allow for backward-compatibility, as the
// `timestamp` is not used by the `AdminServiceStore`.
#[derive(Debug, Serialize, Clone)]
struct JsonAdminEvent {
    #[serde(serialize_with = "st_as_millis")]
    timestamp: time::SystemTime,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(flatten)]
    event: Option<AdminServiceEvent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "event")]
    #[serde(flatten)]
    event_v1: Option<v1::AdminServiceEvent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<i64>,
}

impl JsonAdminEvent {
    fn new(
        event: &store::AdminServiceEvent,
        protocol_version: u32,
    ) -> Result<Self, InvalidStateError> {
        match protocol_version {
            1 => Ok(Self {
                timestamp: time::SystemTime::now(),
                event: None,
                event_v1: Some(v1::AdminServiceEvent::try_from(event).map_err(|_| {
                    InvalidStateError::with_message(
                        "Unable to convert store event into v1 event".into(),
                    )
                })?),
                event_id: Some(*event.event_id()),
            }),
            // Handles 2
            2 => Ok(Self {
                timestamp: time::SystemTime::now(),
                event: Some(AdminServiceEvent::from(event)),
                event_v1: None,
                event_id: Some(*event.event_id()),
            }),
            _ => Err(InvalidStateError::with_message(format!(
                "Unsupported SplinterProtocolVersion: {}",
                protocol_version
            ))),
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
