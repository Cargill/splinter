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

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::network::dispatch::DispatchMessageSender;
use crate::network::sender::{NetworkMessageSender, SendRequest};
use crate::protos::network::{NetworkMessage, NetworkMessageType};
use crate::transport::matrix::{
    ConnectionMatrixReceiver, ConnectionMatrixRecvError, ConnectionMatrixSender,
};

use super::connector::{PeerLookup, PeerLookupProvider};
use super::error::PeerInterconnectError;

/// PeerInterconnect will receive incoming messages from peers and dispatch them to the
/// NetworkMessageType handlers. It will also receive messages from handlers that need to be
/// sent to other peers.
///
/// When an incoming message is received, the connection_id is converted to a peer_id. The reverse
/// is done for an outgoing message. If a message is received from an unknown connection, the
/// PeerInterconnect will request the current peers from the PeerManager and update the local map
/// of peers.
pub struct PeerInterconnect {
    // sender that will be wrapped in a NetworkMessageSender and given to Dispatchers for sending
    // messages to peers
    dispatched_sender: Sender<SendRequest>,
    recv_join_handle: thread::JoinHandle<()>,
    send_join_handle: thread::JoinHandle<()>,
    shutdown_handle: ShutdownHandle,
}

impl PeerInterconnect {
    /// get a new NetworkMessageSender that can be used to send messages to peers.
    pub fn new_network_sender(&self) -> NetworkMessageSender {
        NetworkMessageSender::new(self.dispatched_sender.clone())
    }

    /// Returns a ShutdownHandle that can be used to shutdown PeerInterconnect
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }

    /// waits for the send and receive thread to shutdown
    pub fn await_shutdown(self) {
        debug!("Shutting down peer interconnect receiver...");
        if let Err(err) = self.send_join_handle.join() {
            error!(
                "Peer interconnect send thread did not shutdown correctly: {:?}",
                err
            );
        };
        debug!("Shutting down peer interconnect receiver (complete)");

        debug!("Shutting down peer interconnect sender...");
        if let Err(err) = self.recv_join_handle.join() {
            error!(
                "Peer interconnect recv thread did not shutdown correctly: {:?}",
                err
            );
        }
        debug!("Shutting down peer interconnect sender (complete)");
    }

    /// Call shutdown on the shutdown handle and then waits for the PeerInterconnect threads to
    /// finish
    pub fn shutdown_and_wait(self) {
        self.shutdown_handle().shutdown();
        self.await_shutdown();
    }
}

#[derive(Default)]
pub struct PeerInterconnectBuilder<T: 'static, U: 'static, P>
where
    T: ConnectionMatrixReceiver,
    U: ConnectionMatrixSender,
    P: PeerLookupProvider + 'static,
{
    // peer lookup provider
    peer_lookup_provider: Option<P>,
    // ConnectionMatrixReceiver to receive messages from peers
    message_receiver: Option<T>,
    // ConnectionMatrixSender to send messages to peers
    message_sender: Option<U>,
    // a Dispatcher with handlers for NetworkMessageTypes
    network_dispatcher_sender: Option<DispatchMessageSender<NetworkMessageType>>,
}

impl<T, U, P> PeerInterconnectBuilder<T, U, P>
where
    T: ConnectionMatrixReceiver,
    U: ConnectionMatrixSender,
    P: PeerLookupProvider + 'static,
{
    /// Creats an empty builder for a PeerInterconnect
    pub fn new() -> Self {
        PeerInterconnectBuilder {
            peer_lookup_provider: None,
            message_receiver: None,
            message_sender: None,
            network_dispatcher_sender: None,
        }
    }

    /// Add a PeerLookupProvider to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `peer_lookup_provider` - a PeerLookupProvider that will be used to facilitate getting the
    ///   peer ids and connection ids for messages.
    pub fn with_peer_connector(mut self, peer_lookup_provider: P) -> Self {
        self.peer_lookup_provider = Some(peer_lookup_provider);
        self
    }

    /// Add a ConnectionMatrixReceiver to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_receiver` - a ConnectionMatrixReceiver that will be used to receive messages
    /// from peers
    pub fn with_message_receiver(mut self, message_receiver: T) -> Self {
        self.message_receiver = Some(message_receiver);
        self
    }

    /// Add a ConnectionMatrixSender to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_sender` - a ConnectionMatrixSender that will be used to send messages to peers
    pub fn with_message_sender(mut self, message_sender: U) -> Self {
        self.message_sender = Some(message_sender);
        self
    }

    /// Add a DispatchMessageSender for NetworkMessageType to PeerInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `network_dispatcher_sender` - a DispatchMessageSender<NetworkMessageType> to dispatch
    ///     NetworkMessages
    pub fn with_network_dispatcher_sender(
        mut self,
        network_dispatcher_sender: DispatchMessageSender<NetworkMessageType>,
    ) -> Self {
        self.network_dispatcher_sender = Some(network_dispatcher_sender);
        self
    }

    /// Build the PeerInterconnect. This function will start up threads to send and recv messages
    /// from the peers.
    ///
    /// Returns the PeerInterconnect object that can be used to get network message senders and
    /// shutdown message threads.
    pub fn build(&mut self) -> Result<PeerInterconnect, PeerInterconnectError> {
        let (dispatched_sender, dispatched_receiver) = channel();
        let peer_lookup_provider = self.peer_lookup_provider.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Peer lookup provider missing".to_string())
        })?;

        let network_dispatcher_sender = self.network_dispatcher_sender.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Network dispatcher sender missing".to_string())
        })?;

        // start receiver loop
        let message_receiver = self.message_receiver.take().ok_or_else(|| {
            PeerInterconnectError::StartUpError("Message receiver missing".to_string())
        })?;

        let recv_peer_lookup = peer_lookup_provider.peer_lookup();
        debug!("Starting peer interconnect receiver");
        let recv_join_handle = thread::Builder::new()
            .name("PeerInterconnect Receiver".into())
            .spawn(move || {
                if let Err(err) = run_recv_loop(
                    &*recv_peer_lookup,
                    message_receiver,
                    network_dispatcher_sender,
                ) {
                    error!("Shutting down peer interconnect recevier: {}", err);
                }
            })
            .map_err(|err| {
                PeerInterconnectError::StartUpError(format!(
                    "Unable to start PeerInterconnect receiver thread {}",
                    err
                ))
            })?;

        let send_peer_lookup = peer_lookup_provider.peer_lookup();
        let message_sender = self
            .message_sender
            .take()
            .ok_or_else(|| PeerInterconnectError::StartUpError("Already started".to_string()))?;
        debug!("Starting peer interconnect sender");
        let send_join_handle = thread::Builder::new()
            .name("PeerInterconnect Sender".into())
            .spawn(move || {
                if let Err(err) =
                    run_send_loop(&*send_peer_lookup, dispatched_receiver, message_sender)
                {
                    error!("Shutting down peer interconnect sender: {}", err);
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
            },
        })
    }
}

fn run_recv_loop<R>(
    peer_connector: &dyn PeerLookup,
    message_receiver: R,
    dispatch_msg_sender: DispatchMessageSender<NetworkMessageType>,
) -> Result<(), String>
where
    R: ConnectionMatrixReceiver + 'static,
{
    let mut connection_id_to_peer_id: HashMap<String, String> = HashMap::new();
    loop {
        // receive messages from peers
        let envelope = match message_receiver.recv() {
            Ok(envelope) => envelope,
            Err(ConnectionMatrixRecvError::Shutdown) => {
                info!("ConnectionMatrix has shutdown");
                break Ok(());
            }
            Err(ConnectionMatrixRecvError::Disconnected) => {
                break Err("Unable to receive message: disconnected".into());
            }
            Err(ConnectionMatrixRecvError::InternalError { context, .. }) => {
                break Err(format!("Unable to receive message: {}", context));
            }
        };

        let connection_id = envelope.id();
        let peer_id = if let Some(peer_id) = connection_id_to_peer_id.get(connection_id) {
            Some(peer_id.to_owned())
        } else if let Some(peer_id) = peer_connector
            .peer_id(connection_id)
            .map_err(|err| format!("Unable to get peer id for {}: {}", connection_id, err))?
        {
            connection_id_to_peer_id.insert(connection_id.to_string(), peer_id.clone());
            Some(peer_id)
        } else {
            None
        };

        // If we have the peer, pass message to dispatcher, else print error
        if let Some(peer_id) = peer_id {
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
            match dispatch_msg_sender.send(
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
            error!(
                "Received message from unknown peer with connection_id {}",
                connection_id
            );
        }
    }
}

fn run_send_loop<S>(
    peer_connector: &dyn PeerLookup,
    receiver: Receiver<SendRequest>,
    message_sender: S,
) -> Result<(), String>
where
    S: ConnectionMatrixSender + 'static,
{
    let mut peer_id_to_connection_id: HashMap<String, String> = HashMap::new();
    loop {
        // receive message from internal handlers to send over the network
        let (recipient, payload) = match receiver.recv() {
            Ok(SendRequest::Message { recipient, payload }) => (recipient, payload),
            Ok(SendRequest::Shutdown) => {
                info!("Received Shutdown");
                break Ok(());
            }
            Err(err) => {
                break Err(format!("Unable to receive message from handlers: {}", err));
            }
        };
        // convert recipient (peer_id) to connection_id
        let connection_id = if let Some(connection_id) = peer_id_to_connection_id.get(&recipient) {
            Some(connection_id.to_owned())
        } else if let Some(connection_id) = peer_connector
            .connection_id(&recipient)
            .map_err(|err| format!("Unable to get connection id for {}: {}", recipient, err))?
        {
            peer_id_to_connection_id.insert(recipient.clone(), connection_id.clone());
            Some(connection_id)
        } else {
            None
        };

        // if peer exists, send message over the network
        if let Some(connection_id) = connection_id {
            // If connection is missing, check with peer manager to see if connection id has
            // changed and try to resend message. Otherwise remove cached connection_id.
            if let Err(err) = message_sender.send(connection_id.to_string(), payload.to_vec()) {
                if let Some(new_connection_id) =
                    peer_connector.connection_id(&recipient).map_err(|err| {
                        format!("Unable to get connection id for {}: {}", recipient, err)
                    })?
                {
                    // if connection_id has changed replace it and try to send again
                    if new_connection_id != connection_id {
                        peer_id_to_connection_id
                            .insert(recipient.clone(), new_connection_id.clone());
                        if let Err(err) = message_sender.send(new_connection_id, payload) {
                            error!("Unable to send message to {}: {}", recipient, err);
                        }
                    }
                } else {
                    error!("Unable to send message to {}: {}", recipient, err);
                    // remove cached connection id, peer has gone away
                    peer_id_to_connection_id.remove(&recipient);
                }
            }
        } else {
            error!("Cannot send message, unknown peer: {}", recipient);
        }
    }
}

#[derive(Clone)]
pub struct ShutdownHandle {
    sender: Sender<SendRequest>,
}

impl ShutdownHandle {
    /// Sends a shutdown notifications to PeerInterconnect and the associated dipatcher thread and
    /// ConnectionMatrix
    pub fn shutdown(&self) {
        if self.sender.send(SendRequest::Shutdown).is_err() {
            warn!("Peer Interconnect is no longer running");
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use protobuf::Message;

    use std::sync::mpsc::{self, Sender};

    use crate::mesh::{Envelope, Mesh};
    use crate::network::connection_manager::{
        AuthorizationResult, Authorizer, AuthorizerError, ConnectionManager,
    };
    use crate::network::dispatch::{
        dispatch_channel, DispatchError, DispatchLoopBuilder, Dispatcher, Handler, MessageContext,
        MessageSender, PeerId,
    };
    use crate::network::peer_manager::{PeerManager, PeerManagerNotification};
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
    //    Handler to send a shutdown notification that will shutdown the test.
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
        let (tx, rx) = mpsc::channel();
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
            assert_eq!(
                network_msg.get_message_type(),
                NetworkMessageType::NETWORK_ECHO
            );

            let echo: NetworkEcho = protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();
            assert_eq!(echo.get_payload().to_vec(), b"test_retrieve".to_vec());

            // Send a message back to PeerInterconnect that will shutdown the test
            let message_bytes =
                echo_to_network_message_bytes("shutdown_string".as_bytes().to_vec());
            let envelope = Envelope::new("test_id".to_string(), message_bytes);
            mesh2.send(envelope).expect("Cannot send message");

            rx.recv().unwrap();

            mesh2.shutdown_signaler().shutdown();
        });

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh1.get_life_cycle())
            .with_matrix_sender(mesh1.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1), "my_id".to_string());
        let peer_connector = peer_manager.start().expect("Cannot start peer_manager");
        let (send, recv) = channel();

        let (dispatcher_sender, dispatcher_receiver) = dispatch_channel();
        let interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_connector.clone())
            .with_message_receiver(mesh1.get_receiver())
            .with_message_sender(mesh1.get_sender())
            .with_network_dispatcher_sender(dispatcher_sender.clone())
            .build()
            .expect("Unable to build PeerInterconnect");

        let mut dispatcher = Dispatcher::new(Box::new(interconnect.new_network_sender()));
        let handler = NetworkTestHandler::new(send);
        dispatcher.set_handler(Box::new(handler));

        let network_dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(dispatcher)
            .with_thread_name("NetworkDispatchLoop".to_string())
            .with_dispatch_channel((dispatcher_sender, dispatcher_receiver))
            .build()
            .expect("Unable to create network dispatch loop");

        let dispatch_shutdown = network_dispatch_loop.shutdown_signaler();

        let mut subscriber = peer_connector
            .subscribe()
            .expect("Unable to get subscriber");

        let peer_ref = peer_connector
            .add_peer_ref("test_peer".to_string(), vec!["test".to_string()])
            .expect("Unable to add peer");

        assert_eq!(peer_ref.peer_id, "test_peer");

        let notification = subscriber.next().expect("Unable to get notification");
        assert_eq!(
            notification,
            PeerManagerNotification::Connected {
                peer: "test_peer".to_string()
            }
        );

        // wait to be told to shutdown, timeout after 60 seconds
        let test_timeout = std::time::Duration::from_secs(60);
        recv.recv_timeout(test_timeout)
            .expect("Failed to receive message");

        // trigger the thread shutdown
        tx.send(()).unwrap();

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        dispatch_shutdown.shutdown();
        mesh1.shutdown_signaler().shutdown();
        interconnect.shutdown_and_wait();
    }

    // Verify that PeerInterconnect can be shutdown after start but without any messages being
    // sent. This test starts up the PeerInterconnect and the associated Connection/PeerManager
    // and then immediately shuts them down.
    #[test]
    fn test_peer_interconnect_shutdown() {
        let transport = Box::new(InprocTransport::default());
        let mesh = Mesh::new(512, 128);

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test_peer")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();
        let mut peer_manager = PeerManager::new(connector, None, Some(1), "my_id".to_string());
        let peer_connector = peer_manager.start().expect("Cannot start PeerManager");
        let (dispatcher_sender, _dispatched_receiver) = dispatch_channel();
        let interconnect = PeerInterconnectBuilder::new()
            .with_peer_connector(peer_connector)
            .with_message_receiver(mesh.get_receiver())
            .with_message_sender(mesh.get_sender())
            .with_network_dispatcher_sender(dispatcher_sender)
            .build()
            .expect("Unable to build PeerInterconnect");

        peer_manager.shutdown_handle().unwrap().shutdown();
        cm.shutdown_signaler().shutdown();
        peer_manager.await_shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
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
