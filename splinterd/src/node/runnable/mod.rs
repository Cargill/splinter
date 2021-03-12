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

//! Contains the implementation of `RunnableNode`.

pub(super) mod admin;
pub(super) mod network;

use std::net::{Ipv4Addr, SocketAddr};

use splinter::error::InternalError;
use splinter::rest_api::actix_web_1::RestApi;
use splinter::rest_api::actix_web_3::RunnableRestApi;

use super::builder::admin::AdminSubsystemBuilder;
use super::{Node, NodeRestApiVariant};

use self::network::RunnableNetworkSubsystem;

pub(super) enum RunnableNodeRestApiVariant {
    ActixWeb1(RestApi),
    ActixWeb3(RunnableRestApi),
}

/// A fully configured and runnable instance of a node.
pub struct RunnableNode {
    pub(super) admin_signer: Box<dyn cylinder::Signer>,
    pub(super) admin_subsystem_builder: AdminSubsystemBuilder,
    pub(super) rest_api_variant: RunnableNodeRestApiVariant,
    pub(super) runnable_network_subsystem: RunnableNetworkSubsystem,
    pub(super) node_id: String,
}

impl RunnableNode {
    /// Starts up the Node.
    pub fn run(self) -> Result<Node, InternalError> {
        let network_subsystem = self.runnable_network_subsystem.run()?;

        let runnable_admin_subsystem = self
            .admin_subsystem_builder
            .with_peer_connector(network_subsystem.peer_connector())
            .with_routing_writer(network_subsystem.routing_table_writer())
            .with_service_transport(network_subsystem.service_transport())
            .build()?;

        let mut admin_subsystem = runnable_admin_subsystem.run()?;

        let node_id = self.node_id;

        let rest_api_variant = match self.rest_api_variant {
            RunnableNodeRestApiVariant::ActixWeb1(rest_api) => {
                let admin_resources = admin_subsystem.take_actix1_resources();
                let (rest_api_shutdown_handle, rest_api_join_handle) = rest_api
                    .add_resources(admin_resources)
                    .run()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;

                let port_numbers = rest_api_shutdown_handle.port_numbers();

                // The REST API's collection of port numbers is busted, so if we
                // see more than one, it is meaningless and we have to abort what
                // we are doing. For example, if you give localhost:0 as a bind
                // argument to the REST API, this will return two ports, one for
                // ipv4 and one for ipv6, it's not clear which is which.
                if port_numbers.len() != 1 {
                    return Err(InternalError::with_message(format!(
                        "Expected a single port number but saw multiple: {:?}",
                        port_numbers
                    )));
                }

                NodeRestApiVariant::ActixWeb1(rest_api_shutdown_handle, rest_api_join_handle)
            }
            RunnableNodeRestApiVariant::ActixWeb3(runnable_rest_api) => {
                let rest_api = runnable_rest_api
                    .run()
                    .map_err(|e| InternalError::from_source(Box::new(e)))?;

                NodeRestApiVariant::ActixWeb3(rest_api)
            }
        };

        let rest_api_port = match &rest_api_variant {
            NodeRestApiVariant::ActixWeb1(shutdown_handle, _) => shutdown_handle.port_numbers()[0],
            NodeRestApiVariant::ActixWeb3(rest_api) => {
                // Determine the http port for IPv4 localhost, as that is the port that Node is
                // expecting to use for the client.
                let port_numbers: Vec<_> = rest_api
                    .bind_addresses()
                    .iter()
                    .filter_map(|bind_address| {
                        if bind_address.scheme == "http" {
                            match bind_address.addr {
                                SocketAddr::V4(addr) if *addr.ip() == Ipv4Addr::LOCALHOST => {
                                    Some(addr.port())
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    })
                    .collect();

                if port_numbers.len() != 1 {
                    return Err(InternalError::with_message(format!(
                        "Unable to determine http port for REST API: {:?}",
                        rest_api.bind_addresses(),
                    )));
                }

                port_numbers[0]
            }
        };

        Ok(Node {
            admin_signer: self.admin_signer,
            admin_subsystem,
            network_subsystem,
            rest_api_variant,
            rest_api_port,
            node_id,
        })
    }
}
