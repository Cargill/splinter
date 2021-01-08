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

//! Database backend support for the `AdminServiceEventStore`, powered by
//! [`Diesel`](https://crates.io/crates/diesel).
//!
//! This module contains the [`DieselAdminServiceStore`].
//!
//! [`DieselAdminServiceEventStore`]: struct.DieselAdminServiceEventStore.html
//! [`AdminServiceEventStore`]: ../trait.AdminServiceEventStore.html

mod models;
mod operations;
mod schema;

use diesel::r2d2::{ConnectionManager, Pool};

use crate::admin::service::event::{
    store::{AdminServiceEventStore, AdminServiceEventStoreError, EventIter},
    AdminServiceEvent,
};
use crate::admin::service::messages;

use operations::add_event::AdminServiceEventStoreAddEventOperation as _;
use operations::list_events_by_management_type_since::AdminServiceEventStoreListEventsByManagementTypeSinceOperation as _;
use operations::list_events_since::AdminServiceEventStoreListEventsSinceOperation as _;
use operations::AdminServiceEventStoreOperations;

/// A database-backed AdminServiceEventStore, powered by [`Diesel`](https://crates.io/crates/diesel).
pub struct DieselAdminServiceEventStore<C: diesel::Connection + 'static> {
    connection_pool: Pool<ConnectionManager<C>>,
}

impl<C: diesel::Connection> DieselAdminServiceEventStore<C> {
    /// Creates a new `DieselAdminServiceEventStore`.
    ///
    /// # Arguments
    ///
    ///  * `connection_pool`: connection pool for the database
    pub fn _new(connection_pool: Pool<ConnectionManager<C>>) -> Self {
        DieselAdminServiceEventStore { connection_pool }
    }
}

#[cfg(feature = "sqlite")]
impl Clone for DieselAdminServiceEventStore<diesel::sqlite::SqliteConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "sqlite")]
impl AdminServiceEventStore for DieselAdminServiceEventStore<diesel::sqlite::SqliteConnection> {
    fn add_event(
        &self,
        event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?).add_event(event)
    }

    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?)
            .list_events_since(start)
    }

    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?)
            .list_events_by_management_type_since(management_type, start)
    }
}

#[cfg(feature = "postgres")]
impl Clone for DieselAdminServiceEventStore<diesel::pg::PgConnection> {
    fn clone(&self) -> Self {
        Self {
            connection_pool: self.connection_pool.clone(),
        }
    }
}

#[cfg(feature = "postgres")]
impl AdminServiceEventStore for DieselAdminServiceEventStore<diesel::pg::PgConnection> {
    fn add_event(
        &self,
        event: messages::AdminServiceEvent,
    ) -> Result<AdminServiceEvent, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?).add_event(event)
    }

    fn list_events_since(&self, start: i64) -> Result<EventIter, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?)
            .list_events_since(start)
    }

    fn list_events_by_management_type_since(
        &self,
        management_type: String,
        start: i64,
    ) -> Result<EventIter, AdminServiceEventStoreError> {
        AdminServiceEventStoreOperations::new(&*self.connection_pool.get()?)
            .list_events_by_management_type_since(management_type, start)
    }
}

#[cfg(all(test, feature = "sqlite"))]
pub mod tests {
    use super::*;

    use crate::admin::service::event::EventType;
    use crate::admin::store::{
        CircuitProposal, CircuitProposalBuilder, ProposalType, ProposedCircuitBuilder,
        ProposedNodeBuilder, ProposedServiceBuilder,
    };
    use crate::hex::parse_hex;
    use crate::migrations::run_sqlite_migrations;

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };

    #[test]
    /// Verify that an event can be added to the store correctly and then returned by the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create a `messages::AdminServiceEvent`
    /// 4. Add the previously created event to store
    /// 5. List all the events from the store by calling `list_events_since(0)`, which should
    ///    return all events with an ID greater than 0, so all events in the store.
    /// 6. Validate event returned in the list matches the expected values
    fn test_add_list_one_event() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event = create_proposal_submitted_messages_event("test");
        store.add_event(event).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_since(0)
            .expect("Unable to get events from store")
            .collect();
        // Assert only the event added is returned
        assert_eq!(events.len(), 1);
        // Assert the event returned matches the expected values
        assert_eq!(events, vec![create_proposal_submitted_event(1, "test")],);
    }

    #[test]
    /// Verify that events can be added to the store correctly and then returned by the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create two `messages::AdminServiceEvent`s
    /// 4. Add the previously created events to store
    /// 5. List all the events from the store by calling `list_events_since(0)`, which should
    ///    return all events with an ID greater than 0, so all events in the store.
    /// 6. Validate the events returned in the list match the expected values
    fn test_list_since_multiple_events() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event_1 = create_proposal_submitted_messages_event("test");
        store.add_event(event_1).expect("Unable to add event");

        let event_2 = create_circuit_ready_messages_event("test");
        store.add_event(event_2).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_since(0)
            .expect("Unable to get events from store")
            .collect();
        // Assert the expected number of events are returned
        assert_eq!(events.len(), 2);
        // Assert the event returned matches the expected values
        assert_eq!(
            events,
            vec![
                create_proposal_submitted_event(1, "test"),
                create_circuit_ready_event(2, "test")
            ],
        );
    }

    #[test]
    /// Verify that events can be added to the store correctly and then returned by the store
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create three `messages::AdminServiceEvent`s
    /// 4. Add the previously created events to store
    /// 5. List the events in the store since the event with an ID of 1
    /// 6. Validate the events returned in the list match the expected values, and the event with
    ///    the ID of 1 is not included
    fn test_list_since() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event_1 = create_proposal_submitted_messages_event("test");
        store.add_event(event_1).expect("Unable to add event");
        let event_2 = create_circuit_ready_messages_event("test");
        store.add_event(event_2).expect("Unable to add event");
        let event_3 = create_proposal_vote_messages_event("test");
        store.add_event(event_3).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_since(1)
            .expect("Unable to get events from store")
            .collect();
        // Assert the expected number of events are returned
        assert_eq!(events.len(), 2);
        // Assert the event returned matches the expected values
        assert_eq!(
            events,
            vec![
                create_circuit_ready_event(2, "test"),
                create_proposal_vote_event(3, "test")
            ],
        );
    }

    #[test]
    /// Verify that events can be added to the store correctly and then returned by the store with
    /// the correct `circuit_management_type`.
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create three `messages::AdminServiceEvent`s
    /// 4. Add the previously created events to store
    /// 5. List the events in the store since the event with an ID of 0 with a
    ///    `circuit_management_type` equal to "not-test".
    /// 6. Validate event returned in the list matches the expected values, including the
    ///    `CircuitProposal` management type.
    fn test_list_one_event_by_management_type() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event = create_proposal_submitted_messages_event("test");
        store.add_event(event).expect("Unable to add event");

        let event_2 = create_circuit_ready_messages_event("not-test");
        store.add_event(event_2).expect("Unable to add event");
        let event_3 = create_proposal_vote_messages_event("test");
        store.add_event(event_3).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_by_management_type_since("not-test".to_string(), 0)
            .expect("Unable to get events from store")
            .collect();
        // Assert one event is returned
        assert_eq!(events.len(), 1);
        // Assert the event returned matches the expected values, with the "not-test" management type
        assert_eq!(events, vec![create_circuit_ready_event(2, "not-test")],);
    }

    #[test]
    /// Verify that events can be added to the store correctly and then returned by the store with
    /// the correct `circuit_management_type`.
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create three `messages::AdminServiceEvent`s
    /// 4. Add the previously created events to store
    /// 5. List the events in the store since the event with an ID of 1 with a
    ///    `circuit_management_type` equal to "not-test".
    /// 6. Validate event returned in the list matches the expected values, including verifying the
    ///    `CircuitProposal`'s `circuit_management_type` and the event ID is not equal or less than
    ///    2.
    fn test_list_event_by_management_type_since() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event = create_proposal_submitted_messages_event("test");
        store.add_event(event).expect("Unable to add event");
        let event_2 = create_circuit_ready_messages_event("not-test");
        store.add_event(event_2).expect("Unable to add event");
        let event_3 = create_proposal_vote_messages_event("test");
        store.add_event(event_3).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_by_management_type_since("not-test".to_string(), 1)
            .expect("Unable to get events from store")
            .collect();
        // Assert one event is returned
        assert_eq!(events.len(), 1);
        // Assert the event returned matches the expected values, with the "not-test" management type
        assert_eq!(events, vec![create_circuit_ready_event(2, "not-test")],);
    }

    #[test]
    /// Verify that events can be added to the store correctly and then returned by the store with
    /// the correct `circuit_management_type`.
    ///
    /// 1. Run sqlite migrations
    /// 2. Create DieselAdminServiceEventStore
    /// 3. Create three `messages::AdminServiceEvent`s
    /// 4. Add the previously created events to store
    /// 5. List the events in the store since the event with an ID of 0 with a
    ///    `circuit_management_type` equal to "test".
    /// 6. Validate the events returned in the list match the expected values, including the
    ///    `CircuitProposal`'s `circuit_management_type`.
    fn test_list_multiple_events_by_management_type() {
        let pool = create_connection_pool_and_migrate();

        let store = DieselAdminServiceEventStore::_new(pool);
        let event = create_proposal_submitted_messages_event("test");
        store.add_event(event).expect("Unable to add event");
        let event_2 = create_circuit_ready_messages_event("not-test");
        store.add_event(event_2).expect("Unable to add event");
        let event_3 = create_proposal_vote_messages_event("test");
        store.add_event(event_3).expect("Unable to add event");

        let events: Vec<AdminServiceEvent> = store
            .list_events_by_management_type_since("test".to_string(), 0)
            .expect("Unable to get events from store")
            .collect();
        // Assert the expected number of events is returned
        assert_eq!(events.len(), 2);
        // Assert the event returned matches the expected values, with the "test" management type
        assert_eq!(
            events,
            vec![
                create_proposal_submitted_event(1, "test"),
                create_proposal_vote_event(3, "test")
            ],
        );
    }

    fn create_proposal_submitted_event(event_id: i64, management_type: &str) -> AdminServiceEvent {
        AdminServiceEvent {
            event_id,
            event_type: EventType::ProposalSubmitted,
            proposal: create_proposal(management_type),
        }
    }

    fn create_proposal_submitted_messages_event(
        management_type: &str,
    ) -> messages::AdminServiceEvent {
        messages::AdminServiceEvent::ProposalSubmitted(messages::CircuitProposal::from(
            create_proposal(management_type),
        ))
    }

    fn create_circuit_ready_event(event_id: i64, management_type: &str) -> AdminServiceEvent {
        AdminServiceEvent {
            event_id,
            event_type: EventType::CircuitReady,
            proposal: create_proposal(management_type),
        }
    }

    fn create_circuit_ready_messages_event(management_type: &str) -> messages::AdminServiceEvent {
        messages::AdminServiceEvent::CircuitReady(messages::CircuitProposal::from(create_proposal(
            management_type,
        )))
    }

    fn create_proposal_vote_event(event_id: i64, management_type: &str) -> AdminServiceEvent {
        let requester =
            &parse_hex("0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482")
                .unwrap();

        AdminServiceEvent {
            event_id,
            event_type: EventType::ProposalVote {
                requester: requester.to_vec(),
            },
            proposal: create_proposal(management_type),
        }
    }

    fn create_proposal_vote_messages_event(management_type: &str) -> messages::AdminServiceEvent {
        let requester =
            &parse_hex("0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482")
                .unwrap();

        messages::AdminServiceEvent::ProposalVote((
            messages::CircuitProposal::from(create_proposal(management_type)),
            requester.to_vec(),
        ))
    }

    fn create_proposal(management_type: &str) -> CircuitProposal {
        CircuitProposalBuilder::default()
            .with_proposal_type(&ProposalType::Create)
            .with_circuit_id("WBKLF-BBBBB")
            .with_circuit_hash(
                "7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d")
            .with_circuit(
                &ProposedCircuitBuilder::default()
                    .with_circuit_id("WBKLF-BBBBB")
                    .with_roster(&vec![
                        ProposedServiceBuilder::default()
                            .with_service_id("a000")
                            .with_service_type("scabbard")
                            .with_node_id(&"acme-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a001\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service"),
                        ProposedServiceBuilder::default()
                            .with_service_id("a001")
                            .with_service_type("scabbard")
                            .with_node_id(&"bubba-node-000")
                            .with_arguments(&vec![
                                ("peer_services".into(), "[\"a000\"]".into()),
                                ("admin_keys".into(),
                               "[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]".into())
                            ])
                            .build().expect("Unable to build service")
                        ])

                    .with_members(
                        &vec![
                        ProposedNodeBuilder::default()
                            .with_node_id("bubba-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-bubba:8044".into()])
                            .build().expect("Unable to build node"),
                        ProposedNodeBuilder::default()
                            .with_node_id("acme-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                            .build().expect("Unable to build node"),
                        ]
                    )
                    .with_application_metadata(b"test")
                    .with_comments("This is a test")
                    .with_circuit_management_type(management_type)
                    .with_display_name("test_display")
                    .build().expect("Unable to build circuit")
            )
            .with_requester(
                &parse_hex(
                    "0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482").unwrap())
            .with_requester_node_id("acme-node-000")
            .build().expect("Unable to build proposals")
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
