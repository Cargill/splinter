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

// To allow the NetworkMessageSender to not make decissions about the threading model, any channel
// that is used must have the following Receiver trait implemented, then the receiver end of the
// channel can be passed to the NetworkMessageSender.
mod crossbeam;
mod error;
#[cfg(test)]
pub mod mock;
mod mpsc;

use std::time::Duration;

pub use super::channel::error::{RecvError, RecvTimeoutError, SendError, TryRecvError};

pub trait Receiver<T>: Send {
    fn recv(&self) -> Result<T, RecvError>;
    fn try_recv(&self) -> Result<T, TryRecvError>;
    fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError>;
}

// To allow the NetworkMessageSender to not make decissions about the threading model, any channel
// that is used must have the following Sender trait implemented, then the send end of the channel
// can be passed to a Handler.
pub trait Sender<T>: Send {
    fn send(&self, t: T) -> Result<(), SendError>;
    fn box_clone(&self) -> Box<dyn Sender<T>>;
}

impl<T> Clone for Box<dyn Sender<T>> {
    fn clone(&self) -> Box<dyn Sender<T>> {
        self.box_clone()
    }
}
