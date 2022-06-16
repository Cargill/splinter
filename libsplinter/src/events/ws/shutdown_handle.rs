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

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use futures::sync::mpsc::Sender;

use crate::events::WebSocketError;

use super::web_socket_client_cmd::WebSocketClientCmd;

#[derive(Clone)]
pub struct ShutdownHandle {
    pub(super) sender: Sender<WebSocketClientCmd>,
    pub(super) running: Arc<AtomicBool>,
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
