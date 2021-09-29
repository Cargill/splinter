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

pub mod handlers;

use crate::error::InvalidStateError;
use crate::network::auth::AuthorizationManagerStateMachine;

use self::handlers::{AuthTrustRequestHandler, AuthTrustResponseHandler};

use super::{AuthDispatchHandler, Authorization};

pub struct TrustAuthorization {
    auth_manager: AuthorizationManagerStateMachine,
}

impl TrustAuthorization {
    pub fn new(auth_manager: AuthorizationManagerStateMachine) -> Self {
        Self { auth_manager }
    }
}

impl Authorization for TrustAuthorization {
    /// get message handlers for authorization type
    fn get_handlers(&mut self) -> Result<Vec<AuthDispatchHandler>, InvalidStateError> {
        let mut handlers: Vec<AuthDispatchHandler> = vec![Box::new(AuthTrustRequestHandler::new(
            self.auth_manager.clone(),
        ))];

        handlers.push(Box::new(AuthTrustResponseHandler::new(
            self.auth_manager.clone(),
        )));

        Ok(handlers)
    }
}
