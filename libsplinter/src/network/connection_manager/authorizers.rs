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

//! Implementations of the Authorizer trait.
//!
//! This module has a set of implementations of the Authorizer trait to handle several basic
//! authorization concerns. These implementations can handle the case where messages passed between
//! two connections are either not required, or authorization can be delegated to an another
//! authorizer, based on connection type.

use std::collections::HashMap;

use crate::transport::Connection;

use super::{AuthorizationResult, Authorizer, AuthorizerCallback, AuthorizerError};

/// Authorize Inproc Connections with predefined identities.
///
/// The InprocAuthorizer provides identities to connections based its remote endpoint.  The
/// identities are preconfigured when creating the this struct.
///
/// While this struct can accept any connection, it is called the InprocAuthorizer, as it is
/// intended to only be used for known, internal connections.
pub struct InprocAuthorizer {
    endpoint_to_identities: HashMap<String, String>,
}

impl InprocAuthorizer {
    /// Construct a new InprocAuthorizer with a given mapping of endpoints to identities.
    pub fn new<I>(identities: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self {
            endpoint_to_identities: identities.into_iter().collect(),
        }
    }
}

impl Authorizer for InprocAuthorizer {
    fn authorize_connection(
        &self,
        connection_id: String,
        connection: Box<dyn Connection>,
        on_complete: AuthorizerCallback,
    ) -> Result<(), AuthorizerError> {
        if let Some(identity) = self
            .endpoint_to_identities
            .get(&connection.remote_endpoint())
            .cloned()
        {
            (*on_complete)(AuthorizationResult::Authorized {
                connection_id,
                identity,
                connection,
            })
            .map_err(|err| AuthorizerError(err.to_string()))
        } else {
            (*on_complete)(AuthorizationResult::Unauthorized {
                connection_id,
                connection,
            })
            .map_err(|err| AuthorizerError(err.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::mpsc;

    use crate::transport::{Connection, DisconnectError, RecvError, SendError};

    // Test that the inproc authorizer will return a valid identity for the configured value.
    #[test]
    fn inproc_configured_authorization() {
        let authorizer = InprocAuthorizer::new(vec![(
            "inproc://test-conn".to_string(),
            "test-ident1".to_string(),
        )]);
        let (tx, rx) = mpsc::channel();

        authorizer
            .authorize_connection(
                "abcd-1234".into(),
                Box::new(MockConnection::new("inproc://test-conn")),
                Box::new(move |result| tx.send(result).map_err(Box::from)),
            )
            .unwrap();

        let result = rx.recv().unwrap();

        match result {
            AuthorizationResult::Authorized { identity, .. } => {
                assert_eq!("test-ident1", &identity)
            }
            AuthorizationResult::Unauthorized { .. } => panic!("should have been authorized"),
        }
    }

    // Test that the inproc authorizer will return a Unauthorized result for a unconfigured value.
    #[test]
    fn inproc_unconfigured_authorization() {
        let authorizer = InprocAuthorizer::new(vec![(
            "inproc://test-conn".to_string(),
            "test-ident1".to_string(),
        )]);
        let (tx, rx) = mpsc::channel();

        authorizer
            .authorize_connection(
                "abcd-1234".into(),
                Box::new(MockConnection::new("inproc://bad-inproc-conn")),
                Box::new(move |result| tx.send(result).map_err(Box::from)),
            )
            .unwrap();

        let result = rx.recv().unwrap();

        match result {
            AuthorizationResult::Authorized { .. } => panic!("should not have been authorized"),
            AuthorizationResult::Unauthorized { .. } => (),
        }
    }

    struct MockConnection {
        remote_endpoint: String,
    }

    impl MockConnection {
        fn new(remote_endpoint: &str) -> Self {
            Self {
                remote_endpoint: remote_endpoint.to_string(),
            }
        }
    }

    impl Connection for MockConnection {
        fn send(&mut self, _message: &[u8]) -> Result<(), SendError> {
            Ok(())
        }

        fn recv(&mut self) -> Result<Vec<u8>, RecvError> {
            unimplemented!()
        }

        fn remote_endpoint(&self) -> String {
            self.remote_endpoint.clone()
        }

        fn local_endpoint(&self) -> String {
            unimplemented!()
        }

        fn disconnect(&mut self) -> Result<(), DisconnectError> {
            Ok(())
        }

        fn evented(&self) -> &dyn mio::Evented {
            unimplemented!()
        }
    }
}
