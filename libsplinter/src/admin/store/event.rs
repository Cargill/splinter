// Copyright 2018-2022 Cargill Incorporated
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

//! Structs for events associated with the admin store

use std::convert::TryFrom;

use super::CircuitProposal;
use crate::admin::service::messages;
use crate::error::InvalidStateError;

/// Represents the `requester`'s public key associated with an `AdminServiceEvent`
pub type PublicKey = Vec<u8>;

#[derive(Debug, PartialEq, Eq, Clone)]
/// Representation of an `AdminServiceEvent` defined by the admin messages
pub struct AdminServiceEvent {
    event_id: i64,
    event_type: EventType,
    proposal: CircuitProposal,
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// Native representation of the `AdminServiceEvent` enum variants
pub enum EventType {
    ProposalSubmitted,
    ProposalVote { requester: PublicKey },
    ProposalAccepted { requester: PublicKey },
    ProposalRejected { requester: PublicKey },
    CircuitReady,
    CircuitDisbanded,
}

impl AdminServiceEvent {
    pub fn event_id(&self) -> &i64 {
        &self.event_id
    }

    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    pub fn proposal(&self) -> &CircuitProposal {
        &self.proposal
    }
}

/// Builder to be used to build an `AdminServiceEvent`
#[derive(Default, Clone)]
pub struct AdminServiceEventBuilder {
    event_id: Option<i64>,
    event_type: Option<EventType>,
    proposal: Option<CircuitProposal>,
}

impl AdminServiceEventBuilder {
    /// Creates a new `AdminServiceEventBuilder`
    pub fn new() -> Self {
        AdminServiceEventBuilder::default()
    }

    /// Sets the event ID
    ///
    /// # Arguments
    ///
    /// * `event_id` - The ID of the event
    pub fn with_event_id(mut self, event_id: i64) -> AdminServiceEventBuilder {
        self.event_id = Some(event_id);
        self
    }

    /// Sets the event type
    ///
    /// # Arguments
    ///
    /// * `event_type` - The type of event
    pub fn with_event_type(mut self, event_type: &EventType) -> AdminServiceEventBuilder {
        self.event_type = Some(event_type.clone());
        self
    }

    /// Sets the event's circuit proposal
    ///
    /// # Arguments
    ///
    /// * `proposal` - Circuit proposal associated with the event
    pub fn with_proposal(mut self, proposal: &CircuitProposal) -> AdminServiceEventBuilder {
        self.proposal = Some(proposal.clone());
        self
    }

    /// Builds an `AdminServiceEvent`
    ///
    /// Returns an error if any of the fields are not set.
    pub fn build(self) -> Result<AdminServiceEvent, InvalidStateError> {
        let event_id = self.event_id.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `event_id`".to_string(),
            )
        })?;

        let event_type = self.event_type.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `event_type`".to_string(),
            )
        })?;

        let proposal = self.proposal.ok_or_else(|| {
            InvalidStateError::with_message(
                "unable to build, missing field: `proposal`".to_string(),
            )
        })?;

        let admin_service_event = AdminServiceEvent {
            event_id,
            event_type,
            proposal,
        };

        Ok(admin_service_event)
    }
}

impl TryFrom<(i64, &messages::AdminServiceEvent)> for AdminServiceEvent {
    type Error = InvalidStateError;

    fn try_from(
        (event_id, event): (i64, &messages::AdminServiceEvent),
    ) -> Result<Self, Self::Error> {
        let proposal = CircuitProposal::try_from(event.proposal())?;
        match event {
            messages::AdminServiceEvent::ProposalSubmitted(_) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::ProposalSubmitted,
                proposal,
            }),
            messages::AdminServiceEvent::ProposalVote((_, data)) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::ProposalVote {
                    requester: data.to_vec(),
                },
                proposal,
            }),
            messages::AdminServiceEvent::ProposalAccepted((_, data)) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::ProposalAccepted {
                    requester: data.to_vec(),
                },
                proposal,
            }),
            messages::AdminServiceEvent::ProposalRejected((_, data)) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::ProposalRejected {
                    requester: data.to_vec(),
                },
                proposal,
            }),
            messages::AdminServiceEvent::CircuitReady(_) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::CircuitReady,
                proposal,
            }),
            messages::AdminServiceEvent::CircuitDisbanded(_) => Ok(AdminServiceEvent {
                event_id,
                event_type: EventType::CircuitDisbanded,
                proposal,
            }),
        }
    }
}
