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

use splinter::error::InternalError;
use splinter::network::connection_manager::ConnectionManager;
use splinter::peer::PeerManager;
use splinter::rest_api::actix_web_1::Resource as Actix1Resource;
use splinter::service::ServiceProcessor;
use splinter::threading::lifecycle::ShutdownHandle;

/// A running admin subsystem.
pub struct AdminSubsystem {
    pub(crate) node_id: String,
    pub(crate) _admin_service_processor: ServiceProcessor,
    pub(crate) actix1_resources: Vec<Actix1Resource>,
    pub(crate) connection_manager: ConnectionManager,
    pub(crate) peer_manager: PeerManager,
}

impl AdminSubsystem {
    /// Returns the current node ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Take the available REST Resources from this subsystem.
    pub fn take_actix1_resources(&mut self) -> Vec<Actix1Resource> {
        let mut replaced = vec![];
        std::mem::swap(&mut self.actix1_resources, &mut replaced);
        replaced
    }
}

impl ShutdownHandle for AdminSubsystem {
    fn signal_shutdown(&mut self) {
        self.peer_manager.signal_shutdown();
        self.connection_manager.signal_shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        let mut errors = vec![];

        if let Err(err) = self.peer_manager.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.connection_manager.wait_for_shutdown() {
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
