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

use crate::error::InvalidStateError;
use crate::network::auth::AuthorizationManagerStateMachine;

use self::handlers::{
    AuthorizedHandler, ConnectRequestHandler, ConnectResponseHandler, TrustRequestHandler,
};

use super::{AuthDispatchHandler, Authorization};

pub struct TrustV0Authorization {
    identity: String,
    auth_manager: AuthorizationManagerStateMachine,
}

impl TrustV0Authorization {
    pub fn new(identity: String, auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self {
            identity,
            auth_manager,
        }
    }
}

impl Authorization for TrustV0Authorization {
    /// get message handlers for authorization type
    fn get_handlers(&mut self) -> Result<Vec<AuthDispatchHandler>, InvalidStateError> {
        let mut handlers: Vec<AuthDispatchHandler> = vec![Box::new(ConnectRequestHandler::new(
            self.auth_manager.clone(),
        ))];

        handlers.push(Box::new(ConnectResponseHandler::new(
            self.identity.to_string(),
            self.auth_manager.clone(),
        )));

        handlers.push(Box::new(TrustRequestHandler::new(
            self.auth_manager.clone(),
        )));

        handlers.push(Box::new(AuthorizedHandler::new(self.auth_manager.clone())));

        Ok(handlers)
    }
}
