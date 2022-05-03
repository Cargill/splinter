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
use std::time::SystemTime;

#[cfg(feature = "scabbardv3-consensus")]
use augrim::{
    error::InternalError,
    two_phase_commit::{TwoPhaseCommitAction, TwoPhaseCommitActionNotification},
};
use splinter::service::ServiceId;

#[cfg(feature = "scabbardv3-consensus")]
use crate::service::v3::{ScabbardProcess, ScabbardValue};
use crate::store::scabbard_store::context::ConsensusContext;

use super::message::Message;

#[derive(Debug, PartialEq, Clone)]
pub enum Action {
    Update(ConsensusContext, Option<SystemTime>),
    SendMessage(ServiceId, Message),
    Notify(Notification),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Notification {
    Abort(),
    Commit(),
    MessageDropped(String),
    RequestForStart(),
    CoordinatorRequestForVote(),
    ParticipantRequestForVote(Vec<u8>),
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<TwoPhaseCommitAction<ScabbardProcess, ScabbardValue, SystemTime>> for Action {
    type Error = InternalError;

    fn try_from(
        action: TwoPhaseCommitAction<ScabbardProcess, ScabbardValue, SystemTime>,
    ) -> Result<Self, Self::Error> {
        Ok(match action {
            TwoPhaseCommitAction::Update { context, alarm } => {
                Action::Update(context.try_into()?, alarm)
            }
            TwoPhaseCommitAction::SendMessage(process, message) => {
                Action::SendMessage(process.into(), message.try_into()?)
            }
            TwoPhaseCommitAction::Notify(notification) => Action::Notify(notification.try_into()?),
        })
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<TwoPhaseCommitActionNotification<ScabbardValue>> for Notification {
    type Error = InternalError;

    fn try_from(
        notification: TwoPhaseCommitActionNotification<ScabbardValue>,
    ) -> Result<Self, Self::Error> {
        Ok(match notification {
            TwoPhaseCommitActionNotification::Abort() => Notification::Abort(),
            TwoPhaseCommitActionNotification::Commit() => Notification::Commit(),
            TwoPhaseCommitActionNotification::MessageDropped(msg) => {
                Notification::MessageDropped(msg)
            }
            TwoPhaseCommitActionNotification::RequestForStart() => Notification::RequestForStart(),
            TwoPhaseCommitActionNotification::CoordinatorRequestForVote() => {
                Notification::CoordinatorRequestForVote()
            }
            TwoPhaseCommitActionNotification::ParticipantRequestForVote(val) => {
                Notification::ParticipantRequestForVote(val.into())
            }
        })
    }
}
