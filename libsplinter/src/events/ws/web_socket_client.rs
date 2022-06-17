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

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use awc::ws::{CloseCode, Codec, Frame};
use futures::{
    future::{self, Either},
    sync::mpsc::channel,
    Future,
};
use hyper::{self, header, Body, Client, Request, StatusCode};
use tokio::codec::Decoder;
use tokio::prelude::*;

use crate::events::{ParseBytes, WebSocketError, WsResponse};

use super::{
    do_shutdown, handle_response, web_socket_client_cmd::WebSocketClientCmd, ConnectionStatus,
    Context, Listen, OnErrorHandle,
};

use super::DEFAULT_RECONNECT;
use super::DEFAULT_RECONNECT_LIMIT;
use super::DEFAULT_TIMEOUT;
use super::MAX_FRAME_SIZE;

/// WebSocket client. Configures Websocket connection and produces `Listen` future.
pub struct WebSocketClient<T: ParseBytes<T> + 'static = Vec<u8>> {
    url: String,
    authorization: String,
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
            authorization: self.authorization.clone(),
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
    pub fn new<F>(url: &str, authorization: &str, on_message: F) -> Self
    where
        F: Fn(Context<T>, T) -> WsResponse + Send + Sync + 'static,
    {
        Self {
            url: url.to_string(),
            authorization: authorization.to_string(),
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

    pub fn authorization(&self) -> String {
        self.authorization.clone()
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

    pub fn set_authorization(&mut self, authorization: &str) {
        self.authorization = authorization.to_string();
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
        F: Fn(WebSocketError, Context<T>) -> Result<(), WebSocketError> + Send + Sync + 'static,
    {
        self.on_error = Some(Arc::new(on_error));
    }

    pub fn get_on_error(&self) -> Option<Arc<OnErrorHandle<T>>> {
        match &self.on_error {
            Some(arc) => Some(Arc::clone(arc)),
            None => None,
        }
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

    pub fn get_on_reconnect(
        &self,
    ) -> Option<Arc<dyn Fn(&mut WebSocketClient<T>) + Send + Sync + 'static>> {
        match &self.on_reconnect {
            Some(arc) => Some(Arc::clone(arc)),
            None => None,
        }
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
        let on_stream_error = self
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
        let mut request_builder = builder
            .uri(url)
            .header("Authorization", &self.authorization);

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
                            WebSocketError::ConnectError(format!(
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
                                        ConnectionStatus::UnexpectedClose(original_error) => {
                                            if let Err(err) =
                                                on_stream_error(original_error, context.clone())
                                            {
                                                error!("Failed to call on_error: {}", err);
                                            }

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
