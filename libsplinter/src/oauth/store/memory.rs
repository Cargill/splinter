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

//! Memory-backed InflightOAuthRequestStore implementation.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::collections::TtlMap;
use crate::error::InternalError;
use crate::oauth::{InflightOAuthRequestStore, PendingAuthorization};

/// The amount of time before a pending authorization expires and a new request must be made
const PENDING_AUTHORIZATION_EXPIRATION_SECS: u64 = 3600; // 1 hour

/// A memory-backed implementation of InflightOAuthRequestStore.
///
/// Values in this store expire after an hour.
#[derive(Clone)]
pub struct MemoryInflightOAuthRequestStore {
    pending_authorizations: Arc<Mutex<TtlMap<String, PendingAuthorization>>>,
}

impl MemoryInflightOAuthRequestStore {
    /// Constructs a new instance.
    pub fn new() -> Self {
        Self {
            pending_authorizations: Arc::new(Mutex::new(TtlMap::new(Duration::from_secs(
                PENDING_AUTHORIZATION_EXPIRATION_SECS,
            )))),
        }
    }
}

impl Default for MemoryInflightOAuthRequestStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InflightOAuthRequestStore for MemoryInflightOAuthRequestStore {
    fn insert_request(
        &self,
        request_id: String,
        pending_authorization: PendingAuthorization,
    ) -> Result<(), InternalError> {
        self.pending_authorizations
            .lock()
            .map_err(|_| {
                InternalError::with_message("pending authorizations lock was poisoned".into())
            })?
            .insert(request_id, pending_authorization);

        Ok(())
    }

    fn remove_request(
        &self,
        request_id: &str,
    ) -> Result<Option<PendingAuthorization>, InternalError> {
        Ok(self
            .pending_authorizations
            .lock()
            .map_err(|_| {
                InternalError::with_message("pending authorizations lock was poisoned".into())
            })?
            .remove(request_id))
    }

    fn clone_box(&self) -> Box<dyn InflightOAuthRequestStore> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::oauth::tests::test_request_store_insert_and_remove;

    #[test]
    fn memory_insert_request_and_remove() {
        let inflight_request_store = MemoryInflightOAuthRequestStore::new();
        test_request_store_insert_and_remove(&inflight_request_store);
    }
}
