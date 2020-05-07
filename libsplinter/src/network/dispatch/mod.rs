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

//! Methods for Dispatching and Handling Messages.

mod context;
mod r#loop;
mod peer;
mod proto;

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::hash::Hash;

pub use context::MessageContext;
pub use r#loop::{
    dispatch_channel, DispatchLoop, DispatchLoopBuilder, DispatchLoopError,
    DispatchLoopShutdownSignaler, DispatchMessageReceiver, DispatchMessageSender,
};

/// A wrapper for a PeerId.
///
/// This type constrains a dispatcher to peer-specific messages
#[derive(Debug, Clone, Default)]
pub struct PeerId(String);

impl std::ops::Deref for PeerId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for PeerId {
    fn from(s: String) -> PeerId {
        PeerId(s)
    }
}

impl From<&str> for PeerId {
    fn from(s: &str) -> PeerId {
        PeerId(s.into())
    }
}

impl From<PeerId> for String {
    fn from(peer_id: PeerId) -> String {
        peer_id.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A wrapper for Connection Id
///
/// The type constrains a dispatcher to connection-specific messages
#[derive(Debug, Clone, Default)]
pub struct ConnectionId(String);

impl std::ops::Deref for ConnectionId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for ConnectionId {
    fn from(s: String) -> ConnectionId {
        ConnectionId(s)
    }
}

impl From<&str> for ConnectionId {
    fn from(s: &str) -> ConnectionId {
        ConnectionId(s.into())
    }
}

impl From<ConnectionId> for String {
    fn from(connection_id: ConnectionId) -> String {
        connection_id.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A Handler for a network message.
pub trait Handler: Send {
    type Source;
    type MessageType: Hash + Eq + Debug + Clone;
    type Message: FromMessageBytes;

    /// Handles a given message
    ///
    /// # Errors
    ///
    /// Any issues that occur during processing of the message will result in a DispatchError.
    fn handle(
        &self,
        message: Self::Message,
        message_context: &MessageContext<Self::Source, Self::MessageType>,
        network_sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError>;

    /// Return the message type value that this handler requires to execute;
    fn match_type(&self) -> Self::MessageType;
}

/// Converts bytes into a concrete message instance
pub trait FromMessageBytes: Any + Sized {
    /// Converts the given bytes into the target type
    ///
    /// # Errors
    ///
    /// Any issues that occur during deserialization will result in a DispatchError.
    fn from_message_bytes(message_bytes: &[u8]) -> Result<Self, DispatchError>;
}

/// A container for the raw bytes of a message.
///
/// This is useful for handlers that don't deserialize the bytes via this process.  For example, a
/// handler that forwards the messages may utilize this as a message type.
#[derive(Debug, Clone)]
pub struct RawBytes {
    bytes: Vec<u8>,
}

impl RawBytes {
    /// Unwraps the value.
    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    /// Returns a reference to the bytes
    ///
    /// Note, this same value may be returned by using `as_ref()`:
    ///
    ///     # use splinter::network::dispatch::RawBytes;
    ///     let raw_bytes = RawBytes::from("Value".as_bytes());
    ///     assert_eq!(raw_bytes.bytes(), raw_bytes.as_ref());
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl From<&[u8]> for RawBytes {
    fn from(source: &[u8]) -> Self {
        RawBytes {
            bytes: source.to_vec(),
        }
    }
}

impl AsRef<[u8]> for RawBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl FromMessageBytes for RawBytes {
    fn from_message_bytes(message_bytes: &[u8]) -> Result<Self, DispatchError> {
        Ok(RawBytes::from(message_bytes))
    }
}

/// Dispatch Errors
///
/// These errors may occur when handling a dispatched message.
#[derive(Debug, PartialEq)]
pub enum DispatchError {
    /// An error occurred during message deserialization.
    DeserializationError(String),
    /// An error occurred during message serialization.
    SerializationError(String),
    /// An message was dispatched with an unknown type.
    UnknownMessageType(String),
    /// An error occurred while a handler was trying to send a message.
    NetworkSendError((String, Vec<u8>)),
    /// An error occurred while a handler was executing.
    HandleError(String),
    /// if no network sender is set
    MissingNetworkSender,
}

impl std::error::Error for DispatchError {}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DispatchError::DeserializationError(msg) => {
                write!(f, "unable to deserialize message: {}", msg)
            }
            DispatchError::SerializationError(msg) => {
                write!(f, "unable to serialize message: {}", msg)
            }
            DispatchError::UnknownMessageType(msg) => write!(f, "unknown message type: {}", msg),
            DispatchError::NetworkSendError((recipient, _)) => {
                write!(f, "unable to send message to receipt {}", recipient)
            }
            DispatchError::HandleError(msg) => write!(f, "unable to handle message: {}", msg),
            DispatchError::MissingNetworkSender => write!(f, "missing network sender"),
        }
    }
}

/// A sender for outgoing messages.
///
/// The message sender trait can used by Handlers to send messages based on the received messages.
/// The handler can use this to send any number of messages.
pub trait MessageSender<R>: Send {
    /// Send the given message bytes to the specified recipient.
    ///
    /// # Error
    ///
    /// If an error occurs, return the intended recipient and message bytes.
    fn send(&self, reciptient: R, message: Vec<u8>) -> Result<(), (R, Vec<u8>)>;
}

/// Dispatches messages to handlers.
///
/// The dispatcher routes messages of a specific message type to one of a set of handlers that have
/// been supplied via the `set_handler` function.  It owns a `Sender` for sending messages on a
/// network, which is provided to the handlers. The handlers may use the sender for replying to or
/// broadcasting messages, as needed.
///
/// These messages are run in the same thread as the dispatch function is called. Any asynchronous
/// activity done by a handler must be managed by the handler.  These asynchronous operations must
/// return success for the handler immediately, as the expectation is that the dispatcher should
/// not block the current thread.
///
/// Message Types (MT) merely need to implement Hash, Eq and Debug (for unknown message type
/// results). Beyond that, there are no other requirements.
pub struct Dispatcher<MT, Source = PeerId>
where
    Source: 'static,
    MT: Any + Hash + Eq + Debug + Clone,
{
    handlers: HashMap<MT, HandlerWrapper<Source, MT>>,
    network_sender: Box<dyn MessageSender<Source>>,
}

impl<MT, Source> Dispatcher<MT, Source>
where
    Source: 'static,
    MT: Any + Hash + Eq + Debug + Clone,
{
    /// Creates a Dispatcher
    ///
    /// Creates a dispatcher with a given `NetworkSender` to supply to handlers when they are
    /// executed.
    pub fn new(network_sender: Box<dyn MessageSender<Source>>) -> Self {
        Dispatcher {
            handlers: HashMap::new(),
            network_sender,
        }
    }

    /// Set a handler for a given Message Type.
    ///
    /// This sets a handler on the dispatcher that will trigger based on its `match_type` value.
    /// Only one handler may exist for the value of the handler's `match_type` implementation.  If
    /// a user wishes to run a series handlers, they must supply a single handler that composes the
    /// series.
    pub fn set_handler<T>(
        &mut self,
        handler: Box<dyn Handler<Source = Source, MessageType = MT, Message = T>>,
    ) where
        T: FromMessageBytes,
    {
        self.handlers.insert(
            handler.match_type(),
            HandlerWrapper {
                inner: Box::new(move |message_bytes, message_context, network_sender| {
                    let message = FromMessageBytes::from_message_bytes(message_bytes)?;
                    handler.handle(message, message_context, network_sender)
                }),
            },
        );
    }

    /// Dispatch a message by type.
    ///
    /// This dispatches a message (in raw byte form) as a given message type.  The message will be
    /// handled by a handler that has been set previously via `set_handler`, if one exists.
    ///
    /// Errors
    ///
    /// A DispatchError is returned if either there is no handler for the given message type, or an
    /// error occurs while handling the messages (e.g. the message cannot be deserialized).
    pub fn dispatch(
        &self,
        source_id: Source,
        message_type: &MT,
        message_bytes: Vec<u8>,
    ) -> Result<(), DispatchError> {
        let message_context = MessageContext::new(message_type.clone(), message_bytes, source_id);
        self.execute(message_context)
    }

    /// Dispatch a message by type, including a parent context.
    ///
    /// This dispatches a message (in raw byte form) as a given message type.  The message will be
    /// handled by a handler that has been set previously via `set_handler`, if one exists.
    ///
    /// Errors
    ///
    /// A DispatchError is returned if either there is no handler for the given message type, or an
    /// error occurs while handling the messages (e.g. the message cannot be deserialized).
    pub fn dispatch_with_parent_context(
        &self,
        source_id: Source,
        message_type: &MT,
        message_bytes: Vec<u8>,
        parent_context: Box<dyn Any + Send>,
    ) -> Result<(), DispatchError> {
        let mut message_context =
            MessageContext::new(message_type.clone(), message_bytes, source_id);
        message_context.set_parent_context(parent_context);

        self.execute(message_context)
    }

    fn execute(&self, ctx: MessageContext<Source, MT>) -> Result<(), DispatchError> {
        self.handlers
            .get(ctx.message_type())
            .ok_or_else(|| {
                DispatchError::UnknownMessageType(format!(
                    "No handler for type {:?}",
                    ctx.message_type(),
                ))
            })
            .and_then(|handler| handler.handle(ctx.message_bytes(), &ctx, &*self.network_sender))
    }
}

/// A function that handles inbound message bytes.
type InnerHandler<Source, MT> = Box<
    dyn Fn(
            &[u8],
            &MessageContext<Source, MT>,
            &dyn MessageSender<Source>,
        ) -> Result<(), DispatchError>
        + Send,
>;

/// The HandlerWrapper provides a typeless wrapper for typed Handler instances.
struct HandlerWrapper<Source, MT>
where
    MT: Hash + Eq + Debug + Clone,
{
    inner: InnerHandler<Source, MT>,
}

impl<Source, MT> HandlerWrapper<Source, MT>
where
    MT: Hash + Eq + Debug + Clone,
{
    fn handle(
        &self,
        message_bytes: &[u8],
        message_context: &MessageContext<Source, MT>,
        network_sender: &dyn MessageSender<Source>,
    ) -> Result<(), DispatchError> {
        (*self.inner)(message_bytes, message_context, network_sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use protobuf::Message;

    use crate::mesh::Mesh;
    use crate::network::sender;
    use crate::network::Network;
    use crate::protos::network::{NetworkEcho, NetworkMessageType};

    /// Verify that messages can be dispatched to handlers via the trait.
    ///
    /// This test does the following:
    ///
    /// * Create a Dispatcher
    /// * Add a handler implemented as a struct with the Handler trait
    /// * Dispatch a message of the expected type and verify that it was called
    #[test]
    fn dispatch_to_handler() {
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();

        let mut dispatcher = Dispatcher::new(Box::new(network_sender));

        let handler = NetworkEchoHandler::default();
        let echos = handler.echos.clone();

        dispatcher.set_handler(Box::new(handler));

        let mut outgoing_message = NetworkEcho::new();
        outgoing_message.set_payload(b"test_dispatcher".to_vec());
        let outgoing_message_bytes = outgoing_message.write_to_bytes().unwrap();

        assert_eq!(
            Ok(()),
            dispatcher.dispatch(
                "TestPeer".into(),
                &NetworkMessageType::NETWORK_ECHO,
                outgoing_message_bytes
            )
        );

        assert_eq!(
            vec!["test_dispatcher".to_string()],
            echos.lock().unwrap().clone()
        );
    }

    /// Verify that a dispatcher can be moved to a thread.
    ///
    /// This test does the following:
    ///
    /// * Create a Dispatcher in the main thread
    /// * Add a handler implemented as a struct with the Handler trait
    /// * Spawn a thread and move the dispatcher to this thread
    /// * Dispatch a message of the expected type in the spawned thread
    /// * Join the thread and verify the dispatched message was handled
    #[test]
    fn move_dispatcher_to_thread() {
        let mesh1 = Mesh::new(1, 1);
        let network1 = Network::new(mesh1.clone(), 0).unwrap();

        let network_message_queue = sender::Builder::new()
            .with_network(network1.clone())
            .build()
            .expect("Unable to create queue");
        let network_sender = network_message_queue.new_network_sender();
        let mut dispatcher = Dispatcher::new(Box::new(network_sender));

        let handler = NetworkEchoHandler::default();
        let echos = handler.echos.clone();
        dispatcher.set_handler(Box::new(handler));

        std::thread::spawn(move || {
            let mut outgoing_message = NetworkEcho::new();
            outgoing_message.set_payload(b"thread_echo".to_vec());
            let outgoing_message_bytes = outgoing_message.write_to_bytes().unwrap();

            assert_eq!(
                Ok(()),
                dispatcher.dispatch(
                    "TestPeer".into(),
                    &NetworkMessageType::NETWORK_ECHO,
                    outgoing_message_bytes
                )
            );
        })
        .join()
        .unwrap();

        assert_eq!(
            vec!["thread_echo".to_string()],
            echos.lock().unwrap().clone()
        );
    }

    #[derive(Default)]
    struct NetworkEchoHandler {
        echos: Arc<Mutex<Vec<String>>>,
    }

    impl Handler for NetworkEchoHandler {
        type Source = PeerId;
        type MessageType = NetworkMessageType;
        type Message = NetworkEcho;

        fn match_type(&self) -> Self::MessageType {
            NetworkMessageType::NETWORK_ECHO
        }

        fn handle(
            &self,
            message: NetworkEcho,
            _message_context: &MessageContext<Self::Source, NetworkMessageType>,
            _: &dyn MessageSender<Self::Source>,
        ) -> Result<(), DispatchError> {
            let echo_string = String::from_utf8(message.get_payload().to_vec()).unwrap();
            self.echos.lock().unwrap().push(echo_string);
            Ok(())
        }
    }
}
