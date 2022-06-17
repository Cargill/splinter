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

//! WebSocket Module.
//!
//! Module for establishing WebSocket connections with Splinter Services.
//!
//!``` no_run
//! use std::{thread::sleep, time};
//! use splinter::events::{WsResponse, WebSocketClient, Reactor, ParseBytes};
//!
//! let reactor = Reactor::new();
//!
//! let mut ws = WebSocketClient::new(
//!    "http://echo.websocket.org", "Bearer token", |ctx, msg: Vec<u8>| {
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

mod listen;
mod parse_bytes;
mod shutdown_handle;
mod web_socket_client;
mod web_socket_client_cmd;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};

use actix_http::ws;
use awc::ws::{CloseCode, CloseReason, Codec, Message};
use futures::sink::Wait;
use hyper::{self, upgrade::Upgraded};
use tokio::codec::Framed;
use tokio::prelude::*;

use crate::events::{Igniter, WebSocketError};

pub use listen::Listen;
pub use parse_bytes::ParseBytes;
pub use shutdown_handle::ShutdownHandle;
pub use web_socket_client::WebSocketClient;

type OnErrorHandle<T> =
    dyn Fn(WebSocketError, Context<T>) -> Result<(), WebSocketError> + Send + Sync + 'static;

const MAX_FRAME_SIZE: usize = 10_000_000;
const DEFAULT_RECONNECT: bool = false;
const DEFAULT_RECONNECT_LIMIT: u64 = 10;
const DEFAULT_TIMEOUT: u64 = 300; // default timeout if no message is received from server in seconds

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

        if self.ws.reconnect() && self.reconnect_count < self.ws.reconnect_limit() {
            let on_reconnect = self
                .ws
                .get_on_reconnect()
                .clone()
                .unwrap_or_else(|| Arc::new(|_| ()));

            on_reconnect(&mut self.ws);

            self.reconnect()
        } else {
            let error_message = if self.ws.reconnect() {
                WebSocketError::ReconnectError(
                    "Cannot connect to ws server. Reached maximum limit of reconnection attempts"
                        .to_string(),
                )
            } else {
                WebSocketError::ConnectError("Cannot connect to ws server".to_string())
            };
            let on_error = self
                .ws
                .get_on_error()
                .clone()
                .unwrap_or_else(|| Arc::new(|_, _| Ok(())));

            self.reset_wait();
            self.reset_reconnect_count();
            on_error(error_message, self.clone())
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
            self.reconnect_count,
            self.ws.reconnect_limit()
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
