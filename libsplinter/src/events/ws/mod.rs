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

mod connection_status;
mod context;
mod listen;
mod parse_bytes;
mod shutdown_handle;
mod web_socket_client;
mod web_socket_client_cmd;
mod ws_respoonse;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use actix_http::ws;
use awc::ws::{CloseCode, CloseReason, Codec, Message};
use futures::sink::Wait;
use hyper::{self, upgrade::Upgraded};
use tokio::codec::Framed;
use tokio::prelude::*;

use crate::events::WebSocketError;

pub use context::Context;
pub use listen::Listen;
pub use parse_bytes::ParseBytes;
pub use shutdown_handle::ShutdownHandle;
pub use web_socket_client::WebSocketClient;
pub use ws_respoonse::WsResponse;

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
