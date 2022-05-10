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

use std::time::SystemTime;

use splinter::service::ServiceId;

use super::message::Scabbard2pcMessage;
use crate::store::scabbard_store::context::ConsensusContext;

#[derive(Debug, PartialEq, Clone)]
pub enum ConsensusAction {
    Update(ConsensusContext, Option<SystemTime>),
    SendMessage(ServiceId, Scabbard2pcMessage),
    Notify(ConsensusActionNotification),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConsensusActionNotification {
    Abort(),
    Commit(),
    MessageDropped(String),
    RequestForStart(),
    CoordinatorRequestForVote(),
    ParticipantRequestForVote(Vec<u8>),
}
