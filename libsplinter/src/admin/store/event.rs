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

//! Structs for events associated with the admin store

use std::convert::TryFrom;

use super::CircuitProposal;
use crate::admin::service::messages;
use crate::error::InvalidStateError;

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
    ProposalVote { requester: Vec<u8> },
    ProposalAccepted { requester: Vec<u8> },
    ProposalRejected { requester: Vec<u8> },
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
