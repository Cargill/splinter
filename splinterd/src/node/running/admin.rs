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

//! This module defines the running admin subsystem.

use splinter::admin::client::event::{AdminServiceEventClient, AwcAdminServiceEventClientBuilder};
use splinter::error::InternalError;
use splinter::events::Reactor;
use splinter::registry::RegistryWriter;
use splinter::rest_api::actix_web_1::Resource as Actix1Resource;
use splinter::service::ServiceProcessorShutdownHandle;
use splinter::store::StoreFactory;
use splinter::threading::lifecycle::ShutdownHandle;

/// The configured, running Admin Service Event client variants.
pub enum AdminServiceEventClientVariant {
    ActixWebClient(Reactor),
}

/// A running admin subsystem.
pub struct AdminSubsystem {
    pub(crate) registry_writer: Box<dyn RegistryWriter>,
    pub(crate) admin_service_shutdown: ServiceProcessorShutdownHandle,
    pub(crate) actix1_resources: Vec<Actix1Resource>,
    pub(crate) store_factory: Box<dyn StoreFactory>,
    pub(crate) admin_service_event_client_variant: AdminServiceEventClientVariant,
}

impl AdminSubsystem {
    /// Take the available REST Resources from this subsystem.
    pub fn take_actix1_resources(&mut self) -> Vec<Actix1Resource> {
        let mut replaced = vec![];
        std::mem::swap(&mut self.actix1_resources, &mut replaced);
        replaced
    }

    pub fn registry_writer(&self) -> &dyn RegistryWriter {
        &*self.registry_writer
    }

    pub fn admin_service_event_client(
        &self,
        splinter_url: String,
        authorization: String,
        event_type: String,
    ) -> Result<Box<dyn AdminServiceEventClient>, InternalError> {
        match &self.admin_service_event_client_variant {
            AdminServiceEventClientVariant::ActixWebClient(reactor) => {
                AwcAdminServiceEventClientBuilder::new()
                    .with_reactor(reactor)
                    .with_splinter_url(splinter_url)
                    .with_event_type(event_type)
                    .with_authorization(authorization)
                    .build()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?
                    .run()
                    .map(|client| Box::new(client) as Box<dyn AdminServiceEventClient>)
            }
        }
    }
}

impl ShutdownHandle for AdminSubsystem {
    fn signal_shutdown(&mut self) {
        let AdminServiceEventClientVariant::ActixWebClient(reactor) =
            &self.admin_service_event_client_variant;

        if let Err(err) = reactor.shutdown_signaler().signal_shutdown() {
            error!("Unable to cleanly signal reactor shutdown: {}", err);
        }

        self.admin_service_shutdown.signal_shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        let mut errors = vec![];

        let AdminServiceEventClientVariant::ActixWebClient(reactor) =
            self.admin_service_event_client_variant;

        if let Err(e) = reactor.wait_for_shutdown() {
            errors.push(InternalError::from_source(Box::new(e)));
        }

        if let Err(err) = self.admin_service_shutdown.wait_for_shutdown() {
            errors.push(err)
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.remove(0)),
            _ => Err(InternalError::with_message(format!(
                "Multiple errors occurred during shutdown: {}",
                errors
                    .into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}
