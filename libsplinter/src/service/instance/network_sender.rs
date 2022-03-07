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

use super::{ServiceMessageContext, ServiceSendError};

/// The ServiceNetworkSender trait allows a service to send its own messages, such as replies to
/// the original message or forwarding the message to other services on the same circuit.  It does
/// not expose the circuit information directly.
pub trait ServiceNetworkSender: Send {
    /// Send the message bytes to the given recipient (another service)
    fn send(&self, recipient: &str, message: &[u8]) -> Result<(), ServiceSendError>;

    /// Send the message bytes to the given recipient (another service) and await the reply.  This
    /// function blocks until the reply is returned.
    fn send_and_await(&self, recipient: &str, message: &[u8]) -> Result<Vec<u8>, ServiceSendError>;

    /// Send the message bytes back to the origin specified in the given message context.
    fn reply(
        &self,
        message_origin: &ServiceMessageContext,
        message: &[u8],
    ) -> Result<(), ServiceSendError>;

    /// Clone this instance into Boxed, dynamic trait
    fn clone_box(&self) -> Box<dyn ServiceNetworkSender>;

    /// Send the message bytes to the given recipient (another service) with a configurable
    /// message sender
    fn send_with_sender(
        &mut self,
        recipient: &str,
        message: &[u8],
        sender: &str,
    ) -> Result<(), ServiceSendError>;
}

impl Clone for Box<dyn ServiceNetworkSender> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}
