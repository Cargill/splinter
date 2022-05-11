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

#[cfg(feature = "scabbardv3-consensus")]
use std::convert::{TryFrom, TryInto as _};

#[cfg(feature = "scabbardv3-consensus")]
use augrim::{error::InternalError, two_phase_commit::TwoPhaseCommitEvent};
use splinter::service::ServiceId;

#[cfg(feature = "scabbardv3-consensus")]
use crate::service::v3::{ScabbardProcess, ScabbardValue};

use super::message::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Alarm(),
    Deliver(ServiceId, Message),
    Start(Vec<u8>),
    Vote(bool),
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<Event> for TwoPhaseCommitEvent<ScabbardProcess, ScabbardValue> {
    type Error = InternalError;

    fn try_from(event: Event) -> Result<Self, Self::Error> {
        Ok(match event {
            Event::Alarm() => Self::Alarm(),
            Event::Deliver(service_id, message) => {
                Self::Deliver(service_id.into(), message.try_into()?)
            }
            Event::Start(val) => Self::Start(val.into()),
            Event::Vote(vote) => Self::Vote(vote),
        })
    }
}
