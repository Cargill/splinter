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

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::collections::BiHashMap;
use crate::matrix::{MatrixReceiver, MatrixRecvError, MatrixSender, MatrixShutdown};
use crate::network::dispatch::{DispatchLoopBuilder, DispatchLoopShutdownSignaler, Dispatcher};
use crate::network::sender::{NetworkMessageSender, SendRequest};
use crate::protos::network::{NetworkMessage, NetworkMessageType};

use super::connector::PeerManagerConnector;
use super::error::PeerInterconnectError;

/// PeerInterconnect will receive incoming messages from peers and dispatch them to the
/// NetworkMessageType handlers. It will also receive messages from handlers that need to be
/// sent to other peers.
///
/// When an incoming message is received, the connection_id is converted to a peer_id. The reverse
/// is done for an outgoing message. If a message is received from an unknown connection, the
/// PeerInterconnect will request the current peers from the PeerManager and update the local map
/// of peers.
pub struct PeerInterconnect<T: 'static>
where
    T: MatrixShutdown,
{
    // sender that will be wrapped in a NetworkMessageSender and given to Dispatchers for sending
    // messages to peers
    dispatched_sender: Sender<SendRequest>,
    recv_join_handle: thread::JoinHandle<()>,
    send_join_handle: thread::JoinHandle<()>,
    shutdown_handle: ShutdownHandle<T>,
}

impl<T> PeerInterconnect<T>
where
    T: MatrixShutdown,
{
    /// get a new NetworkMessageSender that can be used to send messages to peers.
    pub fn new_network_sender(&self) -> NetworkMessageSender {
        NetworkMessageSender::new(self.dispatched_sender.clone())
    }

    /// Returns a ShutdownHandle that can be used to shutdown PeerInterconnect
    pub fn shutdown_handle(&self) -> ShutdownHandle<T> {
        self.shutdown_handle.clone()
    }

    /// waits for the send and receive thread to shutdown
    pub fn await_shutdown(self) {
        if let Err(err) = self.send_join_handle.join() {
            error!(
                "Peer interconnect send thread did not shutdown correctly: {:?}",
                err
            );
        };

        if let Err(err) = self.recv_join_handle.join() {
            error!(
                "Peer interconnect recv thread did not shutdown correctly: {:?}",
                err
            );
        }
    }

    /// Call shutdown on the shutdown handle and then waits for the PeerInterconnect threads to
    /// finish
    pub fn shutdown_and_wait(self) {
        self.shutdown_handle().shutdown();
        self.await_shutdown();
    }
}

#[derive(Default)]
pub struct PeerInterconnectBuilder<T: 'static, U: 'static, V: 'static>
where
    T: MatrixReceiver,
    U: MatrixSender,
    V: MatrixShutdown,
{
    // peer connector to update the local peer map
    peer_connector: Option<PeerManagerConnector>,
    // MatrixReceiver to receive messages from peers
    message_receiver: Option<T>,
    // MatrixSender to send messages to peers
    message_sender: Option<U>,
    // MatrixShutdown to shutdown matrix
    message_shutdown: Option<V>,
    // a Dispatcher with handlers for NetworkMessageTypes
    network_dispatcher: Option<Dispatcher<NetworkMessageType>>,
}

impl<T, U, V> PeerInterconnectBuilder<T, U, V>
where
    T: MatrixReceiver,
    U: MatrixSender,
    V: MatrixShutdown,
{
    /// Creats an empty builder for a PeerInterconnect
    pub fn new() -> Self {
        PeerInterconnectBuilder {
            peer_connector: None,
            message_receiver: None,
            message_sender: None,
            message_shutdown: None,
            network_dispatcher: None,
        }
    }

    /// Add a PeerManagerConnector to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `peer_connector` - a PeerManagerConnector that will be used to get the currently
    ///     connected peers and the associated connection id.
    pub fn with_peer_connector(mut self, peer_connector: PeerManagerConnector) -> Self {
        self.peer_connector = Some(peer_connector);
        self
    }

    /// Add a MatrixReceiver to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_receiver` - a MatrixReceiver that will be used to receive messages from peers
    pub fn with_message_receiver(mut self, message_receiver: T) -> Self {
        self.message_receiver = Some(message_receiver);
        self
    }

    /// Add a MatrixSender to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_sender` - a MatrixSender that will be used to send messages to peers
    pub fn with_message_sender(mut self, message_sender: U) -> Self {
        self.message_sender = Some(message_sender);
        self
    }

    /// Add a MatrixShutdown to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_shutdown` - a MatrixShutdown that will be used to shutdown a MatrixSender
    pub fn with_message_shutdown(mut self, message_shutdown: V) -> Self {
        self.message_shutdown = Some(message_shutdown);
        self
    }

    /// Add a Dispatcher for NetworkMessageType to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `network_dispatcher` - a Dispatcher that is loaded with the Handlers for
    ////       NetworkMessageTypes
    pub fn with_network_dispatcher(
        mut self,
        network_dispatcher: Dispatcher<NetworkMessageType>,
    ) -> Self {
        self.network_dispatcher = Some(network_dispatcher);
        self
    }

    /// Build the PeerInterconnect. This function will build the dispatch loop for network message
    /// types and start up threads to send and recv messages from the peers.
    ///
    /// Returns the PeerInterconnect object that can be used to get network message senders and
    /// shutdown message threads.
    pub fn build(&mut self) -> Result<PeerInterconnect<V>, PeerInterconnectError> {
        let (dispatched_sender, dispatched_receiver) = channel();
        let peers = Arc::new(Mutex::new(BiHashMap::new()));

        let peer_connector = self.peer_connector.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Peer manager connector missing".to_string())
        })?;

        let message_shutdown = self.message_shutdown.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Message shutdown missing".to_string())
        })?;

        let mut network_dispatcher = self.network_dispatcher.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Network dispatcher missing".to_string())
        })?;
        network_dispatcher.set_network_sender(NetworkMessageSender::new(dispatched_sender.clone()));
        let network_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(network_dispatcher)
            .build()
            .map_err(|err| {
                PeerInterconnectError::StartUpError(format!(
                    "Unable to start network dispatch loop: {}",
                    err
                ))
            })?;
        let network_dispatcher_sender = network_dispatch_loop.new_dispatcher_sender();
        let network_dispatcher_shutdown = network_dispatch_loop.shutdown_signaler();

        // start receiver loop
        let message_receiver = self.message_receiver.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Message receiver missing".to_string())
        })?;

        let recv_peers = peers.clone();
        let recv_peer_connector = peer_connector.clone();
        let recv_join_handle = thread::Builder::new()
            .name("PeerInterconnect Receiver".into())
            .spawn(move || {
                loop {
                    // receive messages from peers
                    let envelope = match message_receiver.recv() {
                        Ok(envelope) => envelope,
                        Err(MatrixRecvError::Shutdown) => {
                            info!("Matrix has shutdown");
                            break;
                        }
                        Err(MatrixRecvError::Disconnected) => {
                            error!("Unable to receive message: disconnected");
                            break;
                        }
                        Err(MatrixRecvError::InternalError { context, .. }) => {
                            error!("Unable to receive message: {}", context);
                            break;
                        }
                    };

                    let connection_id = envelope.id();
                    let peer_id = {
                        let mut peers = match recv_peers.lock() {
                            Ok(recv_peers) => recv_peers,
                            Err(_) => {
                                error!("PeerInterconnect state has been poisoned");
                                break;
                            }
                        };

                        let mut peer_id = peers
                            .get_by_value(connection_id)
                            .unwrap_or(&"".to_string())
                            .to_string();

                        // convert connection id to peer id
                        // if peer id is None, fetch peers to see if they have changed
                        if peer_id.is_empty() {
                            *peers = match recv_peer_connector.connection_ids() {
                                Ok(peers) => peers,
                                Err(err) => {
                                    error!("Unable to get peer map: {}", err);
                                    break;
                                }
                            };
                            peer_id = peers
                                .get_by_value(connection_id)
                                .to_owned()
                                .unwrap_or(&"".to_string())
                                .to_string();
                        }
                        peer_id
                    };

                    // If we have the peer, pass message to dispatcher, else print error
                    if !peer_id.is_empty() {
                        let mut network_msg: NetworkMessage =
                            match protobuf::parse_from_bytes(&envelope.payload()) {
                                Ok(msg) => msg,
                                Err(err) => {
                                    error!("Unable to dispatch message: {}", err);
                                    continue;
                                }
                            };

                        trace!(
                            "Received message from {}: {:?}",
                            peer_id,
                            network_msg.get_message_type()
                        );
                        match network_dispatcher_sender.send(
                            network_msg.get_message_type(),
                            network_msg.take_payload(),
                            peer_id.into(),
                        ) {
                            Ok(()) => (),
                            Err((message_type, _, _)) => {
                                error!("Unable to dispatch message of type {:?}", message_type)
                            }
                        }
                    } else {
                        error!("Received message from unknown peer");
                    }
                }
            })
            .map_err(|err| {
                PeerInterconnectError::StartUpError(format!(
                    "Unable to start PeerInterconnect receiver thread {}",
                    err
                ))
            })?;

        let message_sender = self
            .message_sender
            .take()
            .ok_or_else(|| PeerInterconnectError::StartUpError("Already started".to_string()))?;
        let send_join_handle = thread::Builder::new()
            .name("PeerInterconnect Sender".into())
            .spawn(move || {
                loop {
                    // receive message from internal handlers to send over the network
                    let (recipient, payload) = match dispatched_receiver.recv() {
                        Ok(SendRequest::Message { recipient, payload }) => (recipient, payload),
                        Ok(SendRequest::Shutdown) => {
                            info!("Received Shutdown");
                            break;
                        }
                        Err(err) => {
                            error!("Unable to receive message from handlers: {}", err);
                            break;
                        }
                    };
                    // convert recipient (peer_id) to connection_id
                    let connection_id = {
                        let mut peers = match peers.lock() {
                            Ok(recv_peers) => recv_peers,
                            Err(_) => {
                                error!("PeerInterconnect state has been poisoned");
                                break;
                            }
                        };

                        let mut connection_id = peers
                            .get_by_key(&recipient)
                            .to_owned()
                            .unwrap_or(&"".to_string())
                            .to_string();

                        if connection_id.is_empty() {
                            *peers = match peer_connector.connection_ids() {
                                Ok(peers) => peers,
                                Err(err) => {
                                    error!("Unable to get peer map: {}", err);
                                    break;
                                }
                            };

                            connection_id = peers
                                .get_by_key(&recipient)
                                .to_owned()
                                .unwrap_or(&"".to_string())
                                .to_string();
                        }
                        connection_id
                    };

                    // if peer exists, send message over the network
                    if !connection_id.is_empty() {
                        match message_sender.send(connection_id.to_string(), payload) {
                            Ok(_) => (),
                            Err(err) => {
                                error!("Unable to send message to {}", err);
                            }
                        }
                    } else {
                        error!("Cannot send message, unknown peer: {}", recipient);
                    }
                }
            })
            .map_err(|err| {
                PeerInterconnectError::StartUpError(format!(
                    "Unable to start PeerInterconnect sender thread {}",
                    err
                ))
            })?;

        Ok(PeerInterconnect {
            dispatched_sender: dispatched_sender.clone(),
            recv_join_handle,
            send_join_handle,
            shutdown_handle: ShutdownHandle {
                sender: dispatched_sender,
                dispatch_shutdown: network_dispatcher_shutdown,
                matrix_shutdown: message_shutdown,
            },
        })
    }
}

#[derive(Clone)]
pub struct ShutdownHandle<T: 'static>
where
    T: MatrixShutdown,
{
    sender: Sender<SendRequest>,
    dispatch_shutdown: DispatchLoopShutdownSignaler<NetworkMessageType>,
    matrix_shutdown: T,
}

impl<T> ShutdownHandle<T>
where
    T: MatrixShutdown,
{
    /// Sends a shutdown notifications to PeerInterconnect and the associated dipatcher thread and
    /// Matrix
    pub fn shutdown(&self) {
        if self.sender.send(SendRequest::Shutdown).is_err() {
            warn!("Peer Interconnect is no longer running");
        }

        self.dispatch_shutdown.shutdown();
        self.matrix_shutdown.shutdown();
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use protobuf::Message;

    use std::sync::mpsc::Sender;

    use crate::mesh::{Envelope, Mesh};
    use crate::network::connection_manager::{
        AuthorizationResult, Authorizer, AuthorizerError, ConnectionManager,
    };
    use crate::network::dispatch::{DispatchError, Handler, MessageContext, MessageSender, PeerId};
    use crate::network::peer_manager::PeerManager;
    use crate::protos::network::NetworkEcho;
    use crate::transport::{inproc::InprocTransport, Connection, Transport};

    // Verify that the PeerInterconnect properly receives messages from peers, passes them to
    // the dispatcher, and sends messages from the handlers to other peers.
    //
    // PeerInterconnect will receive a message from peer test_peer and pass it to
    // NetworkTestHandler. This handler will validate it came from test_peer. The handler will
    // then send a message to the PeerInterconnect to send the message back to test_peer.
    // This valdiates that messages can be sent and recieved over the PeerInterconnect.
    //
    // This tests also validates that PeerInterconnect can retrieve the list of peers from the
    // PeerManager using the PeerManagerConnector.
    //
    // Finally, verify that PeerInterconnect can be shutdown by calling shutdown_and_wait.
    //
    // 1. Starts up a PeerManager and requests a peer that is running in another thread. Assert
    //    that the peer is properly created.
    //
    // 2. The PeerInterconnect is created with network disaptcher that contains a Handler for
    //    NetworkEcho. This Handler will act like the normal NetworkEcho handle and respond with
    //    the message that was received. If the bytes for "shutdown_string" are received, the
    //    the handler will send a shutdown notifications that will end the test.
    //
    //    The main thread will then block on waiting to receive a shutdown message
    //
    // 3. The peer running in another thread will send a NetworkEcho with the bytes "test_retrieve"
    //    and will wait to recv a NetworkEcho back from the main thread. This would only happen
    //    if the PeerInterconnect received the message and dispatched it to the correct handler.
    //    That Handle then must use the NetworkSender pass the response to the PeerInterconnect.
    //    The PeerInterconnect will then send the message to the Peer.
    //
    //    Before the PeerInterconnect can dispatch the message it has received, it has to find the
    //    Associated peer_id for the connection id that was returned from Mesh. When it receives
    //    the message from the peer thread, it will not have this information, so it must request
    //    the connection_id to peer_id information from the PeerManager. When it receives the
    //    request, it will update its local copy of the peer map and try to find the peer again.
    //
    // 4. After the peer running in the thread has sucessfully received the NetworkEcho response
    //    it will send another NetworkEcho with "shutdown_string" bytes. This will cause the
    //    Handler to send a shutdown notication that will shutdown the test.
    //
    //    If the shutdown message is not received after 2 seconds, the test will fail.
    //
    // 5. The PeerInterconnect, PeerManager, and ConnectionManger is then shutdown.
    #[test]
    fn test_peer_interconnect() {
        let mut transport = Box::new(InprocTransport::default());
        let mut listener = transport
            .listen("inproc://test")
            .expect("Cannot listen for connections");
        let mesh1 = Mesh::new(512, 128);
        let mesh2 = Mesh::new(512, 128);

        // set up thread for the peer
        thread::spawn(move || {
            // accept incoming connection and add it to mesh2
            let conn = listener.accept().expect("Cannot accept connection");
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");

            // send a NetworkEchoMessage
            let message_bytes = echo_to_network_message_bytes(b"test_retrieve".to_vec());
            let envelope = Envelope::new("test_id".to_string(), message_bytes);
            mesh2.send(envelope).expect("Unable to send message");

            // Verify mesh received the same network echo back
            let envelope = mesh2.recv().expect("Cannot receive message");
            let network_msg: NetworkMessage = protobuf::parse_from_bytes(&envelope.payload())
                .expect("Cannot parse NetworkMessage");

            let echo: NetworkEcho = protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();
            assert_eq!(
                network_msg.get_message_type(),
                NetworkMessageType::NETWORK_ECHO
            );

            assert_eq!(echo.get_payload().to_vec(), b"test_retrieve".to_vec());

            // Send a message back to PeerInterconnect that will shutdown the test
            let message_bytes =
                echo_to_network_message_bytes("shutdown_string".as_bytes().to_vec());
            let envelope = Envelope::new("test_id".to_string(), message_bytes);
            mesh2.send(envelope).expect("Cannot send message");
        });

        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_peer")),
            mesh1.get_life_cycle(),
            mesh1.get_sender(),
            transport,
            Some(1),
            None,
        );
        let connector = cm.start().unwrap();
        let mut peer_manager = PeerManager::new(connector, None);
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");
        let (send, recv) = channel();

        let mut dispatcher = Dispatcher::default();
        let handler = NetworkTestHandler::new(send);
        dispatcher.set_handler(Box::new(handler));
        let interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_connector)
            .with_message_receiver(mesh1.get_receiver())
            .with_message_sender(mesh1.get_sender())
            .with_message_shutdown(mesh1.get_matrix_shutdown())
            .with_network_dispatcher(dispatcher)
            .build()
            .expect("Unable to build PeerInterconnect");

        // wait to be told to shutdown, timeout after 2 seconds
        let test_timeout = std::time::Duration::from_secs(2);
        recv.recv_timeout(test_timeout)
            .expect("Failed to receive message");
        peer_manager.shutdown_and_wait();
        cm.shutdown_and_wait();
        interconnect.shutdown_and_wait();
    }

    // Verify that PeerInterconnect can be shutdown after start but without any messages being
    // sent. This test starts up the PeerInterconnect and the associated Connection/PeerManager
    // and then immediately shuts them down.
    #[test]
    fn test_peer_interconnect_shutdown() {
        let transport = Box::new(InprocTransport::default());
        let mesh = Mesh::new(512, 128);

        let mut cm = ConnectionManager::new(
            Box::new(NoopAuthorizer::new("test_peer")),
            mesh.get_life_cycle(),
            mesh.get_sender(),
            transport,
            Some(1),
            None,
        );
        let connector = cm.start().unwrap();
        let mut peer_manager = PeerManager::new(connector, None);
        let peer_connector = peer_manager.start().expect("Cannot start PeerManager");
        let dispatcher = Dispatcher::default();

        let interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_connector)
            .with_message_receiver(mesh.get_receiver())
            .with_message_sender(mesh.get_sender())
            .with_message_shutdown(mesh.get_matrix_shutdown())
            .with_network_dispatcher(dispatcher)
            .build()
            .expect("Unable to build PeerInterconnect");

        peer_manager.shutdown_and_wait();
        cm.shutdown_and_wait();
        interconnect.shutdown_and_wait();
    }

    struct Shutdown {}

    struct NetworkTestHandler {
        shutdown_sender: Sender<Shutdown>,
    }

    impl NetworkTestHandler {
        fn new(shutdown_sender: Sender<Shutdown>) -> Self {
            NetworkTestHandler { shutdown_sender }
        }
    }

    impl Handler for NetworkTestHandler {
        type Source = PeerId;
        type MessageType = NetworkMessageType;
        type Message = NetworkEcho;

        fn match_type(&self) -> Self::MessageType {
            NetworkMessageType::NETWORK_ECHO
        }

        fn handle(
            &self,
            message: NetworkEcho,
            message_context: &MessageContext<Self::Source, NetworkMessageType>,
            network_sender: &dyn MessageSender<Self::Source>,
        ) -> Result<(), DispatchError> {
            let echo_string = String::from_utf8(message.get_payload().to_vec()).unwrap();
            if &echo_string == "shutdown_string" {
                self.shutdown_sender
                    .send(Shutdown {})
                    .expect("Cannot send shutdown");
            } else {
                assert_eq!(message_context.source_peer_id(), "test_peer");
                let echo_bytes = message.write_to_bytes().unwrap();

                let mut network_msg = NetworkMessage::new();
                network_msg.set_message_type(NetworkMessageType::NETWORK_ECHO);
                network_msg.set_payload(echo_bytes);
                let network_msg_bytes = network_msg.write_to_bytes().unwrap();

                network_sender
                    .send(message_context.source_id().clone(), network_msg_bytes)
                    .expect("Cannot send message");
            }

            Ok(())
        }
    }

    fn echo_to_network_message_bytes(echo_bytes: Vec<u8>) -> Vec<u8> {
        let mut echo_message = NetworkEcho::new();
        echo_message.set_payload(echo_bytes);
        let echo_message_bytes = echo_message.write_to_bytes().unwrap();

        let mut network_message = NetworkMessage::new();
        network_message.set_message_type(NetworkMessageType::NETWORK_ECHO);
        network_message.set_payload(echo_message_bytes);
        network_message.write_to_bytes().unwrap()
    }

    struct NoopAuthorizer {
        authorized_id: String,
    }

    impl NoopAuthorizer {
        fn new(id: &str) -> Self {
            Self {
                authorized_id: id.to_string(),
            }
        }
    }

    impl Authorizer for NoopAuthorizer {
        fn authorize_connection(
            &self,
            connection_id: String,
            connection: Box<dyn Connection>,
            callback: Box<
                dyn Fn(AuthorizationResult) -> Result<(), Box<dyn std::error::Error>> + Send,
            >,
        ) -> Result<(), AuthorizerError> {
            (*callback)(AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity: self.authorized_id.clone(),
            })
            .map_err(|err| AuthorizerError(format!("Unable to return result: {}", err)))
        }
    }
}
