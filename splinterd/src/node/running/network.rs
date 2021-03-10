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

//! This module defines the running network subsystem.

use std::thread::JoinHandle;

use splinter::circuit::routing::{memory::RoutingTable, RoutingTableWriter};
use splinter::error::InternalError;
use splinter::mesh::Mesh;
use splinter::network::connection_manager::ConnectionManager;
use splinter::network::dispatch::DispatchLoop;
use splinter::peer::{interconnect::PeerInterconnect, PeerManager, PeerManagerConnector};
use splinter::protos::circuit::CircuitMessageType;
use splinter::protos::network::NetworkMessageType;
use splinter::threading::lifecycle::ShutdownHandle;
use splinter::transport::inproc::InprocTransport;

/// A running admin subsystem.
pub struct NetworkSubsystem {
    pub(crate) node_id: String,
    pub(crate) connection_manager: ConnectionManager,
    pub(crate) peer_manager: PeerManager,
    pub(crate) routing_table: RoutingTable,
    pub(crate) _network_listener_joinhandles: Vec<JoinHandle<()>>,
    pub(crate) network_endpoints: Vec<String>,
    pub(crate) circuit_dispatch_loop: DispatchLoop<CircuitMessageType>,
    pub(crate) network_dispatch_loop: DispatchLoop<NetworkMessageType>,
    pub(crate) interconnect: PeerInterconnect,
    pub(crate) service_transport: InprocTransport,
    pub(crate) mesh: Mesh,
}

impl NetworkSubsystem {
    /// Returns the current node ID.
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns a peer connector for the node's peer_manager
    pub fn peer_connector(&self) -> PeerManagerConnector {
        self.peer_manager.connector()
    }

    /// Returns a routing_table_writer for the routing table
    pub fn routing_table_writer(&self) -> Box<dyn RoutingTableWriter> {
        Box::new(self.routing_table.clone())
    }

    /// Returns the network endpoints for the node
    pub fn network_endpoints(&self) -> &[String] {
        &self.network_endpoints
    }

    /// Returns the network endpoints for the node
    pub fn service_transport(&self) -> InprocTransport {
        self.service_transport.clone()
    }
}

impl ShutdownHandle for NetworkSubsystem {
    fn signal_shutdown(&mut self) {
        self.interconnect.signal_shutdown();
        self.peer_manager.signal_shutdown();
        self.connection_manager.signal_shutdown();
        self.circuit_dispatch_loop.signal_shutdown();
        self.network_dispatch_loop.signal_shutdown();
        self.mesh.signal_shutdown();
    }

    fn wait_for_shutdown(self) -> Result<(), InternalError> {
        let mut errors = vec![];
        if let Err(err) = self.interconnect.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.peer_manager.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.connection_manager.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.circuit_dispatch_loop.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.network_dispatch_loop.wait_for_shutdown() {
            errors.push(err)
        }

        if let Err(err) = self.mesh.wait_for_shutdown() {
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
