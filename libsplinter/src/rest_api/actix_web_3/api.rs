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

//! Contains the implementation of `RestApi`.

use std::future::Future;
use std::io::Error as IoError;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use actix_0_10::System as ActixSystem;
use actix_web_3::{dev::Server, middleware, App, HttpServer};
use futures_0_3::executor::block_on;
use openssl::ssl::SslAcceptorBuilder;

use crate::error::InternalError;
use crate::rest_api::RestApiServerError;
use crate::threading::shutdown::ShutdownHandle;

use super::ResourceProvider;

/// Contains information about the ports to which the REST API is bound.
#[derive(Debug)]
pub struct BindAddress {
    /// The SocketAddr which defines the bound port.
    pub addr: SocketAddr,

    /// The scheme (such as http) that is running on this port.
    pub scheme: String,
}

enum FromThreadMessage {
    IoError(IoError, String),
    Running(Server, Vec<BindAddress>),
}

/// A running instance of the REST API.
pub struct RestApi {
    bind_addresses: Vec<BindAddress>,
    join_handle: Option<JoinHandle<()>>,
    server: Server,
    shutdown_future: Option<Pin<Box<dyn Future<Output = ()>>>>,
}

impl RestApi {
    pub(super) fn new(
        bind_url: String,
        bind_acceptor_builder: Option<SslAcceptorBuilder>,
        resource_providers: Vec<Box<dyn ResourceProvider>>,
    ) -> Result<Self, RestApiServerError> {
        let providers: Arc<Mutex<Vec<_>>> = Arc::new(Mutex::new(resource_providers));
        let (sender, receiver) = mpsc::channel();

        let join_handle = thread::Builder::new()
            .name("SplinterRestApi".into())
            .spawn(move || {
                let sys = ActixSystem::new("ActixSystem-Splinter-REST-API");
                let mut http_server = HttpServer::new(move || {
                    let app = App::new();

                    let mut app = app.wrap(middleware::Logger::default());

                    for provider in providers.lock().unwrap().iter() {
                        for resource in provider.resources() {
                            app = app.service(resource)
                        }
                    }
                    app
                });

                http_server = match if let Some(acceptor_builder) = bind_acceptor_builder {
                    http_server.bind_openssl(&bind_url, acceptor_builder)
                } else {
                    http_server.bind(&bind_url)
                } {
                    Ok(http_server) => http_server,
                    Err(err1) => {
                        let error_msg = format!("Bind to \"{}\" failed", bind_url);
                        if let Err(err2) =
                            sender.send(FromThreadMessage::IoError(err1, error_msg.clone()))
                        {
                            error!("{}", error_msg);
                            error!("Failed to notify receiver of bind error: {}", err2);
                        }
                        return;
                    }
                };

                let bind_addresses = http_server
                    .addrs_with_scheme()
                    .iter()
                    .map(|(addr, scheme)| BindAddress {
                        addr: *addr,
                        scheme: scheme.to_string(),
                    })
                    .collect();

                let server = http_server.disable_signals().system_exit().run();

                // Send the server and bind addresses to the parent thread
                if let Err(err) = sender.send(FromThreadMessage::Running(server, bind_addresses)) {
                    error!("Unable to send running message to parent thread: {}", err);
                    return;
                }

                match sys.run() {
                    Ok(()) => info!("Rest API terminating"),
                    Err(err) => error!("REST API unexpectedly exiting: {}", err),
                };
            })?;

        let (server, bind_addresses) = loop {
            match receiver.recv() {
                Ok(FromThreadMessage::Running(server, bind_address)) => {
                    break (server, bind_address);
                }
                Ok(FromThreadMessage::IoError(err, error_msg)) => Err(
                    RestApiServerError::StartUpError(format!("{}: {}", error_msg, err)),
                ),
                Err(err) => Err(RestApiServerError::StartUpError(format!(
                    "Error receiving message from Rest Api thread: {}",
                    err
                ))),
            }?;
        };

        Ok(RestApi {
            bind_addresses,
            join_handle: Some(join_handle),
            server,
            shutdown_future: None,
        })
    }

    /// Returns the list of addresses to which this REST API is bound.
    pub fn bind_addresses(&self) -> &Vec<BindAddress> {
        &self.bind_addresses
    }
}

impl ShutdownHandle for RestApi {
    fn signal_shutdown(&mut self) {
        self.shutdown_future = Some(Box::pin(self.server.stop(true)));
    }

    fn wait_for_shutdown(&mut self) -> Result<(), InternalError> {
        match (self.shutdown_future.take(), self.join_handle.take()) {
            (Some(f), Some(join_handle)) => {
                block_on(f);
                join_handle.join().map_err(|_| {
                    InternalError::with_message(
                        "RestApi thread panicked, join() failed".to_string(),
                    )
                })?;
                Ok(())
            }
            (_, _) => Err(InternalError::with_message(
                "Called wait_for_shutdown() prior to signal_shutdown()".to_string(),
            )),
        }
    }
}
