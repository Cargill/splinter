// Copyright 2018-2020 Cargill Incorporated
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

use std::any::Any;

use super::{ConnectionId, PeerId};

/// The Message Context
///
/// The message context provides information about an incoming message beyond its parsed bytes.  It
/// includes the source peer id, the message type, the original bytes, and potentially other,
/// future items.
pub struct MessageContext<Source, MT> {
    source_id: Source,
    message_type: MT,
    message_bytes: Vec<u8>,
    parent_context: Option<Box<dyn Any + Send>>,
}

impl<Source, MT> MessageContext<Source, MT> {
    pub(super) fn new(message_type: MT, message_bytes: Vec<u8>, source_id: Source) -> Self {
        Self {
            message_type,
            message_bytes,
            source_id,
            parent_context: None,
        }
    }

    /// The Message Type.
    ///
    /// This is the message type that determined which handler to execute on receipt of this
    /// message.
    pub fn message_type(&self) -> &MT {
        &self.message_type
    }

    /// The raw message bytes.
    pub fn message_bytes(&self) -> &[u8] {
        &self.message_bytes
    }

    pub fn source_id(&self) -> &Source {
        &self.source_id
    }

    /// Add an item to the parent context.
    pub(super) fn set_parent_context(&mut self, parent_context: Box<dyn Any + Send>) {
        self.parent_context = Some(parent_context);
    }

    /// Get an item of a given type in the context, if it exists.
    pub fn get_parent_context<T: 'static>(&self) -> Option<&T> {
        self.parent_context
            .as_ref()
            .and_then(|boxed| boxed.downcast_ref())
    }
}

impl<MT> MessageContext<PeerId, MT> {
    /// The Source Peer ID.
    ///
    /// This is the peer id of the original sender of the message
    pub fn source_peer_id(&self) -> &str {
        &self.source_id
    }
}

impl<MT> MessageContext<ConnectionId, MT> {
    /// The Source Connection ID.
    ///
    /// This is the connection id of the original sender of the message
    pub fn source_connection_id(&self) -> &str {
        &self.source_id
    }
}
