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

//! Defines an API to manage in-flight OAuth2 requests.

mod memory;

use crate::error::InternalError;

use super::PendingAuthorization;

pub use memory::MemoryInflightOAuthRequestStore;

/// A Store for the in-flight information pertaining to an OAauth2 request.
///
/// An OAuth2 request consists of a request to the provider, and then a callback request back to
/// the library user's REST API.  There is information created for the first request that must be
/// verified by the second request. This store manages that information.
pub trait InflightOAuthRequestStore: Sync + Send {
    /// Insert a request into the store.
    fn insert_request(
        &self,
        request_id: String,
        authorization: PendingAuthorization,
    ) -> Result<(), InternalError>;

    /// Remove a request from the store and return it, if it exists.
    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<PendingAuthorization>, InternalError>;

    /// Clone the store for dynamic dispatch.
    fn clone_box(&self) -> Box<dyn InflightOAuthRequestStore>;
}

impl Clone for Box<dyn InflightOAuthRequestStore> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This test checks that a store implementation provides the insert and remove functionality
    /// correctly.  It does the following:
    /// 1. Insert a Pending authorization
    /// 2. Remove it and verify that the pending authorization is removed
    /// 3. Remove it a second time and verify that None is returned, indicating that the request
    ///    has been handled.
    pub fn test_request_store_insert_and_remove(
        inflight_request_store: &dyn InflightOAuthRequestStore,
    ) {
        inflight_request_store
            .insert_request(
                "test_request".to_string(),
                PendingAuthorization {
                    pkce_verifier: "this is a pkce_verifier".into(),
                    client_redirect_url: "http://example.com/someplace/nice".into(),
                },
            )
            .expect("Unable to insert pending request");

        let request = inflight_request_store
            .remove_request("test_request")
            .expect("Unable to remove and return the pending request");

        assert_eq!(
            Some(PendingAuthorization {
                pkce_verifier: "this is a pkce_verifier".into(),
                client_redirect_url: "http://example.com/someplace/nice".into(),
            }),
            request
        );

        // Attempt to remove again, and receive a None value
        let request = inflight_request_store
            .remove_request("test_request")
            .expect("Unable to remove and return the pending request");

        assert_eq!(None, request);
    }
}
