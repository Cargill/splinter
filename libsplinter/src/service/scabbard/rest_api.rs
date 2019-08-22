// Copyright 2019 Cargill Incorporated
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

use std::sync::{Arc, Mutex};

use transact::protocol::batch::BatchPair;
use transact::protos::FromBytes;

use crate::actix_web::{web, Error as ActixError, HttpResponse};
use crate::futures::{stream::Stream, Future, IntoFuture};
use crate::rest_api::{Method, Resource, RestResourceProvider};

use super::{shared::ScabbardShared, Scabbard};

impl RestResourceProvider for Scabbard {
    fn resources(&self) -> Vec<Resource> {
        let shared = self.shared.clone();
        vec![Resource::new(Method::Post, "/batches", move |_, p| {
            add_batch_to_queue(p, shared.clone())
        })]
    }
}

fn add_batch_to_queue(
    payload: web::Payload,
    shared: Arc<Mutex<ScabbardShared>>,
) -> Box<Future<Item = HttpResponse, Error = ActixError>> {
    Box::new(
        payload
            .from_err()
            .fold(web::BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, ActixError>(body)
            })
            .and_then(move |body| {
                let batches: Vec<BatchPair> = match Vec::from_bytes(&body) {
                    Ok(batches) => batches,
                    Err(_) => return Box::new(HttpResponse::BadRequest().finish().into_future()),
                };

                if let Ok(mut shared) = shared.lock() {
                    for batch in batches {
                        shared.add_batch_to_queue(batch);
                    }
                    Box::new(HttpResponse::Ok().finish().into_future())
                } else {
                    Box::new(HttpResponse::InternalServerError().finish().into_future())
                }
            })
            .into_future(),
    )
}
