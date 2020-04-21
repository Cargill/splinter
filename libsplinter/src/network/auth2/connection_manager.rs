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

use crate::network::connection_manager::{
    AuthorizationResult, Authorizer, AuthorizerCallback, AuthorizerError,
};
use crate::transport::Connection;

use super::{AuthorizationPoolError, ConnectionAuthorizationState, PoolAuthorizer};

impl Authorizer for PoolAuthorizer {
    fn authorize_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        callback: AuthorizerCallback,
    ) -> Result<(), AuthorizerError> {
        self.add_connection(
            connection_id,
            connection,
            Box::new(move |state| (*callback)(state.into())),
        )
        .map_err(AuthorizerError::from)
    }
}

impl From<ConnectionAuthorizationState> for AuthorizationResult {
    fn from(state: ConnectionAuthorizationState) -> Self {
        match state {
            ConnectionAuthorizationState::Authorized {
                connection_id,
                connection,
                identity,
            } => AuthorizationResult::Authorized {
                connection_id,
                connection,
                identity,
            },

            ConnectionAuthorizationState::Unauthorized {
                connection_id,
                connection,
            } => AuthorizationResult::Unauthorized {
                connection_id,
                connection,
            },
        }
    }
}

impl From<AuthorizationPoolError> for AuthorizerError {
    fn from(err: AuthorizationPoolError) -> Self {
        AuthorizerError(err.to_string())
    }
}
