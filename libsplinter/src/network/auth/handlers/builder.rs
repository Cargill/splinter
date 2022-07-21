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

//! Builder for creating the dispatcher for authorization messages

use crate::error::InvalidStateError;
use crate::network::auth::authorization::Authorization;
use crate::network::auth::AuthorizationManagerStateMachine;
use crate::network::auth::ConnectionAuthorizationType;
use crate::network::dispatch::{ConnectionId, Dispatcher, MessageSender};
use crate::protos::network::NetworkMessageType;

#[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
use super::v1_handlers::{
    builders::{AuthProtocolRequestHandlerBuilder, AuthProtocolResponseHandlerBuilder},
    AuthCompleteHandler,
};
use super::{AuthorizationErrorHandler, AuthorizationMessageHandler};

/// Builder for creating a Dispatcher for Authorization messages
///
/// Creates and configures a Dispatcher to handle messages from an AuthorizationMessage envelope.
/// The dispatcher is provided the given network sender for response messages, and the network
/// itself to handle updating identities (or removing connections with authorization failures).
///
/// The identity provided is sent to connections for Trust authorizations.
#[derive(Default)]
pub struct AuthorizationDispatchBuilder {
    identity: Option<String>,
    expected_authorization: Option<ConnectionAuthorizationType>,
    local_authorization: Option<ConnectionAuthorizationType>,
    authorizations: Vec<Box<dyn Authorization>>,
}

impl AuthorizationDispatchBuilder {
    pub fn new() -> Self {
        AuthorizationDispatchBuilder::default()
    }

    /// Sets the identity
    ///
    /// # Arguments
    ///
    ///  * `identity` - The local node ID
    pub fn with_identity(mut self, identity: &str) -> Self {
        self.identity = Some(identity.to_string());
        self
    }

    /// Sets the expected authorization
    ///
    /// # Arguments
    ///
    ///  * `expected_authorization` - The expected authorization type of the connecting connection
    pub fn with_expected_authorization(
        mut self,
        expected_authorization: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.expected_authorization = expected_authorization;
        self
    }

    /// Sets the local authorization
    ///
    /// # Arguments
    ///
    ///  * `local_authorization` - The authorization type the local node must use to connect
    pub fn with_local_authorization(
        mut self,
        local_authorization: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.local_authorization = local_authorization;
        self
    }

    /// Adds an authorization implementation
    ///
    /// # Arguments
    ///
    ///  * `authorization` - An implementation of an authorization type
    pub fn add_authorization(mut self, authorization: Box<dyn Authorization>) -> Self {
        self.authorizations.push(authorization);
        self
    }

    /// Builder dispatcher
    ///
    /// If identity, nonce or verifier is not set, an InvalidStateError is returned
    pub fn build(
        self,
        auth_msg_sender: impl MessageSender<ConnectionId> + Clone + 'static,
        auth_manager: AuthorizationManagerStateMachine,
    ) -> Result<Dispatcher<NetworkMessageType, ConnectionId>, InvalidStateError> {
        #[cfg(feature = "trust-authorization")]
        let identity = self.identity.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `identity` field".to_string())
        })?;

        let mut auth_dispatcher = Dispatcher::new(Box::new(auth_msg_sender.clone()));

        // v1 message handlers
        #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
        {
            // allow unused mut, required if challenge-authorization is enabled
            #![allow(unused_mut)]
            let mut auth_protocol_request_builder = AuthProtocolRequestHandlerBuilder::default()
                .with_auth_manager(auth_manager.clone());

            auth_protocol_request_builder = auth_protocol_request_builder
                .with_expected_authorization(self.expected_authorization.clone())
                .with_local_authorization(self.local_authorization.clone());

            auth_dispatcher.set_handler(Box::new(auth_protocol_request_builder.build()?));

            let mut auth_protocol_response_builder = AuthProtocolResponseHandlerBuilder::default()
                .with_auth_manager(auth_manager.clone());

            #[cfg(feature = "trust-authorization")]
            {
                auth_protocol_response_builder =
                    auth_protocol_response_builder.with_identity(&identity);
            }

            // allow redundant clone, required if challenge-authorization is enabled
            #[allow(clippy::redundant_clone)]
            {
                auth_protocol_response_builder = auth_protocol_response_builder
                    .with_required_local_auth(self.local_authorization.clone());
            }

            auth_dispatcher.set_handler(Box::new(auth_protocol_response_builder.build()?));

            auth_dispatcher.set_handler(Box::new(AuthCompleteHandler::new(auth_manager.clone())));
        }

        for mut authorization in self.authorizations.into_iter() {
            let handlers = authorization.get_handlers()?;

            for handler in handlers {
                auth_dispatcher.set_handler(handler);
            }
        }

        auth_dispatcher.set_handler(Box::new(AuthorizationErrorHandler::new(auth_manager)));

        let mut network_msg_dispatcher = Dispatcher::new(Box::new(auth_msg_sender));

        network_msg_dispatcher
            .set_handler(Box::new(AuthorizationMessageHandler::new(auth_dispatcher)));

        Ok(network_msg_dispatcher)
    }
}
