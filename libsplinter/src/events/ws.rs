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

//! WebSocket Module.
//!
//! Module for establishing WebSocket connections with Splinter Services.
//!
//!```
//! use std::{thread::sleep, time};
//! use splinter::events::{WsResponse, WebSocketClient, Reactor, ParseBytes};
//!
//! let reactor = Reactor::new();
//!
//! let mut ws = WebSocketClient::new(
//!    "http://echo.websocket.org", |ctx, msg: Vec<u8>| {
//!    if let Ok(s) = String::from_utf8(msg.clone()) {
//!         println!("Received {}", s);
//!    } else {
//!       println!("malformed message: {:?}", msg);
//!    };
//!    WsResponse::Text("welcome to earth!!!".to_string())
//! });
//!
//! // Optional callback for when connection is established
//! ws.on_open(|_| {
//!    println!("sending message");
//!    WsResponse::Text("hello, world".to_string())
//! });
//!
//! ws.on_error(move |err, ctx| {
//!     println!("Error!: {:?}", err);
//!     // ws instance can be used to restart websocket
//!     ctx.start_ws().unwrap();
//!     Ok(())
//! });
//!
//! reactor.igniter().start_ws(&ws).unwrap();
//!
//! sleep(time::Duration::from_secs(1));
//! println!("stopping");
//! reactor.shutdown().unwrap();
//! ```

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};

use actix_http::ws;
use awc::ws::{CloseCode, CloseReason, Codec, Frame, Message};
use bytes::Bytes;
use futures::{
    channel::mpsc::{channel, Sender},
    future::Future,
    stream,
    stream::SplitSink,
    SinkExt, StreamExt as FutureStreamExt,
};
use hyper::{self, header, upgrade::Upgraded, Body, Client, Request, StatusCode};
use tokio::time;
use tokio_util::codec::{Decoder, Framed};

use crate::events::{Igniter, ParseError, WebSocketError};

type OnErrorHandle<T> =
    dyn Fn(&WebSocketError, Context<T>) -> Result<(), WebSocketError> + Send + Sync + 'static;

const MAX_FRAME_SIZE: usize = 10_000_000;
const DEFAULT_RECONNECT: bool = false;
const DEFAULT_RECONNECT_LIMIT: u64 = 10;
const DEFAULT_TIMEOUT: u64 = 300; // default timeout if no message is received from server in seconds

/// Wrapper around future created by `WebSocketClient`. In order for
/// the future to run it must be passed to `Igniter::start_ws`
pub struct Listen {
    future: Pin<Box<dyn Future<Output = Result<(), WebSocketError>> + Send + 'static>>,
    sender: Sender<WebSocketClientCmd>,
    running: Arc<AtomicBool>,
}

type ListenHandleFuture =
    Pin<Box<dyn Future<Output = Result<(), WebSocketError>> + Send + 'static>>;

impl Listen {
    pub fn into_shutdown_handle(self) -> (ListenHandleFuture, ShutdownHandle) {
        (
            self.future,
            ShutdownHandle {
                sender: self.sender,
                running: self.running,
            },
        )
    }
}

#[derive(Clone)]
pub struct ShutdownHandle {
    sender: Sender<WebSocketClientCmd>,
    running: Arc<AtomicBool>,
}

impl ShutdownHandle {
    /// Sends shutdown message to websocket
    pub fn shutdown(mut self) -> Result<(), WebSocketError> {
        if self.sender.try_send(WebSocketClientCmd::Stop).is_err() {
            // ignore the error, as the connection may already be closed
        }

        Ok(())
    }

    pub fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

enum WebSocketClientCmd {
    Frame(Frame),
    Stop,
}

/// WebSocket client. Configures Websocket connection and produces `Listen` future.
pub struct WebSocketClient<T: ParseBytes<T> + 'static = Vec<u8>> {
    url: String,
    on_message: Arc<dyn Fn(Context<T>, T) -> WsResponse + Send + Sync + 'static>,
    on_open: Option<Arc<dyn Fn(Context<T>) -> WsResponse + Send + Sync + 'static>>,
    on_error: Option<Arc<OnErrorHandle<T>>>,
    reconnect: bool,
    reconnect_limit: u64,
    timeout: u64,
    additional_headers: HashMap<String, String>,
}

impl<T: ParseBytes<T> + 'static> Clone for WebSocketClient<T> {
    fn clone(&self) -> Self {
        WebSocketClient {
            url: self.url.clone(),
            on_message: self.on_message.clone(),
            on_open: self.on_open.clone(),
            on_error: self.on_error.clone(),
            reconnect: self.reconnect,
            reconnect_limit: self.reconnect_limit,
            timeout: self.timeout,
            additional_headers: self.additional_headers.clone(),
        }
    }
}

impl<T: ParseBytes<T> + 'static> WebSocketClient<T> {
    pub fn new<F>(url: &str, on_message: F) -> Self
    where
        F: Fn(Context<T>, T) -> WsResponse + Send + Sync + 'static,
    {
        Self {
            url: url.to_string(),
            on_message: Arc::new(on_message),
            on_open: None,
            on_error: None,
            reconnect: DEFAULT_RECONNECT,
            reconnect_limit: DEFAULT_RECONNECT_LIMIT,
            timeout: DEFAULT_TIMEOUT,
            additional_headers: HashMap::new(),
        }
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn set_reconnect(&mut self, reconnect: bool) {
        self.reconnect = reconnect
    }

    pub fn set_reconnect_limit(&mut self, reconnect_limit: u64) {
        self.reconnect_limit = reconnect_limit
    }

    pub fn set_timeout(&mut self, timeout: u64) {
        self.timeout = timeout
    }

    pub fn header(&mut self, header: &str, value: String) {
        self.additional_headers.insert(header.into(), value);
    }

    pub fn reconnect(&self) -> bool {
        self.reconnect
    }

    pub fn reconnect_limit(&self) -> u64 {
        self.reconnect_limit
    }

    pub fn timeout(&self) -> u64 {
        self.timeout
    }

    /// Adds optional `on_open` closure. This closer is called after a connection is initially
    /// established with the server, and is used for printing debug information and sending initial
    /// messages to server if necessary.
    pub fn on_open<F>(&mut self, on_open: F)
    where
        F: Fn(Context<T>) -> WsResponse + Send + Sync + 'static,
    {
        self.on_open = Some(Arc::new(on_open));
    }

    /// Adds optional `on_error` closure. This closure would be called when the Websocket has closed due to
    /// an unexpected error. This callback should be used to shutdown any IO resources being used by the
    /// Websocket or to reestablish the connection if appropriate.
    pub fn on_error<F>(&mut self, on_error: F)
    where
        F: Fn(&WebSocketError, Context<T>) -> Result<(), WebSocketError> + Send + Sync + 'static,
    {
        self.on_error = Some(Arc::new(on_error));
    }

    /// Returns `Listen` for WebSocket.
    pub fn listen(&self, mut context: Context<T>) -> Result<Listen, WebSocketError> {
        let url = self.url.clone();
        let (cmd_sender, cmd_receiver) = channel(1);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let on_open = self
            .on_open
            .clone()
            .unwrap_or_else(|| Arc::new(|_| WsResponse::Empty));
        let on_message = self.on_message.clone();

        let timeout = self.timeout;

        let running_connection = running.clone();
        let mut context_connection = context.clone();

        debug!("starting: {}", url);

        let builder = Request::builder();
        let mut request_builder = builder.uri(url);

        for (header, value) in self.additional_headers.iter() {
            request_builder = request_builder.header(header, value);
        }

        let request = request_builder
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "Upgrade")
            .header(header::SEC_WEBSOCKET_VERSION, "13")
            .header(header::SEC_WEBSOCKET_KEY, "13")
            .body(Body::empty())
            .map_err(|err| WebSocketError::RequestBuilderError(format!("{:?}", err)))?;

        #[allow(clippy::cognitive_complexity)]
        let future = async move {
            let response =
                time::timeout(Duration::from_secs(timeout), Client::new().request(request))
                    .await
                    .map_err(move |err| {
                        if let Err(err) = context_connection.try_reconnect() {
                            error!("Context returned an error  {}", err);
                        }

                        running_connection.store(false, Ordering::SeqCst);
                        WebSocketError::ConnectError(format!("Failed to connect: {}", err))
                    })??;

            if response.status() != StatusCode::SWITCHING_PROTOCOLS {
                error!("The server didn't upgrade: {}", response.status());
            }

            debug!("response: {:?}", response);

            let upgraded = response.into_body().on_upgrade().await?;

            let codec = Codec::new().max_size(MAX_FRAME_SIZE).client_mode();
            let framed = codec.framed(upgraded);

            let (mut sink, stream) = framed.split();

            let mut source = stream::select(
                stream.map(|result| WebSocketClientCmd::Frame(result.unwrap())),
                cmd_receiver,
            );

            if handle_response(&mut sink, on_open(context.clone()), running_clone.clone())
                .await
                .is_err()
            {
                return Ok(());
            }

            context.ws_connected();

            while let Some(message) = source.next().await {
                let mut closed = false;
                let status = match message {
                    WebSocketClientCmd::Frame(Frame::Text(msg))
                    | WebSocketClientCmd::Frame(Frame::Binary(msg)) => {
                        let bytes = msg.to_vec();

                        let result = match T::from_bytes(&bytes) {
                            Ok(message) => {
                                handle_response(
                                    &mut sink,
                                    on_message(context.clone(), message),
                                    running_clone.clone(),
                                )
                                .await
                            }
                            Err(parse_error) => {
                                error!("Failed to parse server message {}", parse_error);
                                if let Err(shutdown_error) = do_shutdown(
                                    &mut sink,
                                    CloseCode::Protocol,
                                    running_clone.clone(),
                                )
                                .await
                                {
                                    Err(WebSocketError::ParserError {
                                        parse_error,
                                        shutdown_error: Some(shutdown_error),
                                    })
                                } else {
                                    Err(WebSocketError::ParserError {
                                        parse_error,
                                        shutdown_error: None,
                                    })
                                }
                            }
                        };

                        if let Err(err) = result {
                            ConnectionStatus::UnexpectedClose(err)
                        } else {
                            ConnectionStatus::Open
                        }
                    }
                    WebSocketClientCmd::Frame(Frame::Ping(msg)) => {
                        trace!("Received Ping {:?} sending pong", msg);
                        if let Err(err) = handle_response(
                            &mut sink,
                            WsResponse::Pong(format!("{:?}", msg)),
                            running_clone.clone(),
                        )
                        .await
                        {
                            ConnectionStatus::UnexpectedClose(err)
                        } else {
                            ConnectionStatus::Open
                        }
                    }
                    WebSocketClientCmd::Frame(Frame::Pong(msg)) => {
                        trace!("Received Pong {:?}", msg);
                        ConnectionStatus::Open
                    }
                    WebSocketClientCmd::Frame(Frame::Close(msg)) => {
                        debug!("Received close message {:?}", msg);
                        let result =
                            do_shutdown(&mut sink, CloseCode::Normal, running_clone.clone())
                                .await
                                .map_err(WebSocketError::from);
                        ConnectionStatus::Close(result)
                    }
                    WebSocketClientCmd::Frame(Frame::Continuation(msg)) => {
                        trace!("Received Continuation Frame: {:?}", msg);
                        ConnectionStatus::Open
                    }
                    WebSocketClientCmd::Stop => {
                        closed = true;
                        let result =
                            do_shutdown(&mut sink, CloseCode::Normal, running_clone.clone())
                                .await
                                .map_err(WebSocketError::from);
                        ConnectionStatus::Close(result)
                    }
                };

                if closed {
                    break;
                } else {
                    match status {
                        ConnectionStatus::Open => continue,
                        ConnectionStatus::UnexpectedClose(_original_error) => {
                            if let Err(err) = context.try_reconnect() {
                                error!("Context returned an error  {}", err);
                            }

                            break;
                        }
                        ConnectionStatus::Close(_res) => {
                            if let Err(err) = context.try_reconnect() {
                                error!("Context returned an error  {}", err);
                            }
                            break;
                        }
                    }
                }
            }

            Ok(())
        };

        Ok(Listen {
            future: Box::pin(future),
            sender: cmd_sender,
            running,
        })
    }
}

async fn handle_response(
    wait_sink: &mut SplitSink<Framed<Upgraded, Codec>, ws::Message>,
    res: WsResponse,
    running: Arc<AtomicBool>,
) -> Result<(), WebSocketError> {
    let outgoing = match res {
        WsResponse::Text(msg) => Message::Text(msg),
        WsResponse::Bytes(bytes) => {
            //let bytes = bytes.as_slice().into().clone();
            Message::Binary(Bytes::copy_from_slice(bytes.as_ref()))
        }
        WsResponse::Pong(msg) => Message::Pong(Bytes::copy_from_slice(msg.as_bytes())),
        WsResponse::Close => {
            return do_shutdown(wait_sink, CloseCode::Normal, running)
                .await
                .map_err(WebSocketError::from);
        }
        WsResponse::Empty => return Ok(()),
    };

    if let Err(protocol_error) = wait_sink.send(outgoing).await {
        error!("Error occurred while handling message {:?}", protocol_error);
        if let Err(shutdown_error) = do_shutdown(wait_sink, CloseCode::Protocol, running).await {
            Err(WebSocketError::AbnormalShutdownError {
                protocol_error,
                shutdown_error,
            })
        } else {
            Err(WebSocketError::from(protocol_error))
        }
    } else {
        wait_sink.flush().await?;
        Ok(())
    }
}

async fn do_shutdown(
    blocking_sink: &mut SplitSink<Framed<Upgraded, Codec>, ws::Message>,
    close_code: CloseCode,
    running: Arc<AtomicBool>,
) -> Result<(), ws::ProtocolError> {
    debug!("Sending close to server");

    running.store(false, Ordering::SeqCst);

    if blocking_sink
        .send(Message::Close(Some(CloseReason::from(close_code))))
        .await
        .is_err()
    {
        blocking_sink.close().await
    } else {
        debug!("Socket connection closed successfully");
        blocking_sink.close().await
    }
}

/// Websocket context object. It contains an Igniter pointing
/// to the Reactor on which the websocket future is running and
/// a copy of the WebSocketClient object.
#[derive(Clone)]
pub struct Context<T: ParseBytes<T> + 'static> {
    igniter: Igniter,
    ws: WebSocketClient<T>,
    reconnect_count: u64,
    last_reconnect: SystemTime,
    wait: Duration,
}

impl<T: ParseBytes<T> + 'static> Context<T> {
    pub fn new(igniter: Igniter, ws: WebSocketClient<T>) -> Self {
        Self {
            igniter,
            ws,
            reconnect_count: 0,
            last_reconnect: SystemTime::now(),
            wait: Duration::from_secs(1),
        }
    }

    /// Starts an instance of the Context's websocket.
    pub fn start_ws(&self) -> Result<(), WebSocketError> {
        let listen = self.ws.listen(self.clone())?;
        self.igniter.start_ws_with_listen(listen)
    }

    /// Returns a copy of the igniter used to start the websocket.
    pub fn igniter(&self) -> Igniter {
        self.igniter.clone()
    }

    /// Should called by the ws to inform that the connection was established successfully
    /// the Context resets the wait and reconnect cound to its intial values.
    pub fn ws_connected(&mut self) {
        self.reset_wait();
        self.reset_reconnect_count();
    }

    /// Checks that ws client can reconnect. If it can it attempts to reconnect if it cannot it
    /// calls the on_error function provided by the user and exits.
    pub fn try_reconnect(&mut self) -> Result<(), WebSocketError> {
        // Check that the ws is configure for automatic reconnect attempts and that the number
        // of reconnect attempts hasn't exceeded the maximum configure
        if self.ws.reconnect && self.reconnect_count < self.ws.reconnect_limit {
            self.reconnect()
        } else {
            let error_message = if self.ws.reconnect {
                WebSocketError::ReconnectError(
                    "Cannot connect to ws server. Reached maximum limit of reconnection attempts"
                        .to_string(),
                )
            } else {
                WebSocketError::ConnectError("Cannot connect to ws server".to_string())
            };
            let on_error = self
                .ws
                .on_error
                .clone()
                .unwrap_or_else(|| Arc::new(|_, _| Ok(())));

            self.reset_wait();
            self.reset_reconnect_count();
            on_error(&error_message, self.clone())
        }
    }

    fn reconnect(&mut self) -> Result<(), WebSocketError> {
        // loop until wait time has passed or reactor received shutdown signal
        debug!("Reconnecting in {:?}", self.wait);
        loop {
            // time elapsed since last reconnect attempt
            let elapsed = SystemTime::now()
                .duration_since(self.last_reconnect)
                .unwrap_or(Duration::from_secs(0));

            if elapsed >= self.wait {
                break;
            }

            if !self.igniter.is_reactor_running() {
                return Ok(());
            }
        }

        self.reconnect_count += 1;
        self.last_reconnect = SystemTime::now();

        let new_wait = self.wait.as_secs_f64() * 2.0;

        self.wait = Duration::from_secs_f64(new_wait);

        debug!(
            "Attempting to reconnect. Attempt number {} out of {}",
            self.reconnect_count, self.ws.reconnect_limit
        );

        self.start_ws()
    }

    fn reset_reconnect_count(&mut self) {
        self.reconnect_count = 0
    }

    fn reset_wait(&mut self) {
        self.wait = Duration::from_secs(1)
    }
}

enum ConnectionStatus {
    Open,
    UnexpectedClose(WebSocketError),
    Close(Result<(), WebSocketError>),
}

/// Response object returned by `WebSocket` client callbacks.
#[derive(Debug)]
pub enum WsResponse {
    Empty,
    Close,
    Pong(String),
    Text(String),
    Bytes(Vec<u8>),
}

pub trait ParseBytes<T: 'static>: Send + Sync + Clone {
    fn from_bytes(bytes: &[u8]) -> Result<T, ParseError>;
}

impl ParseBytes<Vec<u8>> for Vec<u8> {
    fn from_bytes(bytes: &[u8]) -> Result<Vec<u8>, ParseError> {
        Ok(bytes.to_vec())
    }
}
