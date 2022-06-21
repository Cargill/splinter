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

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::events::{Igniter, ParseBytes, WebSocketClient, WebSocketError};

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
