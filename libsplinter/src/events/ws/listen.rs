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

use std::sync::{atomic::AtomicBool, Arc};

use futures::{sync::mpsc::Sender, Future};

use crate::events::WebSocketError;

use super::ShutdownHandle;
use super::WebSocketClientCmd;

/// Wrapper around future created by `WebSocketClient`. In order for
/// the future to run it must be passed to `Igniter::start_ws`
pub struct Listen {
    pub(super) future: Box<dyn Future<Item = (), Error = WebSocketError> + Send + 'static>,
    pub(super) sender: Sender<WebSocketClientCmd>,
    pub(super) running: Arc<AtomicBool>,
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
