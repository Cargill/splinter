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

//! Defines a memory-backed implementation of the `AdminServiceEventStore`.
//!
//! The public interface includes the struct [`MemoryAdminServiceEventStore`].
//!
//! [`MemoryAdminServiceEventStore`]: struct.MemoryAdminServiceEventStore.html

use std::cmp;
use std::collections::BTreeSet;
use std::convert::TryInto;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use crate::admin::service::event::store::{
    AdminServiceEventStore, AdminServiceEventStoreError, EventIter,
};
use crate::admin::service::messages::AdminServiceEvent;
use crate::error::InternalError;

/// A simple entry for `AdminServiceEvent` values, to be ordered by the `id`
#[derive(Debug, Eq, PartialEq, Clone)]
struct EventEntry {
    id: i64,
    event: AdminServiceEvent,
}

impl cmp::Ord for EventEntry {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl cmp::PartialOrd for EventEntry {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::borrow::Borrow<i64> for EventEntry {
    fn borrow(&self) -> &i64 {
        &self.id
    }
}

/// A memory-backed implementation of the `AdminServiceEventStore`
///
/// This set is bounded, where in it will drop the first item in the set, based on the natural
/// order of the items stored.
#[derive(Default)]
pub struct MemoryAdminServiceEventStore {
    /// Inner data structure which holds `AdminServiceEvent` values
    inner: Arc<Mutex<BTreeSet<EventEntry>>>,
    /// In-memory bound of the event store
    bound: usize,
    /// ID of the last event added to the store
    last_event_id: AtomicI64,
}

impl MemoryAdminServiceEventStore {
    /// Creates a new `MemoryAdminServiceEventStore`.
    pub fn new_boxed() -> Box<dyn AdminServiceEventStore> {
        Box::new(MemoryAdminServiceEventStore {
            inner: Arc::new(Mutex::new(BTreeSet::new())),
            bound: std::usize::MAX,
            last_event_id: AtomicI64::new(0),
        })
    }

    pub fn new_boxed_with_bound(bound: NonZeroUsize) -> Box<dyn AdminServiceEventStore> {
        Box::new(Self {
            inner: Arc::new(Mutex::new(BTreeSet::new())),
            bound: bound.get(),
            last_event_id: AtomicI64::new(0),
        })
    }
}

impl AdminServiceEventStore for MemoryAdminServiceEventStore {
    /// Add an event to the `MemoryAdminServiceEventStore`.  Returns the recorded event ID and a
    /// copy of the event.
    fn add_event(
        &self,
        event: AdminServiceEvent,
    ) -> Result<(i64, AdminServiceEvent), AdminServiceEventStoreError> {
        let mut inner = self.inner.lock().map_err(|_| {
            AdminServiceEventStoreError::InternalError(InternalError::with_message(String::from(
                "Cannot access admin events: mutex lock poisoned",
            )))
        })?;
        // Fetch the `last_event_id` and convert to usize to ensure the in-memory bounds on the
        // store have not been reached.
        // This uses `fetch_add`, which increments the `last_event_id`
        let previous_event_id: usize = self
            .last_event_id
            .load(Ordering::Relaxed)
            .try_into()
            .map_err(|_| {
                AdminServiceEventStoreError::InternalError(InternalError::with_message(
                    String::from("Unable to convert previous event ID into usize"),
                ))
            })?;
        if previous_event_id == self.bound {
            // Remove the first (oldest) event in the store to make room for the new event
            let rm_lowest = inner.iter().cloned().next().ok_or({
                AdminServiceEventStoreError::InternalError(InternalError::with_message(
                    String::from("Cannot access admin events to remove last"),
                ))
            })?;
            inner.remove(&rm_lowest.id);
        }
        // Uses the `fetch_add` method to increment the `last_event_id`
        self.last_event_id.fetch_add(1, Ordering::Relaxed);
        inner.insert(EventEntry {
            id: self.last_event_id.load(Ordering::Relaxed),
            event: event.clone(),
        });

        Ok((self.last_event_id.load(Ordering::Relaxed), event))
    }

    /// List `AdminServiceEvent`s that have been added to the store since the provided index.
    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceEventStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            AdminServiceEventStoreError::InternalError(InternalError::with_message(String::from(
                "Cannot access admin events: mutex lock poisoned",
            )))
        })?;
        // Increment the `start` index to exclude that ID
        let exclusive_start = start + 1;
        // Construct a list of tuples of the event ID and the corresponding event to be returned.
        let inner_iter: Vec<(i64, AdminServiceEvent)> = inner
            .range(exclusive_start..)
            .map(|entry| (entry.id, entry.event.clone()))
            .collect();
        Ok(Box::new(inner_iter.into_iter()))
    }

    /// List `AdminServiceEvent`s, with a corresponding `CircuitProposal` that has the specified
    /// `circuit_management_type`, that have been added to the store since the provided index.
    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        let inner = self.inner.lock().map_err(|_| {
            AdminServiceEventStoreError::InternalError(InternalError::with_message(String::from(
                "Cannot access admin events: mutex lock poisoned",
            )))
        })?;
        // Increment the `start` index to exclude that ID
        let exclusive_start = start + 1;
        // Construct a list of tuples of the event ID and the corresponding event to be returned.
        let inner_iter: Vec<(i64, AdminServiceEvent)> = inner
            .range(exclusive_start..)
            .filter_map(|entry| {
                if entry.event.proposal().circuit.circuit_management_type == management_type {
                    return Some((entry.id, entry.event.clone()));
                }
                None
            })
            .collect();
        Ok(Box::new(inner_iter.into_iter()))
    }
}

#[cfg(test)]
mod tests {
    use crate::admin::messages::{self, AdminServiceEvent, CircuitProposal, ProposalType};

    use super::*;

    /// Validate the `AdminServiceEventStore` successfully creates a list of the events added.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Creates and adds multiple `AdminServiceEvent`s to the store
    /// 3. Creates a list of events from the store, using the `list_events` method.
    /// 4. Validate this list, created after all events have been added to the store, contains
    ///    all of the expected events
    #[test]
    fn test_memory_admin_event_store_list() {
        let event_store = MemoryAdminServiceEventStore::new_boxed();

        event_store
            .add_event(make_event("circuit_one", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_one", "gameroom"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("circuit_two", "default"))
            .expect("Unable to add event");

        assert_eq!(
            vec![
                (1, make_event("circuit_one", "default")),
                (2, make_event("gameroom_one", "gameroom")),
                (3, make_event("circuit_two", "default")),
            ],
            event_store
                .list_events_since(0)
                .expect("Unable to create an admin events list")
                .collect::<Vec<(i64, AdminServiceEvent)>>()
        )
    }

    /// Verifies an empty `AdminServiceEventStore` returns no events.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Validate a list created from the empty event store is empty
    #[test]
    fn test_memory_admin_event_store_list_empty() {
        let event_store = MemoryAdminServiceEventStore::new_boxed();
        assert!(&event_store
            .list_events_since(0)
            .expect("Unable to create an admin events list")
            .collect::<Vec<(i64, AdminServiceEvent)>>()
            .is_empty());
    }

    /// Validate the `AdminServiceEventStore` successfully creates a list of the events added
    /// before returning the events, excluding any events that have been added to the store
    /// afterwards.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Creates and adds two `AdminServiceEvent`s to the store
    /// 3. Creates a list from the event store
    /// 4. Creates and adds another `AdminServiceEvent` to the store.
    /// 5. Validate the list of events only contains the two events added before creating the list
    #[test]
    fn test_memory_admin_event_store_ignores_new() {
        let event_store = MemoryAdminServiceEventStore::new_boxed();

        event_store
            .add_event(make_event("circuit_one", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_one", "gameroom"))
            .expect("Unable to add event");

        let event_list = event_store
            .list_events_since(0)
            .expect("Unable to create an admin events list")
            .collect::<Vec<(i64, AdminServiceEvent)>>();

        event_store
            .add_event(make_event("circuit_two", "default"))
            .expect("Unable to add event");

        assert_eq!(
            vec![
                (1, make_event("circuit_one", "default")),
                (2, make_event("gameroom_one", "gameroom")),
            ],
            event_list,
        );
    }

    /// Validate the `AdminServiceEventStore` successfully creates a list of the events added
    /// since the index provided.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Creates and adds three `AdminServiceEvent`s to the store
    /// 3. Validate an event list created by passing in `1` to the `iter_since` method will return
    ///    the events added to the store after the first item.
    #[test]
    fn test_memory_admin_event_store_list_since() {
        let event_store = MemoryAdminServiceEventStore::new_boxed();

        event_store
            .add_event(make_event("circuit_one", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_one", "gameroom"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("circuit_two", "default"))
            .expect("Unable to add event");

        assert_eq!(
            vec![
                (2, make_event("gameroom_one", "gameroom")),
                (3, make_event("circuit_two", "default")),
            ],
            event_store
                .list_events_since(1)
                .expect("Unable to create an admin events list")
                .collect::<Vec<(i64, AdminServiceEvent)>>()
        )
    }

    /// Validate the `AdminServiceEventStore` successfully creates a list of the events added since
    /// the specified index.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Creates and adds multiple `AdminServiceEvent`s to the store
    /// 3. Creates a list of events from the store, using the
    ///    `list_events_by_management_type_since` method, specifying the `gameroom` management type.
    /// 4. Validate this list, created after all events have been added to the store, contains
    ///    only the events with the `gameroom` management type that have been added after the
    ///    second event.
    #[test]
    fn test_memory_admin_event_store_list_with_management_type() {
        let event_store = MemoryAdminServiceEventStore::new_boxed();

        event_store
            .add_event(make_event("circuit_one", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_one", "gameroom"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("circuit_two", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_two", "gameroom"))
            .expect("Unable to add event");

        // Validate only the second "gameroom" circuit appears in the results, as the
        // `list_events_by_management_type_since` is filtering on the "gameroom" mangaement type
        // and should only select events since the second event.
        assert_eq!(
            vec![(4, make_event("gameroom_two", "gameroom"))],
            event_store
                .list_events_by_management_type_since("gameroom".to_string(), 2)
                .expect("Unable to create an admin events list")
                .collect::<Vec<(i64, AdminServiceEvent)>>()
        )
    }

    /// Validate the `AdminServiceEventStore` successfully creates a list of the events added since
    /// the specified index, while also successfully returning an error when using an invalid
    /// event ID.
    ///
    /// 1. Creates a `MemoryAdminServiceEventStore`
    /// 2. Creates and adds multiple `AdminServiceEvent`s to the store
    /// 3. Creates a list of events from the store, using the
    ///    `list_events_by_management_type_since` method, specifying an invalid index that should
    ///    return an error
    /// 4. Creates another list, created after all events have been added to the store, and
    ///    validates this list contains only the event with the `gameroom` management type that
    ///    has been added after the second event.
    #[test]
    fn test_memory_admin_event_store_list_bounded() {
        let event_store = MemoryAdminServiceEventStore::new_boxed_with_bound(
            std::num::NonZeroUsize::new(3).unwrap(),
        );

        event_store
            .add_event(make_event("circuit_one", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_one", "gameroom"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("circuit_two", "default"))
            .expect("Unable to add event");
        event_store
            .add_event(make_event("gameroom_two", "gameroom"))
            .expect("Unable to add event");

        // Validate only the second "gameroom" circuit appears in the results, as the
        // `list_events_by_management_type_since` is filtering on the "gameroom" mangaement type
        // and should only select events since the second event.
        assert_eq!(
            vec![
                (2, make_event("gameroom_one", "gameroom")),
                (3, make_event("circuit_two", "default")),
                (4, make_event("gameroom_two", "gameroom")),
            ],
            event_store
                .list_events_since(1)
                .expect("Unable to create an admin events list")
                .collect::<Vec<(i64, AdminServiceEvent)>>()
        );

        assert_eq!(
            vec![(4, make_event("gameroom_two", "gameroom")),],
            event_store
                .list_events_by_management_type_since("gameroom".to_string(), 2)
                .expect("Unable to create an admin events list")
                .collect::<Vec<(i64, AdminServiceEvent)>>()
        );
    }

    fn make_event(circuit_id: &str, event_type: &str) -> AdminServiceEvent {
        AdminServiceEvent::ProposalSubmitted(CircuitProposal {
            proposal_type: ProposalType::Create,
            circuit_id: circuit_id.into(),
            circuit_hash: "not real hash for tests".into(),
            circuit: messages::CreateCircuit {
                circuit_id: circuit_id.into(),
                roster: vec![],
                members: vec![],
                authorization_type: messages::AuthorizationType::Trust,
                persistence: messages::PersistenceType::Any,
                durability: messages::DurabilityType::NoDurability,
                routes: messages::RouteType::Any,
                circuit_management_type: event_type.into(),
                application_metadata: vec![],
                comments: "mock circuit".into(),
                display_name: None,
            },
            votes: vec![],
            requester: vec![],
            requester_node_id: "another-node".into(),
        })
    }
}
