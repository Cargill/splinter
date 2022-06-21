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

use crossbeam_channel::Sender;
use futures::Future;

use crate::events::ws::{Context, Listen, ParseBytes, WebSocketClient};
use crate::events::{ReactorError, WebSocketError};

use super::ReactorMessage;

/// The Igniter is a channel that allows for communication with a Reactor runtime
#[derive(Clone)]
pub struct Igniter {
    pub(super) sender: Sender<ReactorMessage>,
    pub(super) reactor_running: Arc<AtomicBool>,
}

impl Igniter {
    pub fn start_ws<T: ParseBytes<T>>(
        &self,
        ws: &WebSocketClient<T>,
    ) -> Result<(), WebSocketError> {
        let context = Context::new(self.clone(), ws.clone());
        self.sender
            .send(ReactorMessage::StartWs(ws.listen(context)?))
            .map_err(|err| {
                WebSocketError::ListenError(format!("Failed to start ws {}: {}", ws.url(), err))
            })
    }

    pub fn send(
        &self,
        req: Box<dyn Future<Item = (), Error = ()> + Send + 'static>,
    ) -> Result<(), ReactorError> {
        self.sender
            .send(ReactorMessage::HttpRequest(req))
            .map_err(|err| {
                ReactorError::RequestSendError(format!("Failed to send request to reactor {}", err))
            })
    }

    pub fn start_ws_with_listen(&self, listen: Listen) -> Result<(), WebSocketError> {
        self.sender
            .send(ReactorMessage::StartWs(listen))
            .map_err(|err| WebSocketError::ListenError(format!("Failed to start ws {}", err)))
    }

    pub fn is_reactor_running(&self) -> bool {
        self.reactor_running.load(Ordering::SeqCst)
    }
}
