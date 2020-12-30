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

use std::sync::mpsc;
use std::thread;

use actix_web::{middleware, App, HttpServer};
use futures::Future;

#[cfg(feature = "authorization")]
use crate::rest_api::auth::PermissionMap;
#[cfg(feature = "auth")]
use crate::rest_api::auth::{actix::Authorization, identity::IdentityProvider};
#[cfg(feature = "rest-api-cors")]
use crate::rest_api::cors::Cors;
use crate::rest_api::{RestApiBind, RestApiServerError};

use super::Resource;

/// Shutdown handle returned by `RestApi::run`. Allows rest api instance to be shut down
/// gracefully.
pub struct RestApiShutdownHandle {
    do_shutdown: Box<dyn Fn() -> Result<(), RestApiServerError> + Send>,
    port_numbers: Vec<u16>,
}

impl RestApiShutdownHandle {
    pub fn shutdown(&self) -> Result<(), RestApiServerError> {
        (*self.do_shutdown)()
    }

    pub fn port_numbers(&self) -> Vec<u16> {
        self.port_numbers.clone()
    }
}

/// `RestApi` is used to create an instance of a restful web server.
pub struct RestApi {
    pub(super) resources: Vec<Resource>,
    pub(super) bind: RestApiBind,
    #[cfg(feature = "rest-api-cors")]
    pub(super) whitelist: Option<Vec<String>>,
    #[cfg(feature = "auth")]
    pub(super) identity_providers: Vec<Box<dyn IdentityProvider>>,
}

impl RestApi {
    pub fn run(
        self,
    ) -> Result<(RestApiShutdownHandle, thread::JoinHandle<()>), RestApiServerError> {
        let (tx, rx) = mpsc::channel();

        let bind_config_for_err = self.bind.clone();
        let resources = self.resources;
        #[cfg(feature = "rest-api-cors")]
        let whitelist = self.whitelist;
        #[cfg(feature = "auth")]
        let authorization = Authorization::new(self.identity_providers.to_owned());

        #[cfg(feature = "rest-api-cors")]
        let cors = match &whitelist {
            Some(list) => Cors::new(list.to_vec()),
            None => Cors::new_allow_any(),
        };

        #[cfg(feature = "https-bind")]
        let bind_info = match self.bind {
            RestApiBind::Secure {
                bind,
                cert_path,
                key_path,
            } => {
                let mut acceptor =
                    openssl::ssl::SslAcceptor::mozilla_modern(openssl::ssl::SslMethod::tls())?;
                acceptor.set_private_key_file(key_path, openssl::ssl::SslFiletype::PEM)?;
                acceptor.set_certificate_chain_file(&cert_path)?;
                acceptor.check_private_key()?;

                (bind, Some(acceptor))
            }
            RestApiBind::Insecure(bind) => (bind, None),
        };

        #[cfg(not(feature = "https-bind"))]
        let RestApiBind::Insecure(bind_info) = self.bind;

        let join_handle = thread::Builder::new()
            .name("SplinterDRestApi".into())
            .spawn(move || {
                let sys = actix::System::new("SplinterD-Rest-API");
                let server = HttpServer::new(move || {
                    let app = App::new();

                    #[cfg(feature = "rest-api-cors")]
                    let app = app.wrap(cors.clone());

                    #[cfg(feature = "auth")]
                    let app = app.wrap(authorization.clone());

                    let mut app = app.wrap(middleware::Logger::default());

                    #[cfg(feature = "authorization")]
                    let mut permission_map = PermissionMap::new();

                    for resource in resources.clone() {
                        #[cfg(feature = "authorization")]
                        {
                            let (route, mut permissions) = resource.into_route();
                            permission_map.append(&mut permissions);
                            app = app.service(route);
                        }
                        #[cfg(not(feature = "authorization"))]
                        {
                            app = app.service(resource.into_route());
                        }
                    }

                    #[cfg(feature = "authorization")]
                    let app = app.data(permission_map);

                    app
                });

                #[cfg(feature = "https-bind")]
                let (bind_url, opt_acceptor) = bind_info;
                #[cfg(not(feature = "https-bind"))]
                let bind_url = bind_info;

                #[cfg(feature = "https-bind")]
                let server = if let Some(acceptor) = opt_acceptor {
                    server.bind_ssl(&bind_url, acceptor)
                } else {
                    server.bind(&bind_url)
                };

                #[cfg(not(feature = "https-bind"))]
                let server = server.bind(&bind_url);

                let server = match server {
                    Ok(server) => server,
                    Err(err) => {
                        let error_msg = format!("Invalid REST API bind {}: {}", bind_url, err);
                        error!("{}", error_msg);
                        if let Err(err) = tx.send(Err(error_msg)) {
                            error!("Failed to notify receiver of bind error: {}", err);
                        }
                        return;
                    }
                };
                let port_numbers = server.addrs().iter().map(|addrs| addrs.port()).collect();

                let addr = server.disable_signals().system_exit().start();

                if let Err(err) = tx.send(Ok((addr, port_numbers))) {
                    error!("Unable to send Server Addr: {}", err);
                }

                if let Err(err) = sys.run() {
                    error!("REST Api unexpectedly exiting: {}", err);
                };

                info!("Rest API terminating");
            })?;

        let (addr, port_numbers) = rx
            .recv()
            .map_err(|err| {
                RestApiServerError::StartUpError(format!("Unable to receive Server Addr: {}", err))
            })?
            .map_err(|err| {
                RestApiServerError::BindError(format!(
                    "Failed to bind to URL {}: {}",
                    bind_config_for_err, err
                ))
            })?;

        let do_shutdown = Box::new(move || {
            debug!("Shutting down Rest API");
            if let Err(err) = addr.stop(true).wait() {
                error!("An error occured while shutting down rest API: {:?}", err);
            }
            debug!("Graceful signal sent to Rest API");

            Ok(())
        });

        Ok((
            RestApiShutdownHandle {
                do_shutdown,
                port_numbers,
            },
            join_handle,
        ))
    }

    /// Builds the `RestApi` without requiring any security configuration
    #[cfg(test)]
    pub fn run_insecure(
        self,
    ) -> Result<(RestApiShutdownHandle, thread::JoinHandle<()>), RestApiServerError> {
        let (tx, rx) = mpsc::channel();

        #[cfg(feature = "https-bind")]
        let bind_url = match self.bind.clone() {
            RestApiBind::Secure { bind, .. } => bind,

            RestApiBind::Insecure(bind) => bind,
        };

        #[cfg(not(feature = "https-bind"))]
        let RestApiBind::Insecure(bind_url) = self.bind.clone();

        let resources = self.resources.to_owned();
        #[cfg(feature = "rest-api-cors")]
        let whitelist = self.whitelist.to_owned();

        #[cfg(feature = "rest-api-cors")]
        let cors = match &whitelist {
            Some(list) => Cors::new(list.to_vec()),
            None => Cors::new_allow_any(),
        };

        let join_handle = thread::Builder::new()
            .name("SplinterDRestApi".into())
            .spawn(move || {
                let sys = actix::System::new("SplinterD-Rest-API");
                let mut server = HttpServer::new(move || {
                    let app = App::new();

                    #[cfg(feature = "rest-api-cors")]
                    let app = app.wrap(cors.clone());

                    let mut app = app.wrap(middleware::Logger::default());

                    for resource in resources.clone() {
                        #[cfg(feature = "authorization")]
                        {
                            app = app.service(resource.into_route().0);
                        }
                        #[cfg(not(feature = "authorization"))]
                        {
                            app = app.service(resource.into_route());
                        }
                    }
                    app
                });

                server = match server.bind(&bind_url) {
                    Ok(server) => server,
                    Err(err) => {
                        let error_msg = format!("Invalid REST API bind {}: {}", bind_url, err);
                        error!("{}", error_msg);
                        if let Err(err) = tx.send(Err(error_msg)) {
                            error!("Failed to notify receiver of bind error: {}", err);
                        }
                        return;
                    }
                };
                let port_numbers = server.addrs().iter().map(|addrs| addrs.port()).collect();

                let addr = server.disable_signals().system_exit().start();

                if let Err(err) = tx.send(Ok((addr, port_numbers))) {
                    error!("Unable to send Server Addr: {}", err);
                }

                if let Err(err) = sys.run() {
                    error!("REST Api unexpectedly exiting: {}", err);
                };

                info!("Rest API terminating");
            })?;

        let (addr, port_numbers) = rx
            .recv()
            .map_err(|err| {
                RestApiServerError::StartUpError(format!("Unable to receive Server Addr: {}", err))
            })?
            .map_err(|err| {
                RestApiServerError::BindError(format!(
                    "Failed to bind to URL {}: {}",
                    self.bind, err
                ))
            })?;

        let do_shutdown = Box::new(move || {
            debug!("Shutting down Rest API");
            if let Err(err) = addr.stop(true).wait() {
                error!("An error occured while shutting down rest API: {:?}", err);
            }
            debug!("Graceful signal sent to Rest API");

            Ok(())
        });

        Ok((
            RestApiShutdownHandle {
                do_shutdown,
                port_numbers,
            },
            join_handle,
        ))
    }
}
