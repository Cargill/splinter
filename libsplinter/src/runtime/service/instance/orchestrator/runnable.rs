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

//! A module containing a configured, but not started ServiceOrchestrator.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use crate::error::InternalError;
use crate::mesh::Mesh;
use crate::network::reply::InboundRouter;
use crate::transport::Connection;

use super::{JoinHandles, OrchestratableServiceFactory, ServiceOrchestrator};

/// A runnable service orchestrator is configured, but not started ServiceOrchestrator. It may only
/// be used for operations that can be done during that state, such as starting the orchestrator.
pub struct RunnableServiceOrchestrator {
    pub(super) connection: Box<dyn Connection>,
    pub(super) incoming_capacity: usize,
    pub(super) outgoing_capacity: usize,
    pub(super) channel_capacity: usize,
    pub(super) service_factories: Vec<Box<dyn OrchestratableServiceFactory>>,
    pub(super) supported_service_types: Vec<String>,
}

impl RunnableServiceOrchestrator {
    /// Starts the ServiceOrchestrator.
    ///
    /// This transforms the instance into a [ServiceOrchestrator], resulting in the orchestrator
    /// being in a started state.
    ///
    /// # Returns
    ///
    /// A running [ServiceOrchestrator].  This instance can have operations applied to it, such as
    /// creating or destroying services.
    ///
    /// # Errors
    ///
    /// Returns an [InternalError] if the orchestrator cannot be started.
    pub fn run(self) -> Result<ServiceOrchestrator, InternalError> {
        let service_factories = self.service_factories;
        let supported_service_types = self.supported_service_types;

        let services = Arc::new(Mutex::new(HashMap::new()));
        let stopped_services = Arc::new(Mutex::new(HashMap::new()));

        let mesh = Mesh::new(self.incoming_capacity, self.outgoing_capacity);
        let mesh_id = format!("{}", Uuid::new_v4());

        mesh.add(self.connection, mesh_id.to_string())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let running = Arc::new(AtomicBool::new(true));

        let (network_sender, network_receiver) = crossbeam_channel::bounded(self.channel_capacity);
        let (inbound_sender, inbound_receiver) = crossbeam_channel::bounded(self.channel_capacity);
        let inbound_router = InboundRouter::new(Box::new(inbound_sender));

        // Start thread that handles incoming messages from a splinter node.
        let incoming_mesh = mesh.clone();
        let incoming_running = running.clone();
        let incoming_router = inbound_router.clone();
        let incoming_join_handle = thread::Builder::new()
            .name("Orchestrator Incoming".into())
            .spawn(move || {
                if let Err(err) =
                    super::run_incoming_loop(incoming_mesh, incoming_running, incoming_router)
                {
                    error!(
                        "Terminating orchestrator incoming thread due to error: {}",
                        err
                    );
                    Err(err)
                } else {
                    Ok(())
                }
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        // Start thread that handles messages that do not have a matching correlation id.
        let inbound_services = services.clone();
        let inbound_running = running.clone();
        let inbound_join_handle = thread::Builder::new()
            .name("Orchestrator Inbound".into())
            .spawn(move || {
                if let Err(err) =
                    super::run_inbound_loop(inbound_services, inbound_receiver, inbound_running)
                {
                    error!(
                        "Terminating orchestrator inbound thread due to error: {}",
                        err
                    );
                    Err(err)
                } else {
                    Ok(())
                }
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        // Start thread that handles outgoing messages that need to be sent to the splinter node.
        let outgoing_running = running.clone();
        let outgoing_join_handle = thread::Builder::new()
            .name("Orchestrator Outgoing".into())
            .spawn(move || {
                if let Err(err) =
                    super::run_outgoing_loop(mesh, outgoing_running, network_receiver, mesh_id)
                {
                    error!(
                        "Terminating orchestrator outgoing thread due to error: {}",
                        err
                    );
                    Err(err)
                } else {
                    Ok(())
                }
            })
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let join_handles = JoinHandles::new(vec![
            incoming_join_handle,
            inbound_join_handle,
            outgoing_join_handle,
        ]);

        info!("Service orchestrator started");
        Ok(ServiceOrchestrator {
            services,
            stopped_services,
            service_factories,
            supported_service_types,
            network_sender,
            inbound_router,
            running,
            join_handles: Some(join_handles),
        })
    }
}
