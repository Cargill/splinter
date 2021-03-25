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
//

use std::collections::HashMap;
use std::convert::Into;
use std::convert::TryFrom;
use std::default::Default;
use std::io;
use std::sync::mpsc::{channel, Sender};
use std::thread;

use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use zmq::{Context, Socket};

use crate::transport::{
    AcceptError, ConnectError, Connection, DisconnectError, ListenError, Listener, RecvError,
    SendError, Transport,
};

const POLL_TIMEOUT: i64 = 10;

/// Message types that are passed from ZmqConnections to
/// the ZmqListeners to facilitate peer to peer
/// communication.
///
/// Messages sent by ZmqConnections are multipart payloads.
/// Each segment of the payload is delimited by a null frame.
/// The payload format used by a ZmqConnection sending a message
/// to a peer is as follows:
///
/// |  partner_id |  null  |  message  |  null  |  message_type  |
///
/// `partner_id` is the `socket_id` of the ZmqConnection the message is
/// intended for. `socket_id`s are generated by the ZmqListener.
///
/// `message` is the body of the payload.
///
/// `message_type` is the string representation of the message type
/// serialized into bytes.
///
/// The router sockets used by the ZmqListener prepend an additional 2 frames
/// to messages sent by ZmqConnections. As such, messages sent to
/// and received by the ZmqListener by ZmqConnections have the
/// following format:
///
/// |  sender_id  | null | partner_id | null  |  message  |  null  |  message_type |
///
/// `sender_id` is a unique socket ID generated by the router, and
/// used to identify to ZmqConnection sending the message.
///
/// The router sockets used by the ZmqListener consume the first two
/// frames of all messages sent by a ZmqListener. As such, ZmqConnections
/// receive payloads that are formatted as follows.
///
/// |  partner_id  |  null  |  message  | null  |  message_type  |
///
#[derive(Debug)]
enum MessageType {
    /// Request that a connection be paired with a
    /// another connection by the router.
    ///
    RequestConnection,

    /// Response to `RequestConnection` returned by
    /// ZmqListener.
    ConnectionAccepted,

    /// Request sent by ZmqConnection requesting any
    /// pending messages from matched peer.
    RequestData,

    /// ZmqListener response to `RequestData`, indicating
    /// that no data is available from matched peer.
    NoDataAvailable,

    /// ZmqListener response to `RequestData`, indicating
    /// data is available from matched peer. Data is stored
    /// in message frame of this response.
    DataAvailable,

    /// Request from ZmqConnection containing message in
    /// message frame for matched peer.
    SendingMessage,

    /// Response from ZmqListener to `SendingMessage`, indicating
    /// message was received and forwarded to matched peer.
    MessageReceived,
}

impl TryFrom<Vec<u8>> for MessageType {
    type Error = String;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let msg = if let Ok(m) = String::from_utf8(value.to_vec()) {
            m
        } else {
            return Err("Could not serialize bytes".to_string());
        };

        match msg.as_ref() {
            "CONNECT" => Ok(MessageType::RequestConnection),
            "CONNECTION_ACCEPTED" => Ok(MessageType::ConnectionAccepted),
            "REQUEST_DATA" => Ok(MessageType::RequestData),
            "DATA_AVAILABLE" => Ok(MessageType::DataAvailable),
            "NO_DATA_AVAILABLE" => Ok(MessageType::NoDataAvailable),
            "SENDING_MESSAGE" => Ok(MessageType::SendingMessage),
            "MESSAGE_RECEIVED" => Ok(MessageType::MessageReceived),
            _ => Err("Invalid Message Type".to_string()),
        }
    }
}

impl From<MessageType> for Vec<u8> {
    fn from(msg_type: MessageType) -> Self {
        match msg_type {
            MessageType::RequestConnection => "CONNECT".into(),
            MessageType::ConnectionAccepted => "CONNECTION_ACCEPTED".into(),
            MessageType::RequestData => "REQUEST_DATA".into(),
            MessageType::NoDataAvailable => "NO_DATA_AVAILABLE".into(),
            MessageType::DataAvailable => "DATA_AVAILABLE".into(),
            MessageType::SendingMessage => "SENDING_MESSAGE".into(),
            MessageType::MessageReceived => "MESSAGE_RECEIVED".into(),
        }
    }
}

/// Payload router thread inside
/// ZmqListener receives from
/// ZmqConnections.
#[derive(Debug)]
struct RouterPayload {
    pub socket_id: Vec<u8>,
    pub partner_id: Vec<u8>,
    pub message: Vec<u8>,
    pub message_type: MessageType,
}

#[derive(Debug, Clone)]
pub enum ZmqError {
    ListenerError(String),
    SendError(String),
    RecvError(String),
    MalformedMessageType(String),
    PollingError(String),
    DisconnectError(String),
    RouterThreadJoinError(String),
}

pub struct ZmqTransport {
    context: Context,
}

impl Default for ZmqTransport {
    fn default() -> Self {
        ZmqTransport {
            context: Context::new(),
        }
    }
}

impl Transport for ZmqTransport {
    fn accepts(&self, address: &str) -> bool {
        address.starts_with("zmq:")
    }

    fn connect(&mut self, endpoint: &str) -> Result<Box<dyn Connection>, ConnectError> {
        if !self.accepts(endpoint) {
            return Err(ConnectError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                endpoint
            )));
        }

        let subprotocol = &endpoint[4..];
        let address = if subprotocol.contains("://") {
            subprotocol.to_string()
        } else {
            format!("tcp://{}", subprotocol)
        };
        Ok(Box::new(ZmqConnection::connect(&self.context, &address)?))
    }

    fn listen(&mut self, bind: &str) -> Result<Box<dyn Listener>, ListenError> {
        if !self.accepts(bind) {
            return Err(ListenError::ProtocolError(format!(
                "Invalid protocol \"{}\"",
                bind
            )));
        }

        let subprotocol = &bind[4..];
        let address = if subprotocol.contains("://") {
            subprotocol.to_string()
        } else {
            format!("tcp://{}", subprotocol)
        };
        Ok(Box::new(ZmqListener::start(self.context.clone(), address)?))
    }
}

pub struct ZmqConnection {
    socket: EventSocket,
    partner_id: Vec<u8>,
    endpoint: String,
}

impl ZmqConnection {
    pub fn connect(context: &Context, endpoint: &str) -> Result<Self, ConnectError> {
        let socket = context.socket(zmq::REQ).map_err(|err| {
            ConnectError::ProtocolError(format!("Could not create zmq REQ socket: {:?}", err))
        })?;

        socket.connect(endpoint).map_err(|err| {
            ConnectError::ProtocolError(format!("Failed to connect socket to backend: {:?}", err))
        })?;

        let payload = vec![
            vec![],
            "".into(),
            vec![],
            "".into(),
            MessageType::RequestConnection.into(),
        ];

        socket.send_multipart(payload, 0).map_err(|err| {
            ConnectError::ProtocolError(format!("Failed to send connection payload: {:?}", err))
        })?;

        let payload = socket.recv_multipart(0).map_err(|err| {
            ConnectError::ProtocolError(format!("Failed to recv connection response: {:?}", err))
        })?;

        Ok(ZmqConnection {
            socket: EventSocket::new(socket),
            partner_id: payload[0].clone(),
            endpoint: endpoint.to_string(),
        })
    }
}

impl Connection for ZmqConnection {
    fn send(&mut self, message: &[u8]) -> Result<(), SendError> {
        let payload = vec![
            self.partner_id.clone(),
            "".into(),
            message.to_vec(),
            "".into(),
            MessageType::SendingMessage.into(),
        ];

        self.socket
            .inner()
            .send_multipart(payload, 0)
            .map_err(|err| {
                SendError::ProtocolError(format!("Failed to send payload: {:?}", err))
            })?;

        let message = self.socket.inner().recv_multipart(0).map_err(|err| {
            SendError::ProtocolError(format!("Failed to receive acknowledgement: {:?}", err))
        })?;

        if let Ok(MessageType::MessageReceived) = MessageType::try_from(message[4].clone()) {
            Ok(())
        } else {
            Err(SendError::ProtocolError(
                "Failed to receive acknowledgement".into(),
            ))
        }
    }

    fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
        loop {
            let payload = vec![
                self.partner_id.clone(),
                "".into(),
                Vec::new(),
                "".into(),
                MessageType::RequestData.into(),
            ];

            let poll_result = self
                .socket
                .inner()
                .poll(zmq::POLLOUT, POLL_TIMEOUT)
                .map_err(|err| {
                    RecvError::ProtocolError(format!("Failed to poll socket {:?}", err))
                })?;

            if poll_result > 0 {
                self.socket
                    .inner()
                    .send_multipart(payload, 0)
                    .map_err(|err| {
                        RecvError::ProtocolError(format!("Failed to request data {:?}", err))
                    })?;

                let response = self.socket.inner().recv_multipart(0).map_err(|err| {
                    RecvError::ProtocolError(format!(
                        "Failed while receiving response {:?} {}",
                        err, self.endpoint
                    ))
                })?;

                if let Ok(MessageType::DataAvailable) = MessageType::try_from(response[4].clone()) {
                    return Ok(response[2].clone());
                }
            }
        }
    }

    fn remote_endpoint(&self) -> String {
        format!("zmq:{}", self.endpoint)
    }

    fn local_endpoint(&self) -> String {
        format!("zmq:{}", self.endpoint)
    }

    fn disconnect(&mut self) -> Result<(), DisconnectError> {
        self.socket
            .inner()
            .disconnect(&self.endpoint)
            .map_err(|err| {
                DisconnectError::ProtocolError(format!(
                    "An error occurred while attempting to disconnect socket {:?}",
                    err
                ))
            })
    }

    fn evented(&self) -> &dyn Evented {
        &self.socket
    }
}

pub struct EventSocket {
    socket: Socket,
}

impl EventSocket {
    pub fn new(socket: Socket) -> Self {
        EventSocket { socket }
    }

    pub fn inner(&self) -> &Socket {
        &self.socket
    }
}

impl Evented for EventSocket {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        let fd = self.socket.get_fd()?;
        EventedFd(&fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        let fd = self.socket.get_fd()?;
        EventedFd(&fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        let fd = self.socket.get_fd()?;
        EventedFd(&fd).deregister(poll)
    }
}

pub struct ZmqListener {
    context: Context,
    endpoint: String,
    sender: Sender<()>,
    join_handle: thread::JoinHandle<Result<(), ZmqError>>,
}

impl ZmqListener {
    fn start(context: Context, endpoint: String) -> Result<Self, ListenError> {
        let frontend = context.socket(zmq::ROUTER).map_err(|err| {
            ListenError::ProtocolError(format!("Could not create zmq ROUTER socket: {:?}", err))
        })?;

        frontend.bind(&endpoint).map_err(|err| {
            ListenError::ProtocolError(format!(
                "Failed to bind ROUTER socket to {}: {:?}",
                endpoint, err
            ))
        })?;

        let backend = context.socket(zmq::ROUTER).map_err(|err| {
            ListenError::ProtocolError(format!("Could not create zmq ROUTER socket: {:?}", err))
        })?;

        backend.bind("inproc://backend").map_err(|err| {
            ListenError::ProtocolError(format!(
                "Failed to bind ROUTER socket to inproc://backend: {:?}",
                err
            ))
        })?;

        let (sender, recv) = channel();

        let frontend_endpoint = endpoint.clone();

        let join_handle = thread::spawn(move || -> Result<(), ZmqError> {
            let mut unmatched_clients: Vec<Vec<u8>> = Vec::new();
            let mut unmatched_workers: Vec<Vec<u8>> = Vec::new();
            let mut message_queue: HashMap<Vec<u8>, Vec<Vec<u8>>> = HashMap::new();

            loop {
                let mut items = [
                    frontend.as_poll_item(zmq::POLLIN),
                    backend.as_poll_item(zmq::POLLIN),
                ];

                zmq::poll(&mut items, POLL_TIMEOUT)
                    .map_err(|err| ZmqError::PollingError(format!("{:?}", err)))?;

                // Check for messages from frontend sockets
                if items[0].is_readable() {
                    let payload = match Self::recv_msg(&frontend) {
                        Ok(res) => res,
                        Err(err) => {
                            debug!("Recv error: {:?}", err);
                            continue;
                        }
                    };

                    match payload.message_type {
                        MessageType::RequestConnection => {
                            if unmatched_workers.is_empty()
                                && unmatched_clients.iter().all(|x| *x != payload.socket_id)
                            {
                                unmatched_clients.push(payload.socket_id.clone());
                            } else {
                                let worker_id = unmatched_workers.pop().unwrap();

                                Self::send_msg(
                                    &frontend,
                                    &payload.socket_id,
                                    &worker_id,
                                    &[],
                                    MessageType::ConnectionAccepted,
                                )?;

                                Self::send_msg(
                                    &backend,
                                    &worker_id,
                                    &payload.socket_id,
                                    &[],
                                    MessageType::ConnectionAccepted,
                                )?;
                            }
                        }
                        MessageType::RequestData => {
                            if let Some(queue) = message_queue.get_mut(&payload.socket_id) {
                                if let Some(msg) = queue.pop() {
                                    Self::send_msg(
                                        &frontend,
                                        &payload.socket_id,
                                        &payload.partner_id,
                                        &msg,
                                        MessageType::DataAvailable,
                                    )?;
                                } else {
                                    Self::send_msg(
                                        &frontend,
                                        &payload.socket_id,
                                        &payload.partner_id,
                                        &[],
                                        MessageType::NoDataAvailable,
                                    )?;
                                }
                            } else {
                                Self::send_msg(
                                    &frontend,
                                    &payload.socket_id,
                                    &payload.partner_id,
                                    &[],
                                    MessageType::NoDataAvailable,
                                )?;
                            }
                        }
                        MessageType::SendingMessage => {
                            if message_queue.contains_key(&payload.partner_id) {
                                message_queue
                                    .get_mut(&payload.partner_id)
                                    .unwrap()
                                    .push(payload.message.clone());
                            } else {
                                message_queue.insert(
                                    payload.partner_id.clone(),
                                    vec![payload.message.clone()],
                                );
                            }
                            Self::send_msg(
                                &frontend,
                                &payload.socket_id,
                                &payload.partner_id,
                                &[],
                                MessageType::MessageReceived,
                            )?;
                        }
                        _ => {
                            debug!("Unhandled message type {:?}", payload.message_type);
                        }
                    }
                }

                // Check for messages from backend sockets
                if items[1].is_readable() {
                    let payload = match Self::recv_msg(&backend) {
                        Ok(res) => res,
                        Err(err) => {
                            debug!("Recv error: {:?}", err);
                            continue;
                        }
                    };

                    match payload.message_type {
                        MessageType::RequestConnection => {
                            if unmatched_clients.is_empty()
                                && unmatched_workers.iter().all(|x| *x != payload.socket_id)
                            {
                                unmatched_workers.push(payload.socket_id.clone());
                            } else {
                                let client_id = unmatched_clients.pop().unwrap();

                                Self::send_msg(
                                    &frontend,
                                    &client_id,
                                    &payload.socket_id,
                                    &[],
                                    MessageType::ConnectionAccepted,
                                )?;

                                Self::send_msg(
                                    &backend,
                                    &payload.socket_id,
                                    &client_id,
                                    &[],
                                    MessageType::ConnectionAccepted,
                                )?;
                            }
                        }
                        MessageType::RequestData => {
                            if let Some(queue) = message_queue.get_mut(&payload.socket_id) {
                                if let Some(msg) = queue.pop() {
                                    Self::send_msg(
                                        &backend,
                                        &payload.socket_id,
                                        &payload.partner_id,
                                        &msg,
                                        MessageType::DataAvailable,
                                    )?;
                                } else {
                                    Self::send_msg(
                                        &backend,
                                        &payload.socket_id,
                                        &payload.partner_id,
                                        &[],
                                        MessageType::NoDataAvailable,
                                    )?;
                                }
                            } else {
                                Self::send_msg(
                                    &backend,
                                    &payload.socket_id,
                                    &payload.partner_id,
                                    &[],
                                    MessageType::NoDataAvailable,
                                )?;
                            }
                        }
                        MessageType::SendingMessage => {
                            if message_queue.contains_key(&payload.partner_id) {
                                message_queue
                                    .get_mut(&payload.partner_id)
                                    .unwrap()
                                    .push(payload.message.clone());
                            } else {
                                message_queue.insert(
                                    payload.partner_id.clone(),
                                    vec![payload.message.clone()],
                                );
                            }
                            Self::send_msg(
                                &backend,
                                &payload.socket_id,
                                &payload.partner_id,
                                &[],
                                MessageType::MessageReceived,
                            )?;
                        }
                        _ => {
                            debug!("Unhandled message type {:?}", payload.message_type);
                        }
                    }
                }

                if let Ok(()) = recv.try_recv() {
                    debug!("Shutting down router");
                    break;
                }
            }

            frontend
                .disconnect(&frontend_endpoint)
                .map_err(|err| ZmqError::DisconnectError(format!("{:?}", err)))?;

            backend
                .disconnect("inproc://backend")
                .map_err(|err| ZmqError::DisconnectError(format!("{:?}", err)))?;

            Ok(())
        });

        Ok(ZmqListener {
            context,
            sender,
            join_handle,
            endpoint,
        })
    }

    pub fn stop(self) -> Result<(), ZmqError> {
        self.sender
            .send(())
            .map_err(|err| ZmqError::DisconnectError(format!("{:?}", err)))?;

        self.join_handle
            .join()
            .map_err(|err| ZmqError::RouterThreadJoinError(format!("{:?}", err)))?
    }

    fn send_msg(
        socket: &Socket,
        socket_id: &[u8],
        partner_id: &[u8],
        message: &[u8],
        message_type: MessageType,
    ) -> Result<(), ZmqError> {
        socket
            .send_multipart(
                vec![
                    socket_id.to_vec(),
                    "".into(),
                    partner_id.to_vec(),
                    "".into(),
                    message.to_vec(),
                    "".into(),
                    message_type.into(),
                ],
                0,
            )
            .map_err(|err| ZmqError::SendError(format!("{:?}", err)))
    }

    fn recv_msg(socket: &Socket) -> Result<RouterPayload, ZmqError> {
        let payload = socket
            .recv_multipart(0)
            .map_err(|err| ZmqError::RecvError(format!("{:?}", err)))?;

        let message_type =
            MessageType::try_from(payload[6].clone()).map_err(ZmqError::MalformedMessageType)?;

        Ok(RouterPayload {
            socket_id: payload[0].clone(),
            partner_id: payload[2].clone(),
            message: payload[4].clone(),
            message_type,
        })
    }
}

impl Listener for ZmqListener {
    fn accept(&mut self) -> Result<Box<dyn Connection>, AcceptError> {
        let connection =
            ZmqConnection::connect(&self.context, "inproc://backend").map_err(|err| {
                AcceptError::ProtocolError(format!("Failed to connect to backend: {:?}", err))
            })?;

        Ok(Box::new(connection))
    }

    fn endpoint(&self) -> String {
        format!("zmq:{}", self.endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::tests;

    #[test]
    fn test_accepts() {
        let transport = ZmqTransport::default();

        assert!(transport.accepts("zmq:127.0.0.1:8080"));
        assert!(transport.accepts("zmq:tcp://127.0.0.1:8080"));
        assert!(transport.accepts("zmq:udp://127.0.0.1:8080"));
        assert!(!transport.accepts("127.0.0.1:8080"));
    }

    #[test]
    fn test_transport() {
        let transport = ZmqTransport::default();

        tests::test_transport(transport, "zmq:127.0.0.1:8080");
    }

    #[test]
    #[ignore]
    fn test_poll() {
        let transport = ZmqTransport::default();

        tests::test_poll(transport, "zmq:tcp://127.0.0.1:8081");
    }
}
