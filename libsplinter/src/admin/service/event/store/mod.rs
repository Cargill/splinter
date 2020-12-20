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

//! Data store for maintaining ordered records of `AdminServiceEvent`s.
//!
//! The [`AdminServiceEventStore`] trait provides the public interface for storing
//! `AdminServiceEvent`s.
//!
//! [`AdminServiceEventStore`]: trait.AdminServiceEventStore.html

#[cfg(feature = "admin-service-event-store-diesel")]
mod diesel;
mod error;
pub mod memory;

pub use self::error::AdminServiceEventStoreError;
use crate::admin::service::messages::AdminServiceEvent;

/// Return type of the `AdminServiceEventStore` `list_events_*` methods.
pub type EventIter = Box<dyn ExactSizeIterator<Item = (i64, AdminServiceEvent)> + Send>;

/// Interface for performing CRUD operations on `AdminServiceEvent`s.
pub trait AdminServiceEventStore: Send + Sync {
    /// Add an event to the `AdminServiceEventStore`.  Returns the recorded event index and
    /// a copy of the event.
    ///
    /// # Arguments
    ///
    /// * `event` - the `AdminServiceEvent` to be added to the store
    fn add_event(
        &self,
        event: AdminServiceEvent,
    ) -> Result<(i64, AdminServiceEvent), AdminServiceEventStoreError>;

    /// List `AdminServiceEvent`s that have been added to the store since the provided index.
    ///
    /// # Arguments
    ///
    /// * `start` - index used to filter events
    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceEventStoreError>;

    /// List `AdminServiceEvent`s, with a corresponding `CircuitProposal` that has the specified
    /// `circuit_management_type`, that have been added to the store since the provided index.
    ///
    /// # Arguments
    ///
    /// * `management_type` - management type used to filter `CircuitProposal`s
    /// * `start` - index used to filter events
    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError>;
}
