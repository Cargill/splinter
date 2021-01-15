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
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};

use actix_http::ws;
use awc::ws::{CloseCode, CloseReason, Codec, Frame, Message};
use futures::{
    future::{self, Either},
    sink::Wait,
    sync::mpsc::{channel, Sender},
    Future,
};
use hyper::{self, header, upgrade::Upgraded, Body, Client, Request, StatusCode};
use tokio::codec::{Decoder, Framed};
use tokio::prelude::*;

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
    future: Box<dyn Future<Item = (), Error = WebSocketError> + Send + 'static>,
    sender: Sender<WebSocketClientCmd>,
    running: Arc<AtomicBool>,
}

impl Listen {
    pub fn into_shutdown_handle(
        self,
    ) -> (
        Box<dyn Future<Item = (), Error = WebSocketError> + Send + 'static>,
        ShutdownHandle,
    ) {
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
    on_reconnect: Option<Arc<dyn Fn(&mut WebSocketClient<T>) + Send + Sync + 'static>>,
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
            on_reconnect: self.on_reconnect.clone(),
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
            on_reconnect: None,
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

    pub fn set_url(&mut self, url: &str) {
        self.url = url.to_string();
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

    /// Adds optional `on_reconnect` closure. This closure will be called each time the websocket
    /// attempts to reconnect to the server. It's intended to allow the websocket client properties
    /// to be modified before attempting a reconnect or to allow additional checks to be performed
    /// before reconnecting.
    pub fn on_reconnect<F>(&mut self, on_reconnect: F)
    where
        F: Fn(&mut WebSocketClient<T>) + Send + Sync + 'static,
    {
        self.on_reconnect = Some(Arc::new(on_reconnect));
    }

    /// Returns `Listen` for WebSocket.
    pub fn listen(&self, mut context: Context<T>) -> Result<Listen, WebSocketError> {
        let url = self.url.clone();
        let reconnect = self.reconnect;
        let (cmd_sender, cmd_receiver) = channel(1);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let on_open = self
            .on_open
            .clone()
            .unwrap_or_else(|| Arc::new(|_| WsResponse::Empty));
        let on_message = self.on_message.clone();
        let on_error = self
            .on_error
            .clone()
            .unwrap_or_else(|| Arc::new(|_, _| Ok(())));

        let mut context_timeout = context.clone();
        let timeout = self.timeout;

        let running_connection = running.clone();
        let mut context_connection = context.clone();
        let connection_failed_context = context.clone();

        debug!("starting: {}", url);

        let mut builder = Request::builder();
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

        let future = Box::new(
            Client::new()
                .request(request)
                .and_then(move |res| {
                    if res.status() != StatusCode::SWITCHING_PROTOCOLS {
                        error!("The server didn't upgrade: {}", res.status());
                        if let Err(err) = on_error(
                            &WebSocketError::ConnectError(format!(
                            "Received status code {:?} while attempting to establish a connection"
                        , res.status())),
                            connection_failed_context,
                        ) {
                            error!("Failed to establish a connection {:?}", err);
                        }
                    }
                    debug!("response: {:?}", res);

                    res.into_body().on_upgrade()
                })
                .timeout(Duration::from_secs(timeout))
                .map_err(move |err| {
                    // If not running anymore, don't try reconnecting
                    if running_connection.load(Ordering::SeqCst) {
                        if let Err(err) = context_connection.try_reconnect() {
                            error!("Context returned an error  {}", err);
                        }

                        running_connection.store(false, Ordering::SeqCst);
                    }
                    WebSocketError::ConnectError(format!("Failed to connect: {}", err))
                })
                .and_then(move |upgraded| {
                    let codec = Codec::new().max_size(MAX_FRAME_SIZE).client_mode();
                    let framed = codec.framed(upgraded);
                    let (sink, stream) = framed.split();
                    let mut blocking_sink = sink.wait();

                    let source = stream
                        .timeout(Duration::from_secs(timeout))
                        .map_err(move |err| {
                            error!("Connection timeout: {}", err);

                            if let Err(err) = context_timeout.try_reconnect() {
                                error!("Context returned an error  {}", err);
                            }

                            WebSocketError::ListenError("Connection timeout".to_string())
                        })
                        .map(WebSocketClientCmd::Frame)
                        .select(cmd_receiver.map_err(|_| {
                            WebSocketError::ListenError(
                                "All shutdown handles have been dropped".into(),
                            )
                        }));

                    if let Err(_err) = handle_response(
                        &mut blocking_sink,
                        on_open(context.clone()),
                        running_clone.clone(),
                    ) {
                        return Either::A(future::ok(()));
                    }

                    // We're connected
                    context.ws_connected();
                    Either::B(
                        source
                            .take_while(move |message| {
                                let mut closed = false;
                                let status = match message {
                                    WebSocketClientCmd::Frame(Frame::Text(msg))
                                    | WebSocketClientCmd::Frame(Frame::Binary(msg)) => {
                                        let bytes = if let Some(bytes) = msg {
                                            bytes.to_vec()
                                        } else {
                                            Vec::new()
                                        };
                                        let result = T::from_bytes(&bytes)
                                            .map_err(|parse_error| {
                                                error!(
                                                    "Failed to parse server message {}",
                                                    parse_error
                                                );
                                                if let Err(protocol_error) = do_shutdown(
                                                    &mut blocking_sink,
                                                    CloseCode::Protocol,
                                                    running_clone.clone(),
                                                ) {
                                                    WebSocketError::ParserError {
                                                        parse_error,
                                                        shutdown_error: Some(protocol_error),
                                                    }
                                                } else {
                                                    WebSocketError::ParserError {
                                                        parse_error,
                                                        shutdown_error: None,
                                                    }
                                                }
                                            })
                                            .and_then(|message| {
                                                handle_response(
                                                    &mut blocking_sink,
                                                    on_message(context.clone(), message),
                                                    running_clone.clone(),
                                                )
                                            });

                                        if let Err(err) = result {
                                            ConnectionStatus::UnexpectedClose(err)
                                        } else {
                                            ConnectionStatus::Open
                                        }
                                    }
                                    WebSocketClientCmd::Frame(Frame::Ping(msg)) => {
                                        trace!("Received Ping {} sending pong", msg);
                                        if let Err(err) = handle_response(
                                            &mut blocking_sink,
                                            WsResponse::Pong(msg.to_string()),
                                            running_clone.clone(),
                                        ) {
                                            ConnectionStatus::UnexpectedClose(err)
                                        } else {
                                            ConnectionStatus::Open
                                        }
                                    }
                                    WebSocketClientCmd::Frame(Frame::Pong(msg)) => {
                                        trace!("Received Pong {}", msg);
                                        ConnectionStatus::Open
                                    }
                                    WebSocketClientCmd::Frame(Frame::Close(_)) => {
                                        if !reconnect {
                                            let result = do_shutdown(
                                                &mut blocking_sink,
                                                CloseCode::Normal,
                                                running_clone.clone(),
                                            )
                                            .map_err(WebSocketError::from);
                                            ConnectionStatus::Close(result)
                                        } else {
                                            ConnectionStatus::Close(Ok(()))
                                        }
                                    }
                                    WebSocketClientCmd::Stop => {
                                        closed = true;
                                        let result = do_shutdown(
                                            &mut blocking_sink,
                                            CloseCode::Normal,
                                            running_clone.clone(),
                                        )
                                        .map_err(WebSocketError::from);
                                        ConnectionStatus::Close(result)
                                    }
                                };

                                if closed {
                                    future::ok(false)
                                } else {
                                    match status {
                                        ConnectionStatus::Open => future::ok(true),
                                        ConnectionStatus::UnexpectedClose(_original_error) => {
                                            if let Err(err) = context.try_reconnect() {
                                                error!("Context returned an error  {}", err);
                                            }

                                            future::ok(false)
                                        }
                                        ConnectionStatus::Close(_res) => {
                                            if let Err(err) = context.try_reconnect() {
                                                error!("Context returned an error  {}", err);
                                            }
                                            future::ok(false)
                                        }
                                    }
                                }
                            })
                            .for_each(|_| future::ok(())),
                    )
                }),
        );

        Ok(Listen {
            future,
            sender: cmd_sender,
            running,
        })
    }
}

fn handle_response(
    wait_sink: &mut Wait<stream::SplitSink<Framed<Upgraded, Codec>>>,
    res: WsResponse,
    running: Arc<AtomicBool>,
) -> Result<(), WebSocketError> {
    let outgoing = match res {
        WsResponse::Text(msg) => Message::Text(msg),
        WsResponse::Bytes(bytes) => Message::Binary(bytes.as_slice().into()),
        WsResponse::Pong(msg) => Message::Pong(msg),
        WsResponse::Close => {
            return do_shutdown(wait_sink, CloseCode::Normal, running).map_err(WebSocketError::from)
        }
        WsResponse::Empty => return Ok(()),
    };
    wait_sink
        .send(outgoing)
        .and_then(|_| wait_sink.flush())
        .map_err(|protocol_error| {
            error!("Error occurred while handling message {:?}", protocol_error);
            if let Err(shutdown_error) = do_shutdown(wait_sink, CloseCode::Protocol, running) {
                WebSocketError::AbnormalShutdownError {
                    protocol_error,
                    shutdown_error,
                }
            } else {
                WebSocketError::from(protocol_error)
            }
        })
}

fn do_shutdown(
    blocking_sink: &mut Wait<stream::SplitSink<Framed<Upgraded, Codec>>>,
    close_code: CloseCode,
    running: Arc<AtomicBool>,
) -> Result<(), ws::ProtocolError> {
    debug!("Sending close to server");

    running.store(false, Ordering::SeqCst);
    blocking_sink
        .send(Message::Close(Some(CloseReason::from(close_code))))
        .and_then(|_| blocking_sink.flush())
        .and_then(|_| {
            debug!("Socket connection closed successfully");
            blocking_sink.close()
        })
        .or_else(|_| blocking_sink.close())
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
            let on_reconnect = self
                .ws
                .on_reconnect
                .clone()
                .unwrap_or_else(|| Arc::new(|_| ()));

            on_reconnect(&mut self.ws);

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
                .unwrap_or_else(|_| Duration::from_secs(0));

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
