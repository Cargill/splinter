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

//! Builder for creating the dispatcher for authorization messages

#[cfg(feature = "challenge-authorization")]
use cylinder::{Signer, Verifier};

use crate::error::InvalidStateError;
use crate::network::auth::AuthorizationManagerStateMachine;
#[cfg(feature = "challenge-authorization")]
use crate::network::auth::ConnectionAuthorizationType;
use crate::network::dispatch::{ConnectionId, Dispatcher, MessageSender};
use crate::protos::network::NetworkMessageType;

use super::v0_handlers::{
    AuthorizedHandler, ConnectRequestHandler, ConnectResponseHandler, TrustRequestHandler,
};
#[cfg(feature = "challenge-authorization")]
use super::v1_handlers::challenge::{
    AuthChallengeNonceRequestHandler, AuthChallengeNonceResponseHandler,
    AuthChallengeSubmitRequestHandler, AuthChallengeSubmitResponseHandler,
};
#[cfg(feature = "trust-authorization")]
use super::v1_handlers::trust::{AuthTrustRequestHandler, AuthTrustResponseHandler};
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
    #[cfg(feature = "challenge-authorization")]
    signers: Option<Vec<Box<dyn Signer>>>,
    #[cfg(feature = "challenge-authorization")]
    nonce: Option<Vec<u8>>,
    #[cfg(feature = "challenge-authorization")]
    expected_authorization: Option<ConnectionAuthorizationType>,
    #[cfg(feature = "challenge-authorization")]
    local_authorization: Option<ConnectionAuthorizationType>,
    #[cfg(feature = "challenge-authorization")]
    verifier: Option<Box<dyn Verifier>>,
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

    /// Sets the signers
    ///
    /// # Arguments
    ///
    ///  * `signers` - The list of supported signing keys to be used in challenge authorization
    #[cfg(feature = "challenge-authorization")]
    pub fn with_signers(mut self, signers: &[Box<dyn Signer>]) -> Self {
        self.signers = Some(signers.to_vec());
        self
    }

    /// Sets the nonce
    ///
    /// # Arguments
    ///
    ///  * `nonce` - The random bytes that must be signed in challenge authorization
    #[cfg(feature = "challenge-authorization")]
    pub fn with_nonce(mut self, nonce: &[u8]) -> Self {
        self.nonce = Some(nonce.to_vec());
        self
    }

    /// Sets the expected authorization
    ///
    /// # Arguments
    ///
    ///  * `expected_authorization` - The expected authorization type of the connecting connection
    #[cfg(feature = "challenge-authorization")]
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
    #[cfg(feature = "challenge-authorization")]
    pub fn with_local_authorization(
        mut self,
        local_authorization: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.local_authorization = local_authorization;
        self
    }

    /// Sets the verifier
    ///
    /// # Arguments
    ///
    ///  * `verifier` - The authorization type the local node must use to connect
    #[cfg(feature = "challenge-authorization")]
    pub fn with_verifier(mut self, verifier: Box<dyn Verifier>) -> Self {
        self.verifier = Some(verifier);
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
        let identity = self.identity.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `identity` field".to_string())
        })?;

        #[cfg(feature = "challenge-authorization")]
        let signers = self.signers.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `signers` field".to_string())
        })?;

        #[cfg(feature = "challenge-authorization")]
        if signers.is_empty() {
            return Err(InvalidStateError::with_message(
                "At least one signer must be configured".to_string(),
            ));
        };

        #[cfg(feature = "challenge-authorization")]
        let nonce = self.nonce.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `nonce` field".to_string())
        })?;

        #[cfg(feature = "challenge-authorization")]
        let verifier = self.verifier.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `verifier` field".to_string())
        })?;

        let mut auth_dispatcher = Dispatcher::new(Box::new(auth_msg_sender.clone()));

        // v0 message handlers
        auth_dispatcher.set_handler(Box::new(ConnectRequestHandler::new(auth_manager.clone())));

        // allow redundant_clone, must be cloned here if trust-authorization is enabled
        #[allow(clippy::redundant_clone)]
        auth_dispatcher.set_handler(Box::new(ConnectResponseHandler::new(identity.to_string())));

        auth_dispatcher.set_handler(Box::new(TrustRequestHandler::new(auth_manager.clone())));

        auth_dispatcher.set_handler(Box::new(AuthorizedHandler::new(auth_manager.clone())));

        auth_dispatcher.set_handler(Box::new(AuthorizedHandler::new(auth_manager.clone())));

        // v1 message handlers
        #[cfg(any(feature = "trust-authorization", feature = "challenge-authorization"))]
        {
            let mut auth_protocol_request_builder = AuthProtocolRequestHandlerBuilder::default()
                .with_auth_manager(auth_manager.clone());

            #[cfg(feature = "challenge-authorization")]
            {
                auth_protocol_request_builder = auth_protocol_request_builder
                    .with_expected_authorization(self.expected_authorization.clone())
                    .with_local_authorization(self.local_authorization.clone())
            }

            auth_dispatcher.set_handler(Box::new(auth_protocol_request_builder.build()?));

            let mut auth_protocol_response_builder = AuthProtocolResponseHandlerBuilder::default()
                .with_auth_manager(auth_manager.clone());

            #[cfg(feature = "trust-authorization")]
            {
                auth_protocol_response_builder =
                    auth_protocol_response_builder.with_identity(&identity);
            }
            #[cfg(feature = "challenge-authorization")]
            {
                auth_protocol_response_builder = auth_protocol_response_builder
                    .with_required_local_auth(self.local_authorization.clone())
            }

            auth_dispatcher.set_handler(Box::new(auth_protocol_response_builder.build()?));

            auth_dispatcher.set_handler(Box::new(AuthCompleteHandler::new(auth_manager.clone())));
        }

        #[cfg(feature = "trust-authorization")]
        {
            auth_dispatcher
                .set_handler(Box::new(AuthTrustRequestHandler::new(auth_manager.clone())));

            auth_dispatcher.set_handler(Box::new(AuthTrustResponseHandler::new(
                auth_manager.clone(),
            )));
        }

        #[cfg(feature = "challenge-authorization")]
        {
            auth_dispatcher.set_handler(Box::new(AuthChallengeNonceRequestHandler::new(
                auth_manager.clone(),
                nonce.clone(),
            )));

            let signers_to_use = match &self.local_authorization {
                Some(ConnectionAuthorizationType::Challenge { public_key }) => {
                    let signer = signers.iter().find(|signer| match signer.public_key() {
                        Ok(signer_public_key) => {
                            signer_public_key.as_slice() == public_key.as_slice()
                        }
                        Err(_) => false,
                    });

                    match signer {
                        Some(signer) => vec![signer.clone()],
                        None => {
                            return Err(InvalidStateError::with_message(
                                "Required local authorization is not supported".to_string(),
                            ));
                        }
                    }
                }

                // if there is no local_authorization which key is used here does not matter
                _ => signers.clone(),
            };

            auth_dispatcher.set_handler(Box::new(AuthChallengeNonceResponseHandler::new(
                auth_manager.clone(),
                signers_to_use,
            )));

            let expected_public_key = match self.expected_authorization {
                Some(ConnectionAuthorizationType::Challenge { public_key }) => Some(public_key),
                _ => None,
            };

            auth_dispatcher.set_handler(Box::new(AuthChallengeSubmitRequestHandler::new(
                auth_manager.clone(),
                verifier,
                nonce,
                expected_public_key,
            )));

            auth_dispatcher.set_handler(Box::new(AuthChallengeSubmitResponseHandler::new(
                auth_manager.clone(),
            )));
        }

        auth_dispatcher.set_handler(Box::new(AuthorizationErrorHandler::new(auth_manager)));

        let mut network_msg_dispatcher = Dispatcher::new(Box::new(auth_msg_sender));

        network_msg_dispatcher
            .set_handler(Box::new(AuthorizationMessageHandler::new(auth_dispatcher)));

        Ok(network_msg_dispatcher)
    }
}
