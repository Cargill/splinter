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

mod handlers;

use cylinder::{Signer, Verifier};

use crate::error::InvalidStateError;
use crate::network::auth::AuthorizationManagerStateMachine;
use crate::network::auth::ConnectionAuthorizationType;

use self::handlers::{
    AuthChallengeNonceRequestHandler, AuthChallengeNonceResponseHandler,
    AuthChallengeSubmitRequestHandler, AuthChallengeSubmitResponseHandler,
};

use super::{AuthDispatchHandler, Authorization};

pub struct ChallengeAuthorization {
    signers: Vec<Box<dyn Signer>>,
    nonce: Vec<u8>,
    verifier: Option<Box<dyn Verifier>>,
    expected_authorization: Option<ConnectionAuthorizationType>,
    local_authorization: Option<ConnectionAuthorizationType>,
    auth_manager: AuthorizationManagerStateMachine,
}

impl ChallengeAuthorization {
    pub fn new(
        signers: Vec<Box<dyn Signer>>,
        nonce: Vec<u8>,
        verifier: Box<dyn Verifier>,
        expected_authorization: Option<ConnectionAuthorizationType>,
        local_authorization: Option<ConnectionAuthorizationType>,
        auth_manager: AuthorizationManagerStateMachine,
    ) -> Self {
        Self {
            signers,
            nonce,
            verifier: Some(verifier),
            expected_authorization,
            local_authorization,
            auth_manager,
        }
    }
}

impl Authorization for ChallengeAuthorization {
    /// get message handlers for authorization type
    fn get_handlers(&mut self) -> Result<Vec<AuthDispatchHandler>, InvalidStateError> {
        let mut handlers: Vec<AuthDispatchHandler> = vec![Box::new(
            AuthChallengeNonceRequestHandler::new(self.auth_manager.clone(), self.nonce.clone()),
        )];

        let signers_to_use = match &self.local_authorization {
            Some(ConnectionAuthorizationType::Challenge { public_key }) => {
                let signer = self
                    .signers
                    .iter()
                    .find(|signer| match signer.public_key() {
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
            _ => self.signers.clone(),
        };

        handlers.push(Box::new(AuthChallengeNonceResponseHandler::new(
            self.auth_manager.clone(),
            signers_to_use,
        )));

        let expected_public_key = match &self.expected_authorization {
            Some(ConnectionAuthorizationType::Challenge { public_key }) => Some(public_key.clone()),
            _ => None,
        };

        let verifier = self.verifier.take().ok_or_else(|| {
            InvalidStateError::with_message("No verifier to add to handler".to_string())
        })?;

        handlers.push(Box::new(AuthChallengeSubmitRequestHandler::new(
            self.auth_manager.clone(),
            verifier,
            self.nonce.clone(),
            expected_public_key,
        )));

        handlers.push(Box::new(AuthChallengeSubmitResponseHandler::new(
            self.auth_manager.clone(),
        )));

        Ok(handlers)
    }
}
