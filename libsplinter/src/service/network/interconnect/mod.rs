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

//! The interconnect module provides the ServiceInterconnect struct, which may be used to route
//! and receive messages based on a service processor's identity.

mod error;
mod service_connector;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::network::dispatch::{ConnectionId, DispatchMessageSender, MessageSender};
use crate::protos::component::{ComponentMessage, ComponentMessageType};
use crate::transport::matrix::{
    ConnectionMatrixReceiver, ConnectionMatrixRecvError, ConnectionMatrixSender,
};

pub use self::error::{ServiceInterconnectError, ServiceLookupError};

/// The ServiceLookup trait provides an interface for looking up details about individual service
/// connections.
pub trait ServiceLookup: Send {
    /// Retrieves the connection ID for a given service ID.
    ///
    /// # Errors
    ///
    /// Returns a ServiceLookupError if the implementation encounters an unexpected error.
    fn connection_id(&self, service_id: &str) -> Result<Option<String>, ServiceLookupError>;

    /// Retrieves the service ID for a given connection ID.
    ///
    /// # Errors
    ///
    /// Returns a ServiceLookupError if the implementation encounters an unexpected error.
    fn service_id(&self, connection_id: &str) -> Result<Option<String>, ServiceLookupError>;
}

/// Provides a SerivceLookup instance.
pub trait ServiceLookupProvider {
    /// Provide a ServiceLookup instance.
    fn service_lookup(&self) -> Box<dyn ServiceLookup>;
}

/// ServiceInterconnect will receive incoming messages from services and dispatch them to the
/// ComponentMessageType handlers. It will also receive messages from handlers that need to be
/// sent to components.
///
/// When an incoming message is received, the connection ID is converted to a service processor
/// identity. The reverse is done for an outgoing message.
pub struct ServiceInterconnect {
    dispatched_sender: Sender<SendRequest>,
    recv_join_handle: thread::JoinHandle<()>,
    send_join_handle: thread::JoinHandle<()>,
    shutdown_handle: ShutdownHandle,
}

impl ServiceInterconnect {
    /// Return a new MessageSender over connections that can be used to send messages to services.
    pub fn new_message_sender(&self) -> impl MessageSender<ConnectionId> {
        ServiceInterconnectMessageSender::new(self.dispatched_sender.clone())
    }

    /// Return a ShutdownHandle that can be used to shutdown ServiceInterconnect
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }

    /// Wait for the send and receive thread to shutdown
    pub fn await_shutdown(self) {
        if let Err(err) = self.send_join_handle.join() {
            error!(
                "Service interconnect send thread did not shutdown correctly: {:?}",
                err
            );
        };

        if let Err(err) = self.recv_join_handle.join() {
            error!(
                "Service interconnect recv thread did not shutdown correctly: {:?}",
                err
            );
        }
    }

    /// Call shutdown on the shutdown handle and then waits for the ServiceInterconnect threads to
    /// finish
    pub fn shutdown_and_wait(self) {
        self.shutdown_handle().shutdown();
        self.await_shutdown();
    }
}

/// Constructs correctly initialized ServiceInterconnect structs.
#[derive(Default)]
pub struct ServiceInterconnectBuilder<T, U, P>
where
    T: ConnectionMatrixReceiver + 'static,
    U: ConnectionMatrixSender + 'static,
    P: ServiceLookupProvider + 'static,
{
    // service lookup provider
    service_lookup_provider: Option<P>,
    // ConnectionMatrixReceiver to receive messages from services
    message_receiver: Option<T>,
    // ConnectionMatrixSender to send messages to services
    message_sender: Option<U>,
    // a Dispatcher with handlers for ComponentMessageTypes
    service_msg_dispatcher_sender:
        Option<DispatchMessageSender<ComponentMessageType, ConnectionId>>,
}

impl<T, U, P> ServiceInterconnectBuilder<T, U, P>
where
    T: ConnectionMatrixReceiver + 'static,
    U: ConnectionMatrixSender + 'static,
    P: ServiceLookupProvider + 'static,
{
    /// Create an empty builder for a ServiceInterconnect
    pub fn new() -> Self {
        ServiceInterconnectBuilder {
            service_lookup_provider: None,
            message_receiver: None,
            message_sender: None,
            service_msg_dispatcher_sender: None,
        }
    }

    /// Add a ServiceLookupProvider to ServiceInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `service_lookup_provider` - a ServiceLookupProvider that will be used to facilitate getting the
    ///   service IDs and connection IDs for messages.
    pub fn with_service_connector(mut self, service_lookup_provider: P) -> Self {
        self.service_lookup_provider = Some(service_lookup_provider);
        self
    }

    /// Add a ConnectionMatrixReceiver to ServiceInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_receiver` - a ConnectionMatrixReceiver that will be used to receive messages from
    ///   services.
    pub fn with_message_receiver(mut self, message_receiver: T) -> Self {
        self.message_receiver = Some(message_receiver);
        self
    }

    /// Add a ConnectionMatrixSender to ServiceInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `message_sender` - a ConnectionMatrixSender that will be used to send messages to services.
    pub fn with_message_sender(mut self, message_sender: U) -> Self {
        self.message_sender = Some(message_sender);
        self
    }

    /// Add a DispatchMessageSender for ComponentMessageType to ServiceInterconnectBuilder
    ///
    /// # Arguments
    ///
    /// * `service_msg_dispatcher_sender` - a DispatchMessageSender to dispatche ComponentMessage
    pub fn with_service_msg_dispatcher_sender(
        mut self,
        service_msg_dispatcher_sender: DispatchMessageSender<ComponentMessageType, ConnectionId>,
    ) -> Self {
        self.service_msg_dispatcher_sender = Some(service_msg_dispatcher_sender);
        self
    }

    /// Build the ServiceInterconnect. This function will start up threads to send and recv
    /// messages from the services.
    ///
    /// Returns the ServiceInterconnect object that can be used to get network message senders and
    /// shutdown message threads.
    pub fn build(&mut self) -> Result<ServiceInterconnect, ServiceInterconnectError> {
        let (dispatched_sender, dispatched_receiver) = channel();

        let service_lookup_provider = self.service_lookup_provider.take().ok_or_else(|| {
            ServiceInterconnectError("Service lookup provider missing".to_string())
        })?;

        let service_msg_dispatcher_sender =
            self.service_msg_dispatcher_sender.take().ok_or_else(|| {
                ServiceInterconnectError("Network dispatcher  sender missing".to_string())
            })?;

        // start receiver loop
        let message_receiver = self
            .message_receiver
            .take()
            .ok_or_else(|| ServiceInterconnectError("Message receiver missing".to_string()))?;

        let recv_service_lookup = service_lookup_provider.service_lookup();
        let recv_join_handle = thread::Builder::new()
            .name("ServiceInterconnect Receiver".into())
            .spawn(move || {
                if let Err(err) = run_recv_loop(
                    &*recv_service_lookup,
                    message_receiver,
                    service_msg_dispatcher_sender,
                ) {
                    error!("Shutting down service interconnect recevier: {}", err);
                }
            })
            .map_err(|err| {
                ServiceInterconnectError(format!(
                    "Unable to start ServiceInterconnect receiver thread: {}",
                    err
                ))
            })?;

        let send_service_lookup = service_lookup_provider.service_lookup();
        let message_sender = self
            .message_sender
            .take()
            .ok_or_else(|| ServiceInterconnectError("Message sender missing".to_string()))?;
        let send_join_handle = thread::Builder::new()
            .name("ServiceInterconnect Sender".into())
            .spawn(move || {
                if let Err(err) =
                    run_send_loop(&*send_service_lookup, dispatched_receiver, message_sender)
                {
                    error!("Shutting down service interconnect sender: {}", err);
                }
            })
            .map_err(|err| {
                ServiceInterconnectError(format!(
                    "Unable to start ServiceInterconnect sender thread: {}",
                    err
                ))
            })?;

        Ok(ServiceInterconnect {
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
    service_connector: &dyn ServiceLookup,
    message_receiver: R,
    dispatch_msg_sender: DispatchMessageSender<ComponentMessageType, ConnectionId>,
) -> Result<(), String>
where
    R: ConnectionMatrixReceiver + 'static,
{
    let mut connection_id_to_service_id: HashMap<String, String> = HashMap::new();
    loop {
        // receive messages from components
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
        let service_id = if let Some(service_id) = connection_id_to_service_id.get(connection_id) {
            Some(service_id.to_owned())
        } else if let Some(service_id) = service_connector
            .service_id(connection_id)
            .map_err(|err| format!("Unable to get service ID for {}: {}", connection_id, err))?
        {
            connection_id_to_service_id.insert(connection_id.to_string(), service_id.clone());
            Some(service_id)
        } else {
            None
        };

        // If we have the service, pass message to dispatcher, else print error
        if let Some(service_id) = service_id {
            let mut component_msg: ComponentMessage =
                match protobuf::parse_from_bytes(&envelope.payload()) {
                    Ok(msg) => msg,
                    Err(err) => {
                        error!("Unable to dispatch message: {}", err);
                        continue;
                    }
                };

            trace!(
                "Received message from {}: {:?}",
                service_id,
                component_msg.get_message_type()
            );

            if let Err((message_type, _, _)) = dispatch_msg_sender.send(
                component_msg.get_message_type(),
                component_msg.take_payload(),
                service_id.into(),
            ) {
                error!("Unable to dispatch message of type {:?}", message_type)
            }
        } else {
            error!("Received message from unknown service");
        }
    }
}

fn run_send_loop<S>(
    service_connector: &dyn ServiceLookup,
    receiver: Receiver<SendRequest>,
    message_sender: S,
) -> Result<(), String>
where
    S: ConnectionMatrixSender + 'static,
{
    let mut service_id_to_connection_id: HashMap<String, String> = HashMap::new();
    loop {
        // receive message from internal handlers to send over the network
        let (recipient, payload) = match receiver.recv() {
            Ok(SendRequest::Message(recipient, payload)) => (recipient, payload),
            Ok(SendRequest::Shutdown) => {
                info!("Received Shutdown");
                break Ok(());
            }
            Err(err) => {
                break Err(format!("Unable to receive message from handlers: {}", err));
            }
        };
        // convert recipient (service_id) to connection_id
        let connection_id =
            if let Some(connection_id) = service_id_to_connection_id.get(&*recipient) {
                Some(connection_id.to_owned())
            } else if let Some(connection_id) = service_connector
                .connection_id(&recipient)
                .map_err(|err| format!("Unable to get connection ID for {}: {}", recipient, err))?
            {
                service_id_to_connection_id.insert(recipient.clone().into(), connection_id.clone());
                Some(connection_id)
            } else {
                None
            };

        // if service exists, send message over the network
        if let Some(connection_id) = connection_id {
            if let Err(err) = message_sender.send(connection_id, payload) {
                error!("Unable to send message to {}: {}", recipient, err);
            }
        } else {
            error!("Cannot send message, unknown service: {}", recipient);
        }
    }
}

enum SendRequest {
    Message(ConnectionId, Vec<u8>),
    Shutdown,
}

#[derive(Clone)]
struct ServiceInterconnectMessageSender {
    sender: Sender<SendRequest>,
}

impl ServiceInterconnectMessageSender {
    fn new(sender: Sender<SendRequest>) -> Self {
        Self { sender }
    }
}

impl MessageSender<ConnectionId> for ServiceInterconnectMessageSender {
    fn send(
        &self,
        recipient: ConnectionId,
        message: Vec<u8>,
    ) -> Result<(), (ConnectionId, Vec<u8>)> {
        self.sender
            .send(SendRequest::Message(recipient, message))
            .map_err(|err| match err.0 {
                SendRequest::Message(recipient, payload) => (recipient, payload),
                SendRequest::Shutdown => unreachable!(), // we didn't send this
            })
    }
}

#[derive(Clone)]
pub struct ShutdownHandle {
    sender: Sender<SendRequest>,
}

impl ShutdownHandle {
    /// Sends a shutdown notifications to ServiceInterconnect and the associated dipatcher thread and
    /// ConnectionMatrix
    pub fn shutdown(&self) {
        if self.sender.send(SendRequest::Shutdown).is_err() {
            warn!("Service Interconnect is no longer running");
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use protobuf::Message;

    use std::cell::RefCell;
    use std::collections::VecDeque;

    use crate::mesh::{Envelope, Mesh};
    use crate::network::connection_manager::{
        AuthorizationResult, Authorizer, AuthorizerError, ConnectionManager,
    };
    use crate::network::dispatch::{
        dispatch_channel, ConnectionId, DispatchError, DispatchLoopBuilder, Dispatcher, Handler,
        MessageContext, MessageSender,
    };
    use crate::protos::service;
    use crate::service::network::{ServiceConnectionManager, ServiceConnectionNotification};
    use crate::transport::{inproc::InprocTransport, Connection, Transport};

    // Verify that the ServiceInterconnect properly receives messages from services, passes them to
    // the dispatcher, and sends messages from the handlers to other services.
    //
    // ServiceInterconnect will receive a message from services test-service and pass it to
    // ComponentTestHandler. This handler will validate it came from test-service. The handler will
    // then send a message to the ServiceInterconnect to send the message back to test-service.
    // This valdiates that messages can be sent and recieved over the ServiceInterconnect.
    //
    // This tests also validates that ServiceInterconnect can retrieve the list of services from
    // the ServiceConnectionManager using the ServiceConnectionManagerConnector.
    //
    // Finally, verify that ServiceInterconnect can be shutdown by calling shutdown_and_wait.
    //
    // 1. Starts up a ServiceConnectionManager and receives a incomming service connection.
    //
    // 2. The ServiceInterconnect is created with component disaptcher that contains a Handler for
    //    SERVICE messages. This Handler will echo the service processor message content back to
    //    the origin.
    //
    //    The main thread will then block on waiting for the remote thread to complete its
    //    assertions.
    //
    // 3. The service running in another thread will send a ServiceProcessorMessage with the bytes
    //    "test_retrieve" and will wait to recv the echoed bytes back from the main thread. This
    //    would only happen if the ServiceInterconnect received the message and dispatched it to
    //    the correct handler.  That Handler then must use the MessageSender pass the response to
    //    the ServiceInterconnect.  The ServiceInterconnect will then send the message to the
    //    Service.
    //
    //    Before the ServiceInterconnect can dispatch the message it has received, it has to find
    //    the associated service_id for the connection id that was returned from Mesh. When it
    //    receives the message from the service thread, it will not have this information, so it
    //    must request the connection_id to service_id information from the
    //    ServiceConnectionManager.  When it receives the request, it will update its local copy of
    //    the service map and try to find the service again.
    //
    // 4. The ServiceInterconnect, Mesh, ServiceConnectionManager, and ConnectionManger is then
    //    shutdown.
    #[test]
    fn test_service_interconnect() {
        let mut transport = InprocTransport::default();
        let mut listener = transport
            .listen("inproc://test")
            .expect("Cannot listen for connections");
        let mesh1 = Mesh::new(512, 128);
        let mesh2 = Mesh::new(512, 128);

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test-service")))
            .with_matrix_life_cycle(mesh1.get_life_cycle())
            .with_matrix_sender(mesh1.get_sender())
            .with_transport(Box::new(transport.clone()))
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let service_conn_mgr = ServiceConnectionManager::builder()
            .with_connector(connector.clone())
            .start()
            .expect("Unable to start service manager");

        let service_connector = service_conn_mgr.service_connector();
        let (sub_tx, sub_rx) = channel();
        service_connector
            .subscribe(sub_tx)
            .expect("Unable to subscribe");

        // set up thread for the service
        let mut remote_inproc = transport.clone();
        let join_handle = thread::spawn(move || {
            // accept incoming connection and add it to mesh2
            let conn = remote_inproc.connect("inproc://test").unwrap();
            mesh2
                .add(conn, "test_id".to_string())
                .expect("Cannot add connection to mesh");

            // send a NetworkEchoMessage
            let message_bytes = create_service_processor_request(b"test_retrieve".to_vec());
            let envelope = Envelope::new("test_id".to_string(), message_bytes);
            mesh2.send(envelope).expect("Unable to send message");

            // Verify mesh received the same network echo back
            let envelope = mesh2.recv().expect("Cannot receive message");
            let mut network_msg: ComponentMessage = protobuf::parse_from_bytes(&envelope.payload())
                .expect("Cannot parse ComponentMessage");

            if network_msg.get_message_type() == ComponentMessageType::COMPONENT_HEARTBEAT {
                // try to get the service message
                let envelope = mesh2.recv().expect("Cannot receive message");
                network_msg = protobuf::parse_from_bytes(&envelope.payload())
                    .expect("Cannot parse ComponentMessage");
            }

            assert_eq!(
                network_msg.get_message_type(),
                ComponentMessageType::SERVICE
            );

            let echo: service::ServiceProcessorMessage =
                protobuf::parse_from_bytes(network_msg.get_payload()).unwrap();

            assert_eq!(echo.get_payload().to_vec(), b"test_retrieve".to_vec());

            mesh2.shutdown_signaler().shutdown();
        });
        let (dispatcher_sender, dispatcher_receiver) = dispatch_channel();
        let interconnect = ServiceInterconnectBuilder::new()
            .with_service_connector(service_conn_mgr.service_connector())
            .with_message_receiver(mesh1.get_receiver())
            .with_message_sender(mesh1.get_sender())
            .with_service_msg_dispatcher_sender(dispatcher_sender.clone())
            .build()
            .expect("Unable to build ServiceInterconnect");

        let message_sender = interconnect.new_message_sender();

        let mut dispatcher: Dispatcher<ComponentMessageType, ConnectionId> =
            Dispatcher::new(Box::new(message_sender));
        let handler = ComponentTestHandler::new(&[b"test_retrieve"]);
        dispatcher.set_handler(Box::new(handler));

        let dispatch_loop = DispatchLoopBuilder::new()
            .with_dispatcher(dispatcher)
            .with_thread_name("ServiceDispatchLoop".to_string())
            .with_dispatch_channel((dispatcher_sender, dispatcher_receiver))
            .build()
            .expect("Unable to create service dispatch loop");

        let dispatch_shutdown = dispatch_loop.shutdown_signaler();

        let conn = listener.accept().expect("Cannot accept connection");
        connector.add_inbound_connection(conn).unwrap();

        let _notification: ServiceConnectionNotification = sub_rx.recv().unwrap();
        // Wait for the remote to finish it's testing
        join_handle.join().unwrap();

        service_conn_mgr.shutdown_and_wait();
        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        dispatch_shutdown.shutdown();
        mesh1.shutdown_signaler().shutdown();
        interconnect.shutdown_and_wait();
    }

    // Verify that ServiceInterconnect can be shutdown after start but without any messages being
    // sent. This test starts up the ServiceInterconnect and the associated
    // Connection/ServiceConnectionManager and then immediately shuts them down.
    #[test]
    fn test_service_interconnect_shutdown() {
        let transport = Box::new(InprocTransport::default());
        let mesh = Mesh::new(512, 128);

        let cm = ConnectionManager::builder()
            .with_authorizer(Box::new(NoopAuthorizer::new("test-service")))
            .with_matrix_life_cycle(mesh.get_life_cycle())
            .with_matrix_sender(mesh.get_sender())
            .with_transport(transport)
            .start()
            .expect("Unable to start Connection Manager");

        let connector = cm.connector();

        let service_conn_mgr = ServiceConnectionManager::builder()
            .with_connector(connector.clone())
            .start()
            .expect("Unable to start service manager");

        let (dispatcher_sender, _) = dispatch_channel();
        let interconnect = ServiceInterconnectBuilder::new()
            .with_service_connector(service_conn_mgr.service_connector())
            .with_message_receiver(mesh.get_receiver())
            .with_message_sender(mesh.get_sender())
            .with_service_msg_dispatcher_sender(dispatcher_sender.clone())
            .build()
            .expect("Unable to build ServiceInterconnect");

        service_conn_mgr.shutdown_and_wait();
        cm.shutdown_signaler().shutdown();
        cm.await_shutdown();
        mesh.shutdown_signaler().shutdown();
        interconnect.shutdown_and_wait();
    }

    fn create_service_processor_request(payload: Vec<u8>) -> Vec<u8> {
        let mut svc_processor_msg = service::ServiceProcessorMessage::new();
        svc_processor_msg.set_payload(payload);

        let mut service_msg = service::ServiceMessage::new();
        service_msg.set_message_type(service::ServiceMessageType::SM_SERVICE_PROCESSOR_MESSAGE);
        service_msg.set_payload(svc_processor_msg.write_to_bytes().unwrap());

        let mut component_msg = ComponentMessage::new();
        component_msg.set_message_type(ComponentMessageType::SERVICE);
        component_msg.set_payload(service_msg.write_to_bytes().unwrap());

        component_msg.write_to_bytes().unwrap()
    }

    struct ComponentTestHandler {
        expected_messages: RefCell<VecDeque<Vec<u8>>>,
    }

    impl ComponentTestHandler {
        fn new(expected_msgs: &[&[u8]]) -> Self {
            Self {
                expected_messages: RefCell::new(
                    expected_msgs.iter().map(|msg| msg.to_vec()).collect(),
                ),
            }
        }
    }

    impl Handler for ComponentTestHandler {
        type Source = ConnectionId;
        type MessageType = ComponentMessageType;
        type Message = crate::protos::service::ServiceMessage;

        fn match_type(&self) -> Self::MessageType {
            ComponentMessageType::SERVICE
        }

        fn handle(
            &self,
            message: Self::Message,
            message_context: &MessageContext<Self::Source, Self::MessageType>,
            network_sender: &dyn MessageSender<Self::Source>,
        ) -> Result<(), DispatchError> {
            assert_eq!(
                service::ServiceMessageType::SM_SERVICE_PROCESSOR_MESSAGE,
                message.get_message_type()
            );

            let service_processor_msg: service::ServiceProcessorMessage =
                protobuf::parse_from_bytes(message.get_payload()).unwrap();

            let expected_msg = self
                .expected_messages
                .borrow_mut()
                .pop_front()
                .expect("No more messages expected");

            assert_eq!(message_context.source_connection_id(), "test-service");
            assert_eq!(expected_msg, service_processor_msg.get_payload().to_vec());

            // Echo back the service procesor message
            let echo_bytes = service_processor_msg.write_to_bytes().unwrap();

            let mut component_msg = ComponentMessage::new();
            component_msg.set_message_type(ComponentMessageType::SERVICE);
            component_msg.set_payload(echo_bytes);
            let component_msg_bytes = component_msg.write_to_bytes().unwrap();

            network_sender
                .send(message_context.source_id().clone(), component_msg_bytes)
                .expect("Cannot send message");

            Ok(())
        }
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
