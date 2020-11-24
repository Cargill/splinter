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

#[cfg(feature = "diesel")]
mod diesel;
mod error;

use std::time::SystemTime;

pub use self::error::AdminServiceEventStoreError;
use crate::admin::service::messages::AdminServiceEvent;

/// Interface for performing CRUD operations on `AdminServiceEvent`s.
pub trait AdminServiceEventStore: Send + Sync {
    /// Add an event to the `AdminServiceEventStore`.  Returns the recorded event time and a copy
    /// of the event.
    ///
    /// # Arguments
    ///
    /// * `event` - the `AdminServiceEvent` to be added to the store
    fn add(
        &self,
        event: AdminServiceEvent,
    ) -> Result<(SystemTime, AdminServiceEvent), AdminServiceEventStoreError>;

    /// List `AdminServiceEvent`s that have been added to the store since the provided start index.
    ///
    /// # Arguments
    ///
    /// * `start` - index used to filter events
    fn iter_since(
        &self,
        start: i64,
    ) -> Result<Box<dyn ExactSizeIterator<Item = AdminServiceEvent>>, AdminServiceEventStoreError>;

    /// List `AdminServiceEvent`s, with a corresponding `CircuitProposal` that has the specified
    /// `management_type`, that have been added to the store since the provided start index.
    ///
    /// # Arguments
    ///
    /// * `management_type` - management type used to filter `CircuitProposal`s
    /// * `start` - index used to filter events
    fn iter_with_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<Box<dyn ExactSizeIterator<Item = AdminServiceEvent>>, AdminServiceEventStoreError>;
}
