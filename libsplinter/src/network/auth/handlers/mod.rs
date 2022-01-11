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

//! Message handlers for authorization messages

pub mod builder;
#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
mod v1_handlers;

use crate::network::auth::{
    AuthorizationAcceptingAction, AuthorizationAcceptingState, AuthorizationManagerStateMachine,
    AuthorizationMessageSender,
};
use crate::network::dispatch::{
    ConnectionId, DispatchError, Dispatcher, Handler, MessageContext, MessageSender,
};
use crate::protocol::authorization::AuthorizationError;
use crate::protos::authorization;
use crate::protos::network::NetworkMessageType;
use crate::protos::prelude::*;

pub use self::builder::AuthorizationDispatchBuilder;

/// The Handler for authorization network messages.
///
/// This Handler accepts authorization network messages, unwraps the envelope, and forwards the
/// message contents to an authorization dispatcher.
pub struct AuthorizationMessageHandler {
    auth_dispatcher: Dispatcher<authorization::AuthorizationMessageType, ConnectionId>,
}

impl AuthorizationMessageHandler {
    /// Constructs a new AuthorizationMessageHandler
    ///
    /// This constructs an AuthorizationMessageHandler with a sender that will dispatch messages
    /// to a authorization dispatcher.
    pub fn new(
        auth_dispatcher: Dispatcher<authorization::AuthorizationMessageType, ConnectionId>,
    ) -> Self {
        AuthorizationMessageHandler { auth_dispatcher }
    }
}

impl Handler for AuthorizationMessageHandler {
    type Source = ConnectionId;
    type MessageType = NetworkMessageType;
    type Message = authorization::AuthorizationMessage;

    fn match_type(&self) -> Self::MessageType {
        NetworkMessageType::AUTHORIZATION
    }

    fn handle(
        &self,
        mut msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _sender: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let msg_type = msg.get_message_type();
        let payload = msg.take_payload();
        self.auth_dispatcher
            .dispatch(context.source_id().clone(), &msg_type, payload)
    }
}

/// Handler for the Authorization Error Message Type
pub struct AuthorizationErrorHandler {
    auth_manager: AuthorizationManagerStateMachine,
}

impl AuthorizationErrorHandler {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        AuthorizationErrorHandler { auth_manager }
    }
}

impl Handler for AuthorizationErrorHandler {
    type Source = ConnectionId;
    type MessageType = authorization::AuthorizationMessageType;
    type Message = authorization::AuthorizationError;

    fn match_type(&self) -> Self::MessageType {
        authorization::AuthorizationMessageType::AUTHORIZATION_ERROR
    }

    fn handle(
        &self,
        msg: Self::Message,
        context: &MessageContext<Self::Source, Self::MessageType>,
        _: &dyn MessageSender<Self::Source>,
    ) -> Result<(), DispatchError> {
        let auth_error = AuthorizationError::from_proto(msg)?;
        match auth_error {
            AuthorizationError::AuthorizationRejected(err_msg) => {
                match self.auth_manager.next_accepting_state(
                    context.source_connection_id(),
                    AuthorizationAcceptingAction::Unauthorizing,
                ) {
                    Ok(AuthorizationAcceptingState::Unauthorized) => {
                        info!(
                            "Connection unauthorized by connection {}: {}",
                            context.source_connection_id(),
                            &err_msg
                        );
                    }
                    Err(err) => {
                        warn!(
                            "Unable to handle unauthorizing by connection {}: {}",
                            context.source_connection_id(),
                            err
                        );
                    }
                    Ok(next_state) => {
                        panic!("Should not have been able to transition to {}", next_state)
                    }
                }
            }
        }
        Ok(())
    }
}

impl MessageSender<ConnectionId> for AuthorizationMessageSender {
    fn send(&self, id: ConnectionId, message: Vec<u8>) -> Result<(), (ConnectionId, Vec<u8>)> {
        AuthorizationMessageSender::send(self, message).map_err(|msg| (id, msg))
    }
}
