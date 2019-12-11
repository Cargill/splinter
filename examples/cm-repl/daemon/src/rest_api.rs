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

use std::thread;

use serde::{Deserialize, Serialize};
use splinter::{
    actix_web::{web, Error, HttpResponse},
    futures::{future::IntoFuture, stream::Stream, Future},
    network::connection_manager::{
        messages::{CmResponse, CmResponseStatus},
        Connector,
    },
    rest_api::{Method, Resource, RestApiBuilder, RestApiShutdownHandle, RestResourceProvider},
};

pub struct Provider {
    connector: Connector,
}

impl Provider {
    pub fn new(connector: Connector) -> Self {
        Self { connector }
    }
}

impl RestResourceProvider for Provider {
    fn resources(&self) -> Vec<Resource> {
        vec![
            make_add_connection(self.connector.clone()),
            make_remove_connection(self.connector.clone()),
            make_fetch_connections(self.connector.clone()),
        ]
    }
}

fn make_add_connection(connector: Connector) -> Resource {
    Resource::build("/connections/create").add_method(Method::Post, move |_, payload| {
        let connector = connector.clone();
        Box::new(
            payload
                .from_err::<Error>()
                .fold(web::BytesMut::new(), move |mut body, chunk| {
                    body.extend_from_slice(&chunk);
                    Ok::<_, Error>(body)
                })
                .and_then(move |body| match serde_json::from_slice::<Payload>(&body) {
                    Ok(payload) => match connector.request_connection(&payload.url) {
                        Ok(CmResponse::AddConnection {
                            status,
                            error_message,
                        }) if status == CmResponseStatus::Error => {
                            HttpResponse::InternalServerError().body(format!("{:?}", error_message))
                        }
                        Ok(res) => HttpResponse::Ok().body(format!("{:?}", res)),
                        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
                    },
                    Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
                })
                .into_future(),
        )
    })
}

fn make_remove_connection(connector: Connector) -> Resource {
    Resource::build("/connections/delete").add_method(Method::Delete, move |_, payload| {
        let connector = connector.clone();
        Box::new(
            payload
                .from_err::<Error>()
                .fold(web::BytesMut::new(), move |mut body, chunk| {
                    body.extend_from_slice(&chunk);
                    Ok::<_, Error>(body)
                })
                .and_then(move |body| match serde_json::from_slice::<Payload>(&body) {
                    Ok(payload) => match connector.remove_connection(&payload.url) {
                        Ok(CmResponse::AddConnection {
                            status,
                            error_message,
                        }) if status == CmResponseStatus::Error => {
                            HttpResponse::InternalServerError().body(format!("{:?}", error_message))
                        }
                        Ok(res) => HttpResponse::Ok().body(format!("{:?}", res)),
                        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
                    },
                    Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
                })
                .into_future(),
        )
    })
}

fn make_fetch_connections(connector: Connector) -> Resource {
    Resource::build("/connections/fetch").add_method(Method::Get, move |_, _| {
        match connector.list_connections() {
            Ok(res) => Box::new(HttpResponse::Ok().body(format!("{:?}", res)).into_future()),
            Err(err) => Box::new(
                HttpResponse::InternalServerError()
                    .body(format!("{:?}", err))
                    .into_future(),
            ),
        }
    })
}

#[derive(Serialize, Deserialize)]
struct Payload {
    url: String,
}

pub fn start_rest_api(
    connector: Connector,
    bind: &str,
) -> (RestApiShutdownHandle, thread::JoinHandle<()>) {
    let provider = Provider::new(connector);

    let builder = RestApiBuilder::new();
    builder
        .with_bind(bind)
        .add_resources(provider.resources())
        .build()
        .expect("Failed to build rest api")
        .run()
        .expect("Failed to start rest api")
}
