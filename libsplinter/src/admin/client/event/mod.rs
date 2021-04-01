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

//! Client traits to receive AdminServiceEvents.

mod error;
#[cfg(feature = "admin-service-event-client-actix-web-client")]
mod ws;

use super::ProposalSlice;

pub use error::NextEventError;
#[cfg(feature = "admin-service-event-client-actix-web-client")]
pub use ws::actix_web_client::{
    AwcAdminServiceEventClient, AwcAdminServiceEventClientBuilder,
    RunnableAwcAdminServiceEventClient,
};

/// A public key for the private key that signed an admin proposal.
#[derive(Clone, PartialEq)]
#[repr(transparent)]
pub struct PublicKey(pub Vec<u8>);

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&crate::hex::to_hex(&self.0))
    }
}

#[derive(Clone, Debug)]
/// An admin service event.
///
/// This event relays changes about circuits during the proposal phase through the circuit being
/// ready.
pub struct AdminServiceEvent {
    event_id: u64,
    event_type: EventType,
    proposal: ProposalSlice,
}

/// The event type.
///
/// Some variants include a public key, that is associated with the particular event.
#[derive(Clone, Debug, PartialEq)]
pub enum EventType {
    ProposalSubmitted,
    ProposalVote { requester: PublicKey },
    ProposalAccepted { requester: PublicKey },
    ProposalRejected { requester: PublicKey },
    CircuitReady,
    CircuitDisbanded,
}

impl AdminServiceEvent {
    /// The event's ID
    pub fn event_id(&self) -> &u64 {
        &self.event_id
    }

    /// The event type.
    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    /// The complete proposal
    pub fn proposal(&self) -> &ProposalSlice {
        &self.proposal
    }
}

/// An admin service event client will provide events as they are returned from an admin service.
///
/// It provides two methods, a blocking method and a non-blocking method, depending on the caller's
/// tolerance for blocking a thread.
pub trait AdminServiceEventClient {
    /// Returns the next event, if one is available.
    ///
    /// This should be a non-blocking function.
    fn try_next_event(&self) -> Result<Option<AdminServiceEvent>, NextEventError>;

    /// Returns the next event.
    ///
    /// This should block until an event is available.
    fn next_event(&self) -> Result<AdminServiceEvent, NextEventError>;
}
