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

//! Defines an `AdminServiceEvent`, stored and returned by the `AdminServiceEventStore`.
//!
//! The public interface includes the struct [`AdminServiceEvent`].
//!
//! [`AdminServiceEvent`]: struct.AdminServiceEvent.html

pub mod store;

use std::convert::TryFrom;

use crate::admin::service::messages;
use crate::admin::store as admin_store;
use crate::error::InvalidStateError;

#[derive(Debug, PartialEq, Eq)]
/// Representation of an `AdminServiceEvent` defined by the admin messages
pub struct AdminServiceEvent {
    pub event_id: i64,
    pub event_type: EventType,
    pub proposal: admin_store::CircuitProposal,
}

#[derive(Debug, PartialEq, Eq)]
/// Native representation of the `AdminServiceEvent` enum variants
pub enum EventType {
    ProposalSubmitted,
    ProposalVote { requester: Vec<u8> },
    ProposalAccepted { requester: Vec<u8> },
    ProposalRejected { requester: Vec<u8> },
    CircuitReady,
}

impl TryFrom<(i64, &messages::AdminServiceEvent)> for AdminServiceEvent {
    type Error = InvalidStateError;

    fn try_from(
        (event_id, event): (i64, &messages::AdminServiceEvent),
    ) -> Result<Self, Self::Error> {
        let proposal = admin_store::CircuitProposal::try_from(event.proposal())?;
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
        }
    }
}
