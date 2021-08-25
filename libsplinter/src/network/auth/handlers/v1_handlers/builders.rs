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

//! Builders for creating the Authorization protocol request and response handlers

use crate::error::InvalidStateError;
use crate::network::auth::{AuthorizationManagerStateMachine, ConnectionAuthorizationType};

use super::{AuthProtocolRequestHandler, AuthProtocolResponseHandler};

/// Builder for AuthProtocolRequestHandler
#[derive(Default)]
pub struct AuthProtocolRequestHandlerBuilder {
    auth_manager: Option<AuthorizationManagerStateMachine>,
    #[cfg(feature = "challenge-authorization")]
    expected_authorization: Option<ConnectionAuthorizationType>,
    #[cfg(feature = "challenge-authorization")]
    local_authorization: Option<ConnectionAuthorizationType>,
}

impl AuthProtocolRequestHandlerBuilder {
    pub fn with_auth_manager(mut self, auth_manager: AuthorizationManagerStateMachine) -> Self {
        self.auth_manager = Some(auth_manager);
        self
    }

    #[cfg(feature = "challenge-authorization")]
    pub fn with_expected_authorization(
        mut self,
        expected_authorization: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.expected_authorization = expected_authorization;
        self
    }

    #[cfg(feature = "challenge-authorization")]
    pub fn with_local_authorization(
        mut self,
        local_authorization: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.local_authorization = local_authorization;
        self
    }

    pub fn build(self) -> Result<AuthProtocolRequestHandler, InvalidStateError> {
        let auth_manager = self.auth_manager.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `auth_manager` field".to_string())
        })?;

        Ok(AuthProtocolRequestHandler {
            auth_manager,
            #[cfg(feature = "challenge-authorization")]
            expected_authorization: self.expected_authorization,
            #[cfg(feature = "challenge-authorization")]
            local_authorization: self.local_authorization,
        })
    }
}

/// Builder for AuthProtocolResponseHandler
#[derive(Default)]
pub struct AuthProtocolResponseHandlerBuilder {
    auth_manager: Option<AuthorizationManagerStateMachine>,
    #[cfg(feature = "trust-authorization")]
    identity: Option<String>,
    required_local_auth: Option<ConnectionAuthorizationType>,
}

impl AuthProtocolResponseHandlerBuilder {
    pub fn with_auth_manager(mut self, auth_manager: AuthorizationManagerStateMachine) -> Self {
        self.auth_manager = Some(auth_manager);
        self
    }

    #[cfg(feature = "trust-authorization")]
    pub fn with_identity(mut self, identity: &str) -> Self {
        self.identity = Some(identity.to_string());
        self
    }

    #[cfg(feature = "challenge-authorization")]
    pub fn with_required_local_auth(
        mut self,
        required_local_auth: Option<ConnectionAuthorizationType>,
    ) -> Self {
        self.required_local_auth = required_local_auth;
        self
    }

    pub fn build(self) -> Result<AuthProtocolResponseHandler, InvalidStateError> {
        let auth_manager = self.auth_manager.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `auth_manager` field".to_string())
        })?;

        #[cfg(feature = "trust-authorization")]
        let identity = self.identity.ok_or_else(|| {
            InvalidStateError::with_message("Missing required `identity` field".to_string())
        })?;

        Ok(AuthProtocolResponseHandler {
            auth_manager,
            #[cfg(feature = "trust-authorization")]
            identity,
            required_local_auth: self.required_local_auth,
        })
    }
}
