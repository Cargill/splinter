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

//! Data structures for building a `PeerManager` instance.
//!
//! The public interface includes the structs [`PeerManagerBuilder`]

use crate::network::connection_manager::Connector;

use super::error::PeerManagerError;
use super::PeerManager;

// Default value of how often the Pacemaker should send RetryPending message
const DEFAULT_PACEMAKER_INTERVAL: u64 = 10;
// The number of retry attempts for an active endpoint before the PeerManager will try other
// endpoints associated with a peer
const DEFAULT_MAXIMUM_RETRY_ATTEMPTS: u64 = 5;

#[derive(Default)]
pub struct PeerManagerBuilder {
    connector: Option<Connector>,
    max_retry_attempts: Option<u64>,
    retry_interval: Option<u64>,
    identity: Option<String>,
    strict_ref_counts: Option<bool>,
}

/// Constructs new `PeerManager` instances.
///
/// This builder is used to construct new `PeerManager` instances. The `PeerManager` requires
/// a `Connector` to request connections from the `ConnectionManageer` and the unique ID of the
/// node this `PeerManager` is for.  It also has several optional configuration values, such as
/// max_retry_attempts and retry_interval.
impl PeerManagerBuilder {
    /// Construct a new builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the connector instance to use with the resulting `PeerManager`.
    ///
    /// This is the `Connector` to the `ConnectionManager` that will handle the connections
    /// requested by the `PeerManager`.
    pub fn with_connector(mut self, connector: Connector) -> Self {
        self.connector = Some(connector);
        self
    }

    /// Set the max_retry_attempts instance to use with the resulting `PeerManager`.
    ///
    /// The number of retry attempts for an active endpoint before the
    /// `PeerManager` will try other endpoints associated with a peer
    pub fn with_max_retry_attempts(mut self, max_retry_attempts: u64) -> Self {
        self.max_retry_attempts = Some(max_retry_attempts);
        self
    }

    /// Set the retry_interval to use with the resulting `PeerManager`.
    ///
    /// How often (in seconds) the `Pacemaker` should notify the `PeerManager`
    /// to retry pending peers.
    pub fn with_retry_interval(mut self, retry_interval: u64) -> Self {
        self.retry_interval = Some(retry_interval);
        self
    }

    /// Set the identity to use with the resulting `PeerManager`.
    ///
    /// The unique ID of the node this `PeerManager` belongs to.
    pub fn with_identity(mut self, identity: String) -> Self {
        self.identity = Some(identity);
        self
    }

    /// Set strict_ref_counts in the the resulting `PeerManager`.
    ///
    /// Determines whether or not to panic when attempting to remove a
    /// reference to a peer that is not referenced.
    pub fn with_strict_ref_counts(mut self, strict_ref_counts: bool) -> Self {
        self.strict_ref_counts = Some(strict_ref_counts);
        self
    }

    /// Starts the `PeerManager`
    ///
    /// Starts up a thread that will handle incoming requests to add, remove and get peers. Also
    /// handles notifications from the `ConnectionManager`.
    ///
    /// Returns a `PeerManagerConnector` that can be used to send requests to the `PeerManager`.
    pub fn start(&mut self) -> Result<PeerManager, PeerManagerError> {
        let retry_interval = self.retry_interval.unwrap_or(DEFAULT_PACEMAKER_INTERVAL);
        let max_retry_attempts = self
            .max_retry_attempts
            .unwrap_or(DEFAULT_MAXIMUM_RETRY_ATTEMPTS);
        let strict_ref_counts = self.strict_ref_counts.unwrap_or(false);
        let identity = self.identity.take().ok_or_else(|| {
            PeerManagerError::StartUpError("Missing required value `identity`".to_string())
        })?;
        let connector = self.connector.take().ok_or_else(|| {
            PeerManagerError::StartUpError("Missing required value `connector`".to_string())
        })?;

        PeerManager::build(
            retry_interval,
            max_retry_attempts,
            strict_ref_counts,
            identity,
            connector,
        )
    }
}
