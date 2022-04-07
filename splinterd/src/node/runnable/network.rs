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

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use cylinder::VerifierFactory;
use splinter::admin::service::admin_service_id;
use splinter::circuit::handlers::{
    AdminDirectMessageHandler, CircuitDirectMessageHandler, CircuitErrorHandler,
    CircuitMessageHandler, ServiceConnectRequestHandler, ServiceDisconnectRequestHandler,
};
use splinter::circuit::routing::{memory::RoutingTable, RoutingTableReader, RoutingTableWriter};
use splinter::error::InternalError;
use splinter::mesh::Mesh;
use splinter::network::auth::AuthorizationManager;
use splinter::network::connection_manager::{
    authorizers::Authorizers, authorizers::InprocAuthorizer, ConnectionManager, Connector,
};
use splinter::network::dispatch::{
    dispatch_channel, DispatchLoopBuilder, DispatchMessageSender, Dispatcher,
};
use splinter::network::handlers::{NetworkEchoHandler, NetworkHeartbeatHandler};
use splinter::peer::interconnect::NetworkMessageSender;
use splinter::peer::{interconnect::PeerInterconnectBuilder, PeerManager};
use splinter::protos::circuit::CircuitMessageType;
use splinter::protos::network::NetworkMessageType;
use splinter::public_key::PublicKey;
use splinter::transport::{
    inproc::InprocTransport, multi::MultiTransport, AcceptError, Incoming, Listener, Transport,
};

use crate::node::running::network::NetworkSubsystem;

pub struct RunnableNetworkSubsystem {
    pub node_id: String,
    pub transport: MultiTransport,
    pub heartbeat_interval: Duration,
    pub strict_ref_counts: bool,
    pub network_endpoints: Option<Vec<String>>,
    pub signing_context: Arc<Mutex<Box<dyn VerifierFactory>>>,
    pub signers: Vec<Box<dyn cylinder::Signer>>,
}

impl RunnableNetworkSubsystem {
    pub fn run(self) -> Result<NetworkSubsystem, InternalError> {
        let node_id = self.node_id;
        let heartbeat_interval = self.heartbeat_interval;
        let mut transport = self.transport;

        let service_transport = InprocTransport::default();
        transport.add_transport(Box::new(service_transport.clone()));

        let internal_service_listeners = Self::build_internal_service_listeners(&mut transport)?;

        let mut network_endpoints = vec![];
        let mut network_listeners = vec![];
        // setup listener for specified network endpoints. If no endpoints are specified set up 1
        // endpoint for some available port and set that to the network endpoints
        if let Some(specified_network_endpoints) = self.network_endpoints {
            network_listeners.append(&mut Self::build_network_listeners(
                &mut transport,
                &specified_network_endpoints,
            )?);
            network_endpoints = specified_network_endpoints;
        } else {
            network_listeners.append(&mut Self::build_network_listeners(
                &mut transport,
                &["tcp://127.0.0.1:0".to_string()],
            )?);
            for network_listener in network_listeners.iter() {
                network_endpoints.push(network_listener.endpoint().clone())
            }
        }

        let mesh = Mesh::new(512, 128);

        let authorization_manager = AuthorizationManager::new(
            node_id.to_string(),
            self.signers.clone(),
            self.signing_context.clone(),
        )
        .map_err(|err| InternalError::from_source(Box::new(err)))?;

        // Configure connection manager
        let connection_manager = Self::build_connection_manager(
            &node_id,
            Box::new(transport),
            &mesh,
            heartbeat_interval,
            &authorization_manager,
        )?;
        let connection_connector = connection_manager.connector();

        let peer_manager = Self::build_peer_manager(
            &node_id,
            connection_connector.clone(),
            self.strict_ref_counts,
        )?;

        let (network_dispatcher_sender, network_dispatch_receiver) = dispatch_channel();
        let interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_manager.connector())
            .with_message_receiver(mesh.get_receiver())
            .with_message_sender(mesh.get_sender())
            .with_network_dispatcher_sender(network_dispatcher_sender.clone())
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let network_sender = interconnect.new_network_sender();
        let routing_table = RoutingTable::default();
        let routing_writer: Box<dyn RoutingTableWriter> = Box::new(routing_table.clone());
        let routing_reader: Box<dyn RoutingTableReader> = Box::new(routing_table.clone());

        let public_keys = self
            .signers
            .iter()
            .map(|signer| {
                Ok(signer
                    .public_key()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .into())
            })
            .collect::<Result<Vec<PublicKey>, InternalError>>()?;

        // Set up the Circuit dispatcher
        let circuit_dispatcher = Self::set_up_circuit_dispatcher(
            network_sender.clone(),
            &node_id,
            routing_reader,
            routing_writer,
            public_keys,
        );
        let circuit_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(circuit_dispatcher)
            .with_thread_name("CircuitDispatchLoop".to_string())
            .build()
            .map_err(InternalError::with_message)?;

        let circuit_dispatch_sender = circuit_dispatch_loop.new_dispatcher_sender();

        // Set up the Network dispatcher
        let network_dispatcher =
            Self::set_up_network_dispatcher(network_sender, &node_id, circuit_dispatch_sender);

        let network_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(network_dispatcher)
            .with_thread_name("NetworkDispatchLoop".to_string())
            .with_dispatch_channel((network_dispatcher_sender, network_dispatch_receiver))
            .build()
            .map_err(InternalError::with_message)?;

        let mut network_listener_joinhandles =
            Self::listen_for_incoming_peers(network_listeners, &connection_connector)?;

        let service_listener_joinhandle =
            Self::listen_for_services(internal_service_listeners, &connection_connector)?;

        network_listener_joinhandles.push(service_listener_joinhandle);

        Ok(NetworkSubsystem {
            peer_manager,
            authorization_manager,
            connection_manager,
            routing_table,
            _network_listener_joinhandles: network_listener_joinhandles,
            network_endpoints,
            circuit_dispatch_loop,
            network_dispatch_loop,
            interconnect,
            service_transport,
            mesh,
        })
    }

    fn build_internal_service_listeners(
        transport: &mut dyn Transport,
    ) -> Result<Vec<Box<dyn Listener>>, InternalError> {
        Ok(vec![
            transport
                .listen("inproc://admin-service")
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
            transport
                .listen("inproc://orchestator")
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
        ])
    }

    fn build_network_listeners(
        transport: &mut dyn Transport,
        network_endpoints: &[String],
    ) -> Result<Vec<Box<dyn Listener>>, InternalError> {
        let mut listeners = vec![];

        for network_endpoint in network_endpoints {
            listeners.push(
                transport
                    .listen(network_endpoint)
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
            )
        }

        Ok(listeners)
    }

    fn build_connection_manager(
        node_id: &str,
        transport: Box<dyn Transport + Send>,
        mesh: &Mesh,
        heartbeat_interval: Duration,
        authorization_manager: &AuthorizationManager,
    ) -> Result<ConnectionManager, InternalError> {
        let inproc_ids = vec![
            (
                "inproc://orchestator".to_string(),
                format!("orchestator::{}", node_id),
            ),
            (
                "inproc://admin-service".to_string(),
                admin_service_id(node_id),
            ),
        ];

        // Set up Authorization
        let inproc_authorizer = InprocAuthorizer::new(inproc_ids, node_id.to_string());

        let mut authorizers = Authorizers::new();
        authorizers.add_authorizer("inproc", inproc_authorizer);
        authorizers.add_authorizer("", authorization_manager.authorization_connector());

        ConnectionManager::builder()
            .with_authorizer(Box::new(authorizers))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .with_heartbeat_interval(heartbeat_interval.as_secs())
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn build_peer_manager(
        node_id: &str,
        connection_connector: Connector,
        strict_ref_counts: bool,
    ) -> Result<PeerManager, InternalError> {
        PeerManager::builder()
            .with_connector(connection_connector)
            .with_identity(node_id.to_string())
            .with_strict_ref_counts(strict_ref_counts)
            .start()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }

    fn set_up_circuit_dispatcher(
        network_sender: NetworkMessageSender,
        node_id: &str,
        routing_reader: Box<dyn RoutingTableReader>,
        routing_writer: Box<dyn RoutingTableWriter>,
        public_keys: Vec<PublicKey>,
    ) -> Dispatcher<CircuitMessageType> {
        let mut dispatcher = Dispatcher::<CircuitMessageType>::new(Box::new(network_sender));

        let service_connect_request_handler = ServiceConnectRequestHandler::new(
            node_id.to_string(),
            routing_reader.clone(),
            routing_writer.clone(),
        );
        dispatcher.set_handler(Box::new(service_connect_request_handler));

        let service_disconnect_request_handler =
            ServiceDisconnectRequestHandler::new(routing_reader.clone(), routing_writer.clone());
        dispatcher.set_handler(Box::new(service_disconnect_request_handler));

        let direct_message_handler =
            CircuitDirectMessageHandler::new(node_id.to_string(), routing_reader.clone());
        dispatcher.set_handler(Box::new(direct_message_handler));

        let circuit_error_handler =
            CircuitErrorHandler::new(node_id.to_string(), routing_reader.clone());
        dispatcher.set_handler(Box::new(circuit_error_handler));

        // Circuit Admin handlers
        let admin_direct_message_handler =
            AdminDirectMessageHandler::new(node_id.to_string(), routing_reader, public_keys);
        dispatcher.set_handler(Box::new(admin_direct_message_handler));

        dispatcher
    }

    fn set_up_network_dispatcher(
        network_sender: NetworkMessageSender,
        node_id: &str,
        circuit_sender: DispatchMessageSender<CircuitMessageType>,
    ) -> Dispatcher<NetworkMessageType> {
        let mut dispatcher = Dispatcher::<NetworkMessageType>::new(Box::new(network_sender));

        let network_echo_handler = NetworkEchoHandler::new(node_id.to_string());
        dispatcher.set_handler(Box::new(network_echo_handler));

        let network_heartbeat_handler = NetworkHeartbeatHandler::new();
        // do not add auth guard
        dispatcher.set_handler(Box::new(network_heartbeat_handler));

        let circuit_message_handler = CircuitMessageHandler::new(circuit_sender);
        dispatcher.set_handler(Box::new(circuit_message_handler));

        dispatcher
    }

    fn listen_for_incoming_peers(
        network_listeners: Vec<Box<dyn Listener>>,
        connection_connector: &Connector,
    ) -> Result<Vec<thread::JoinHandle<()>>, InternalError> {
        // setup threads to listen on the network ports and add incoming connections to the network
        // these threads will just be dropped on shutdown
        network_listeners
            .into_iter()
            .map(|mut network_listener| {
                let connection_connector_clone = connection_connector.clone();
                thread::Builder::new()
                    .name(format!(
                        "IncomingPeerConnectionListener-{}",
                        network_listener.endpoint()
                    ))
                    .spawn(move || {
                        let endpoint = network_listener.endpoint();
                        for connection_result in network_listener.incoming() {
                            let connection = match connection_result {
                                Ok(connection) => connection,
                                Err(AcceptError::ProtocolError(msg)) => {
                                    warn!("Failed to accept connection on {}: {}", endpoint, msg);
                                    continue;
                                }
                                Err(AcceptError::IoError(err)) => {
                                    warn!("Failed to accept connection on {}: {}", endpoint, err);
                                    continue;
                                }
                            };
                            debug!("Received connection from {}", connection.remote_endpoint());
                            if let Err(err) =
                                connection_connector_clone.add_inbound_connection(connection)
                            {
                                error!(
                                    "Unable to add inbound connection to connection manager: {}",
                                    err
                                );
                                error!("Exiting listener thread for {}", endpoint);
                                break;
                            }
                        }
                    })
                    .map_err(|_| InternalError::with_message("Unable to spawn thread".into()))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    fn listen_for_services(
        internal_service_listeners: Vec<Box<dyn Listener>>,
        connection_connector: &Connector,
    ) -> Result<thread::JoinHandle<()>, InternalError> {
        // this thread will just be dropped on shutdown
        let connection_connector = connection_connector.clone();
        thread::Builder::new()
            .name("ServiceIncomingConnectionListener".into())
            .spawn(move || {
                // accept the internal service connections
                for mut listener in internal_service_listeners.into_iter() {
                    match listener.incoming().next() {
                        Some(Ok(connection)) => {
                            let remote_endpoint = connection.remote_endpoint();
                            if let Err(err) =
                                connection_connector.add_inbound_connection(connection)
                            {
                                error!("Unable to add peer {}: {}", remote_endpoint, err)
                            }
                        }
                        Some(Err(err)) => {
                            error!("Accept Error: {:?}", err);
                            break;
                        }
                        None => {}
                    }
                }
            })
            .map_err(|_| InternalError::with_message("Unable to spawn thread".into()))
    }
}
