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

use transact::protocol::batch::BatchPair;
use transact::protos::FromBytes;

use crate::actix_web::{web, Error as ActixError, HttpResponse};
use crate::futures::{stream::Stream, Future, IntoFuture};
use crate::protocol;
use crate::rest_api::{ErrorResponse, Method, ProtocolVersionRangeGuard};
use crate::service::rest_api::ServiceEndpoint;
use crate::service::scabbard::{Scabbard, SERVICE_TYPE};

pub fn make_add_batches_to_queue_endpoint() -> ServiceEndpoint {
    ServiceEndpoint {
        service_type: SERVICE_TYPE.into(),
        route: "/batches".into(),
        method: Method::Post,
        handler: Arc::new(move |_, payload, service| {
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

            Box::new(
                payload
                    .from_err::<ActixError>()
                    .fold(web::BytesMut::new(), move |mut body, chunk| {
                        body.extend_from_slice(&chunk);
                        Ok::<_, ActixError>(body)
                    })
                    .into_future()
                    .and_then(move |body| {
                        let batches: Vec<BatchPair> = match Vec::from_bytes(&body) {
                            Ok(b) => b,
                            Err(_) => {
                                return HttpResponse::BadRequest()
                                    .json(ErrorResponse::bad_request(
                                        "Invalid body: not a valid list of batches",
                                    ))
                                    .into_future()
                            }
                        };

                        match scabbard.add_batches(batches) {
                            Ok(Some(link)) => HttpResponse::Accepted().json(link).into_future(),
                            Ok(None) => HttpResponse::BadRequest()
                                .json(ErrorResponse::bad_request("No valid batches provided"))
                                .into_future(),
                            Err(err) => {
                                error!("Failed to add batches: {}", err);
                                HttpResponse::InternalServerError()
                                    .json(ErrorResponse::internal_error())
                                    .into_future()
                            }
                        }
                    }),
            )
        }),
        request_guards: vec![Box::new(ProtocolVersionRangeGuard::new(
            protocol::SCABBARD_ADD_BATCHES_PROTOCOL_MIN,
            protocol::SCABBARD_PROTOCOL_VERSION,
        ))],
    }
}
