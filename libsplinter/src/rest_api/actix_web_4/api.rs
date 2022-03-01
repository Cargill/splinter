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

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

use actix_web_4::http::Method;
use actix_web_4::rt::System as ActixSystem;
use actix_web_4::{dev::ServerHandle, middleware, App, HttpServer};
use futures_0_3::executor::block_on;
use openssl::ssl::SslAcceptorBuilder;

use crate::error::InternalError;
use crate::rest_api::auth::authorization::Permission;
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::{AuthorizationHandler, PermissionMap};
#[cfg(feature = "authorization")]
use crate::rest_api::auth::identity::IdentityProvider;
use crate::rest_api::RestApiServerError;
#[cfg(feature = "store-factory")]
use crate::store::StoreFactory;
use crate::threading::lifecycle::ShutdownHandle;

use super::ResourceProvider;

/// Contains information about the ports to which the REST API is bound.
#[derive(Debug)]
pub struct BindAddress {
    /// The SocketAddr which defines the bound port.
    pub addr: SocketAddr,

    /// The scheme (such as http) that is running on this port.
    pub scheme: String,
}

/// A running instance of the REST API.
pub struct RestApi {
    bind_addresses: Vec<BindAddress>,
    handle: ServerHandle,
    shutdown_future: Option<Pin<Box<dyn Future<Output = ()>>>>,
}

impl RestApi {
    pub(super) fn new(
        bind_url: String,
        bind_acceptor_builder: Option<SslAcceptorBuilder>,
        resource_providers: Vec<Box<dyn ResourceProvider>>,
        #[cfg(feature = "store-factory")] store_factory: Option<Box<dyn StoreFactory + Send>>,
        #[cfg(feature = "authorization")] identity_providers: Vec<Box<dyn IdentityProvider>>,
        #[cfg(feature = "authorization")] authorization_handlers: Vec<
            Box<dyn AuthorizationHandler>,
        >,
    ) -> Result<Self, RestApiServerError> {
        let providers: Arc<Mutex<Vec<_>>> = Arc::new(Mutex::new(resource_providers));
        let permission_map = Arc::new(RwLock::new(PermissionMap::new()));
        {
            let mut map = permission_map.write().unwrap();
            map.add_permission(Method::GET, "/", Permission::AllowUnauthenticated);
        }
        let sys = ActixSystem::new();
        #[cfg(feature = "store-factory")]
        let store_factory = store_factory.map(|factory| Arc::new(Mutex::new(factory)));

        let mut http_server = HttpServer::new(move || {
            let auth_transform = super::auth::AuthTransform::new(
                identity_providers.clone(),
                #[cfg(feature = "authorization")]
                authorization_handlers.clone(),
                #[cfg(feature = "authorization")]
                permission_map.clone(),
            );
            let mut app = App::new();
            #[cfg(feature = "store-factory")]
            {
                if let Some(factory) = &store_factory {
                    app = app.app_data(factory.clone());
                }
            }

            let mut app = app.wrap(middleware::Logger::default()).wrap(auth_transform);
            let pros = providers.lock().unwrap();

            for provider in pros.iter() {
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
                return Err(RestApiServerError::StartUpError(format!(
                    "{}: {}",
                    error_msg, err1
                )));
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
        let handle = server.handle();

        // Send the server and bind addresses to the parent thread
        /*
        if let Err(err) = sender.send(FromThreadMessage::Running(server, bind_addresses)) {
            error!("Unable to send running message to parent thread: {}", err);
            return;
        }*/

        match sys.block_on(server) {
            Ok(()) => info!("Rest API terminating"),
            Err(err) => error!("REST API unexpectedly exiting: {}", err),
        };
        Ok(RestApi {
            bind_addresses,
            handle,
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
        self.shutdown_future = Some(Box::pin(self.handle.stop(true)));
    }

    fn wait_for_shutdown(mut self) -> Result<(), InternalError> {
        match self.shutdown_future.take() {
            Some(f) => {
                block_on(f);
                Ok(())
            }
            _ => Err(InternalError::with_message(
                "Called wait_for_shutdown() prior to signal_shutdown()".to_string(),
            )),
        }
    }
}
