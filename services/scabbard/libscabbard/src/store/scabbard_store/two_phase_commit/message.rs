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

use std::convert::TryFrom;

#[cfg(feature = "scabbardv3-consensus")]
use augrim::error::InternalError;
#[cfg(feature = "scabbardv3-consensus")]
use augrim::two_phase_commit::TwoPhaseCommitMessage as AugrimTwoPhaseCommitMessage;

use crate::protocol::v3::{
    message::ScabbardMessage,
    two_phase_commit::{
        Abort, Commit, DecisionRequest, TwoPhaseCommitMessage, VoteRequest, VoteResponse,
    },
};
use crate::protos::{scabbard_v3, IntoBytes, ProtoConversionError};
#[cfg(feature = "scabbardv3-consensus")]
use crate::service::v3::ScabbardValue;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    VoteRequest(u64, Vec<u8>),
    Commit(u64),
    Abort(u64),
    DecisionRequest(u64),
    VoteResponse(u64, bool),
}

impl TryFrom<Message> for ScabbardMessage {
    type Error = ProtoConversionError;

    fn try_from(store_msg: Message) -> Result<Self, Self::Error> {
        let msg_2pc = match store_msg {
            Message::VoteRequest(epoch, value) => {
                TwoPhaseCommitMessage::VoteRequest(VoteRequest { epoch, value })
            }
            Message::Commit(epoch) => TwoPhaseCommitMessage::Commit(Commit { epoch }),
            Message::Abort(epoch) => TwoPhaseCommitMessage::Abort(Abort { epoch }),
            Message::DecisionRequest(epoch) => {
                TwoPhaseCommitMessage::DecisionRequest(DecisionRequest { epoch })
            }
            Message::VoteResponse(epoch, response) => {
                TwoPhaseCommitMessage::VoteResponse(VoteResponse { epoch, response })
            }
        };

        Ok(ScabbardMessage::ConsensusMessage(IntoBytes::<
            scabbard_v3::TwoPhaseCommitMessage,
        >::into_bytes(
            msg_2pc
        )?))
    }
}

impl TryFrom<Message> for Vec<u8> {
    type Error = ProtoConversionError;

    fn try_from(store_msg: Message) -> Result<Self, Self::Error> {
        IntoBytes::<scabbard_v3::ScabbardMessageV3>::into_bytes(ScabbardMessage::try_from(
            store_msg,
        )?)
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<AugrimTwoPhaseCommitMessage<ScabbardValue>> for Message {
    type Error = InternalError;

    fn try_from(msg: AugrimTwoPhaseCommitMessage<ScabbardValue>) -> Result<Self, Self::Error> {
        Ok(match msg {
            AugrimTwoPhaseCommitMessage::VoteRequest(epoch, val) => {
                Self::VoteRequest(epoch, val.into())
            }
            AugrimTwoPhaseCommitMessage::Commit(epoch) => Self::Commit(epoch),
            AugrimTwoPhaseCommitMessage::Abort(epoch) => Self::Abort(epoch),
            AugrimTwoPhaseCommitMessage::DecisionRequest(epoch) => Self::DecisionRequest(epoch),
            AugrimTwoPhaseCommitMessage::VoteResponse(epoch, vote) => {
                Self::VoteResponse(epoch, vote)
            }
        })
    }
}

#[cfg(feature = "scabbardv3-consensus")]
impl TryFrom<Message> for AugrimTwoPhaseCommitMessage<ScabbardValue> {
    type Error = InternalError;

    fn try_from(msg: Message) -> Result<Self, Self::Error> {
        Ok(match msg {
            Message::VoteRequest(epoch, val) => Self::VoteRequest(epoch, val.into()),
            Message::Commit(epoch) => Self::Commit(epoch),
            Message::Abort(epoch) => Self::Abort(epoch),
            Message::DecisionRequest(epoch) => Self::DecisionRequest(epoch),
            Message::VoteResponse(epoch, vote) => Self::VoteResponse(epoch, vote),
        })
    }
}
