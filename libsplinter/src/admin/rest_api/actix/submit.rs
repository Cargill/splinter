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

use actix_web::HttpResponse;
use futures::{Future, IntoFuture};

#[cfg(feature = "authorization")]
use crate::admin::rest_api::CIRCUIT_WRITE_PERMISSION;
use crate::admin::service::{AdminCommands, AdminServiceError, AdminServiceStatus};
use crate::protocol;
use crate::protos::admin::CircuitManagementPayload;
use crate::rest_api::actix_web_1::{into_protobuf, Method, ProtocolVersionRangeGuard, Resource};
use crate::service::ServiceError;

pub fn make_submit_route<A: AdminCommands + Clone + 'static>(admin_commands: A) -> Resource {
    let resource =
        Resource::build("/admin/submit").add_request_guard(ProtocolVersionRangeGuard::new(
            protocol::ADMIN_SUBMIT_PROTOCOL_MIN,
            protocol::ADMIN_PROTOCOL_VERSION,
        ));

    #[cfg(feature = "authorization")]
    {
        resource.add_method(Method::Post, CIRCUIT_WRITE_PERMISSION, move |_, payload| {
            let admin_commands = admin_commands.clone();

            let status = if let Ok(status) = admin_commands.admin_service_status() {
                status
            } else {
                return Box::new(HttpResponse::InternalServerError().finish().into_future());
            };
            if status != AdminServiceStatus::Running {
                warn!("Admin service is not running");
                return Box::new(HttpResponse::ServiceUnavailable().finish().into_future());
            }

            Box::new(
                into_protobuf::<CircuitManagementPayload>(payload).and_then(move |payload| {
                    match admin_commands.submit_circuit_change(payload) {
                        Ok(()) => HttpResponse::Accepted().finish().into_future(),
                        Err(AdminServiceError::ServiceError(
                            ServiceError::UnableToHandleMessage(err),
                        )) => {
                            debug!("{}", err);
                            HttpResponse::BadRequest()
                                .json(json!({
                                    "message": format!("Unable to handle message: {}", err)
                                }))
                                .into_future()
                        }
                        Err(AdminServiceError::ServiceError(
                            ServiceError::InvalidMessageFormat(err),
                        )) => HttpResponse::BadRequest()
                            .json(json!({
                                "message": format!("Failed to parse payload: {}", err)
                            }))
                            .into_future(),
                        Err(err) => {
                            error!("{}", err);
                            HttpResponse::InternalServerError().finish().into_future()
                        }
                    }
                }),
            )
        })
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Post, move |_, payload| {
            let admin_commands = admin_commands.clone();
            let status = if let Ok(status) = admin_commands.admin_service_status() {
                status
            } else {
                return Box::new(HttpResponse::InternalServerError().finish().into_future());
            };
            if status != AdminServiceStatus::Running {
                warn!("Admin service is not running");
                return Box::new(HttpResponse::ServiceUnavailable().finish().into_future());
            }
            Box::new(
                into_protobuf::<CircuitManagementPayload>(payload).and_then(move |payload| {
                    match admin_commands.submit_circuit_change(payload) {
                        Ok(()) => HttpResponse::Accepted().finish().into_future(),
                        Err(AdminServiceError::ServiceError(
                            ServiceError::UnableToHandleMessage(err),
                        )) => {
                            debug!("{}", err);
                            HttpResponse::BadRequest()
                                .json(json!({
                                    "message": format!("Unable to handle message: {}", err)
                                }))
                                .into_future()
                        }
                        Err(AdminServiceError::ServiceError(
                            ServiceError::InvalidMessageFormat(err),
                        )) => HttpResponse::BadRequest()
                            .json(json!({
                                "message": format!("Failed to parse payload: {}", err)
                            }))
                            .into_future(),
                        Err(err) => {
                            error!("{}", err);
                            HttpResponse::InternalServerError().finish().into_future()
                        }
                    }
                }),
            )
        })
    }
}
