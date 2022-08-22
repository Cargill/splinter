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

//! Client traits to receive AdminServiceEvents.

mod error;
#[cfg(feature = "admin-service-event-client-actix-web-client")]
mod ws;

use super::ProposalSlice;
use std::cmp;
use std::thread;
use std::time::{Duration, Instant};

pub use error::{NextEventError, WaitForError};
#[cfg(feature = "admin-service-event-client-actix-web-client")]
pub use ws::actix_web_client::{
    AwcAdminServiceEventClient, AwcAdminServiceEventClientBuilder,
    RunnableAwcAdminServiceEventClient,
};

/// A public key for the private key that signed an admin proposal.
#[derive(Clone, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// EventQuery represents common event types that can be queried for
pub enum EventQuery<'a> {
    ProposalSubmitted { circuit_id: &'a str },
    ProposalVote { circuit_id: &'a str, key: PublicKey },
    ProposalAccepted { circuit_id: &'a str, key: PublicKey },
    ProposalRejected { circuit_id: &'a str, key: PublicKey },
    CircuitReady { circuit_id: &'a str },
    CircuitDisbanded { circuit_id: &'a str },
}

impl<'a> EventQuery<'a> {
    /// Transform the EventQuery into an event filter
    fn filter(self) -> impl Fn(&AdminServiceEvent) -> bool + 'a {
        move |event: &AdminServiceEvent| match &self {
            EventQuery::ProposalSubmitted { circuit_id } => {
                event.event_type() == &EventType::ProposalSubmitted
                    && &event.proposal().circuit_id == circuit_id
            }
            EventQuery::ProposalVote { circuit_id, key } => match event.event_type() {
                EventType::ProposalVote { requester } => {
                    requester == key && &event.proposal().circuit_id == circuit_id
                }
                _ => false,
            },
            EventQuery::ProposalAccepted { circuit_id, key } => match event.event_type() {
                EventType::ProposalAccepted { requester } => {
                    requester == key && &event.proposal().circuit_id == circuit_id
                }
                _ => false,
            },
            EventQuery::ProposalRejected { circuit_id, key } => match event.event_type() {
                EventType::ProposalRejected { requester } => {
                    requester == key && &event.proposal().circuit_id == circuit_id
                }
                _ => false,
            },
            EventQuery::CircuitReady { circuit_id } => {
                event.event_type() == &EventType::CircuitReady
                    && &event.proposal().circuit_id == circuit_id
            }
            EventQuery::CircuitDisbanded { circuit_id } => {
                event.event_type() == &EventType::CircuitDisbanded
                    && &event.proposal().circuit_id == circuit_id
            }
        }
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

    /// Wait for the given event specified by the filter, until a timeout
    ///
    /// This should block until an event is available.
    fn wait_for_filter(
        &self,
        event_filter: &dyn Fn(&AdminServiceEvent) -> bool,
        timeout: Duration,
    ) -> Result<AdminServiceEvent, WaitForError> {
        let start = Instant::now();
        let poll_rate = Duration::from_millis(100);

        loop {
            if let Some(event) = self
                .try_next_event()
                .map_err(WaitForError::NextEventError)?
            {
                if event_filter(&event) {
                    return Ok(event);
                }
            }

            let elapsed = start.elapsed();
            if timeout < elapsed {
                return Err(WaitForError::TimeoutError);
            }

            let timeleft = timeout - elapsed;
            let sleep_duration = cmp::min(timeleft, poll_rate);
            thread::sleep(sleep_duration);
        }
    }

    /// Wait for the given event until a timeout
    ///
    /// This should block until an event is available.
    fn wait_for(
        &self,
        event_query: EventQuery,
        timeout: Duration,
    ) -> Result<AdminServiceEvent, WaitForError> {
        self.wait_for_filter(&event_query.filter(), timeout)
    }
}

impl AdminServiceEventClient for Box<dyn AdminServiceEventClient> {
    fn try_next_event(&self) -> Result<Option<AdminServiceEvent>, NextEventError> {
        (**self).try_next_event()
    }

    fn next_event(&self) -> Result<AdminServiceEvent, NextEventError> {
        (**self).next_event()
    }
}

pub struct BlockingAdminServiceEventIterator<T>
where
    T: AdminServiceEventClient,
{
    client: T,
}

impl<T> BlockingAdminServiceEventIterator<T>
where
    T: AdminServiceEventClient,
{
    pub fn new(client: T) -> BlockingAdminServiceEventIterator<T> {
        Self { client }
    }
}

impl<T> Iterator for BlockingAdminServiceEventIterator<T>
where
    T: AdminServiceEventClient,
{
    type Item = AdminServiceEvent;

    fn next(&mut self) -> Option<Self::Item> {
        self.client.next_event().ok()
    }
}
