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

//! Event Subscriber Map

use std::cell::RefCell;
use std::collections::HashMap;

use crate::admin::store::AdminServiceEvent;

use super::error::AdminSubscriberError;

pub trait AdminServiceEventSubscriber: Send {
    fn handle_event(
        &self,
        admin_service_event: &AdminServiceEvent,
    ) -> Result<(), AdminSubscriberError>;
}

pub struct SubscriberMap {
    subscribers_by_type: RefCell<HashMap<String, Vec<Box<dyn AdminServiceEventSubscriber>>>>,
}

impl SubscriberMap {
    pub fn new() -> Self {
        Self {
            subscribers_by_type: RefCell::new(HashMap::new()),
        }
    }

    pub fn broadcast_by_type(&self, event_type: &str, admin_service_event: &AdminServiceEvent) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        Self::broadcast(&mut subscribers_by_type, event_type, admin_service_event);
        #[cfg(feature = "admin-service-event-subscriber-glob")]
        Self::broadcast(&mut subscribers_by_type, "*", admin_service_event);
    }

    fn broadcast(
        subscribers_by_type: &mut HashMap<String, Vec<Box<dyn AdminServiceEventSubscriber>>>,
        event_type: &str,
        admin_service_event: &AdminServiceEvent,
    ) {
        if let Some(subscribers) = subscribers_by_type.get_mut(event_type) {
            subscribers.retain(
                |subscriber| match subscriber.handle_event(admin_service_event) {
                    Ok(()) => true,
                    Err(AdminSubscriberError::Unsubscribe) => false,
                    Err(AdminSubscriberError::UnableToHandleEvent(msg)) => {
                        error!("Unable to send event: {}", msg);
                        true
                    }
                },
            );
        }
    }

    pub fn add_subscriber(
        &mut self,
        event_type: String,
        listener: Box<dyn AdminServiceEventSubscriber>,
    ) {
        let mut subscribers_by_type = self.subscribers_by_type.borrow_mut();
        let subscribers = subscribers_by_type
            .entry(event_type)
            .or_insert_with(Vec::new);
        subscribers.push(listener);
    }

    pub fn clear(&mut self) {
        self.subscribers_by_type.borrow_mut().clear()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::{channel, Sender};

    use crate::admin::store::{
        AdminServiceEventBuilder, CircuitProposal, CircuitProposalBuilder, EventType, ProposalType,
        ProposedCircuitBuilder, ProposedNodeBuilder, ProposedServiceBuilder, Vote,
        VoteRecordBuilder,
    };
    use crate::hex::parse_hex;
    use crate::public_key::PublicKey;

    use super::*;

    #[test]
    fn test_subscribe() {
        let (tx, rx) = channel();

        let mut subscribers_map = SubscriberMap::new();
        subscribers_map.add_subscriber("some-type".into(), Box::new(ChannelSubscriber(tx)));

        subscribers_map.broadcast_by_type(
            "another-type",
            &create_circuit_ready_event(1, "another-type"),
        );
        subscribers_map.broadcast_by_type("some-type", &create_circuit_ready_event(2, "some-type"));

        let events: Vec<_> = rx.try_iter().collect();

        assert_eq!(1, events.len());
        assert_eq!(&2, events[0].event_id());
    }

    #[cfg(feature = "admin-service-event-subscriber-glob")]
    #[test]
    fn test_glob_subscribe() {
        let mut subscribers_map = SubscriberMap::new();

        let (tx, std_rx) = channel();
        subscribers_map.add_subscriber("some-type".into(), Box::new(ChannelSubscriber(tx)));

        let (tx, glob_rx) = channel();
        subscribers_map.add_subscriber("*".into(), Box::new(ChannelSubscriber(tx)));

        subscribers_map.broadcast_by_type(
            "another-type",
            &create_circuit_ready_event(1, "another-type"),
        );
        subscribers_map.broadcast_by_type("some-type", &create_circuit_ready_event(2, "some-type"));

        let events: Vec<_> = std_rx.try_iter().collect();

        assert_eq!(1, events.len());
        assert_eq!(&2, events[0].event_id());

        let events: Vec<_> = glob_rx.try_iter().collect();

        assert_eq!(2, events.len());
        assert_eq!(&1, events[0].event_id());
        assert_eq!(&2, events[1].event_id());
    }

    struct ChannelSubscriber(Sender<AdminServiceEvent>);

    impl AdminServiceEventSubscriber for ChannelSubscriber {
        fn handle_event(
            &self,
            admin_service_event: &AdminServiceEvent,
        ) -> Result<(), AdminSubscriberError> {
            self.0
                .send(admin_service_event.clone())
                .map_err(|e| AdminSubscriberError::UnableToHandleEvent(e.to_string()))
        }
    }

    fn create_circuit_ready_event(event_id: i64, management_type: &str) -> AdminServiceEvent {
        AdminServiceEventBuilder::new()
            .with_event_id(event_id)
            .with_event_type(&EventType::CircuitReady)
            .with_proposal(&create_messages_proposal(management_type))
            .build()
            .expect("Unable to build AdminServiceEvent")
    }

    // Creates a admin store `CircuitProposal` that is equivalent to the type of `CircuitProposal`
    // created from an admin::messages::CircuitProposal. Specifically, the `circuit_version`
    // is set to 1.
    fn create_messages_proposal(management_type: &str) -> CircuitProposal {
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
                            .with_endpoints(
                                &vec!["tcps://splinterd-node-bubba:8044".into(),
                                      "tcps://splinterd-node-bubba-2:8044".into()])
                            .build().expect("Unable to build node"),
                        ProposedNodeBuilder::default()
                            .with_node_id("acme-node-000".into())
                            .with_endpoints(&vec!["tcps://splinterd-node-acme:8044".into()])
                            .build().expect("Unable to build node"),
                        ]
                    )
                    .with_circuit_version(1)
                    .with_application_metadata(b"test")
                    .with_comments("This is a test")
                    .with_circuit_management_type(management_type)
                    .with_display_name("test_display")
                    .build()
                    .expect("Unable to build circuit")
            )
            .with_requester(
                &PublicKey::from_bytes(parse_hex(
                    "0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482").unwrap()))
            .with_requester_node_id("acme-node-000")
            .with_votes(&vec![VoteRecordBuilder::new()
                .with_public_key(
                    &PublicKey::from_bytes(parse_hex(
                        "035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550",
                    ).unwrap()),
                )
                .with_vote(&Vote::Accept)
                .with_voter_node_id("bubba-node-000")
                .build()
                .expect("Unable to build vote record"),
                VoteRecordBuilder::new()
                    .with_public_key(
                        &PublicKey::from_bytes(parse_hex(
                            "035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550",
                        )
                        .unwrap()),
                    )
                    .with_vote(&Vote::Accept)
                    .with_voter_node_id("bubba-node-002")
                    .build()
                    .expect("Unable to build vote record")]
            )
            .build().expect("Unable to build proposals")
    }
}
