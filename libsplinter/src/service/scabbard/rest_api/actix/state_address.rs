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

use std::sync::Arc;

use crate::actix_web::HttpResponse;
use crate::futures::IntoFuture;
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard};
use crate::service::rest_api::ServiceEndpoint;
use crate::service::scabbard::{Scabbard, SERVICE_TYPE};

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
                    return Box::new(
                        HttpResponse::InternalServerError()
                            .json(ErrorResponse::internal_error())
                            .into_future(),
                    );
                }
            };

            let address = request
                .match_info()
                .get("address")
                .expect("address should not be none");

            Box::new(match scabbard.get_state_at_address(address) {
                Ok(Some(value)) => HttpResponse::Ok().json(value).into_future(),
                Ok(None) => HttpResponse::NotFound()
                    .json(ErrorResponse::not_found("Address not set"))
                    .into_future(),
                Err(err) => {
                    error!("Failed to get state at adddress: {}", err);
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future()
                }
            })
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_GET_STATE_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}
