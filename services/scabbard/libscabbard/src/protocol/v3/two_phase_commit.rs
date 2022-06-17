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

use crate::protos::prelude::*;
use crate::protos::scabbard_v3;

#[derive(Debug)]
pub enum TwoPhaseCommitMessage {
    VoteRequest(VoteRequest),
    VoteResponse(VoteResponse),
    Commit(Commit),
    Abort(Abort),
    DecisionRequest(DecisionRequest),
    DecisionAck(DecisionAck),
}

impl TwoPhaseCommitMessage {
    pub fn epoch(&self) -> u64 {
        match self {
            Self::VoteRequest(VoteRequest { epoch, .. }) => *epoch,
            Self::VoteResponse(VoteResponse { epoch, .. }) => *epoch,
            Self::Commit(Commit { epoch, .. }) => *epoch,
            Self::Abort(Abort { epoch, .. }) => *epoch,
            Self::DecisionRequest(DecisionRequest { epoch, .. }) => *epoch,
            Self::DecisionAck(DecisionAck { epoch, .. }) => *epoch,
        }
    }
}

#[derive(Debug)]
pub struct VoteRequest {
    pub epoch: u64,
    pub value: Vec<u8>,
}

#[derive(Debug)]
pub struct VoteResponse {
    pub epoch: u64,
    pub response: bool,
}

#[derive(Debug)]
pub struct Commit {
    pub epoch: u64,
}

#[derive(Debug)]
pub struct Abort {
    pub epoch: u64,
}

#[derive(Debug)]
pub struct DecisionRequest {
    pub epoch: u64,
}

#[derive(Debug)]
pub struct DecisionAck {
    pub epoch: u64,
}

impl FromProto<scabbard_v3::VoteRequest> for VoteRequest {
    fn from_proto(source: scabbard_v3::VoteRequest) -> Result<Self, ProtoConversionError> {
        Ok(VoteRequest {
            epoch: source.get_epoch(),
            value: source.get_value().to_vec(),
        })
    }
}

impl FromNative<VoteRequest> for scabbard_v3::VoteRequest {
    fn from_native(source: VoteRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::VoteRequest::new();
        proto_msg.set_epoch(source.epoch);
        proto_msg.set_value(source.value.to_vec());
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::VoteResponse> for VoteResponse {
    fn from_proto(source: scabbard_v3::VoteResponse) -> Result<Self, ProtoConversionError> {
        Ok(VoteResponse {
            epoch: source.get_epoch(),
            response: source.get_response(),
        })
    }
}

impl FromNative<VoteResponse> for scabbard_v3::VoteResponse {
    fn from_native(source: VoteResponse) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::VoteResponse::new();
        proto_msg.set_epoch(source.epoch);
        proto_msg.set_response(source.response);
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::Commit> for Commit {
    fn from_proto(source: scabbard_v3::Commit) -> Result<Self, ProtoConversionError> {
        Ok(Commit {
            epoch: source.get_epoch(),
        })
    }
}

impl FromNative<Commit> for scabbard_v3::Commit {
    fn from_native(source: Commit) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::Commit::new();
        proto_msg.set_epoch(source.epoch);
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::Abort> for Abort {
    fn from_proto(source: scabbard_v3::Abort) -> Result<Self, ProtoConversionError> {
        Ok(Abort {
            epoch: source.get_epoch(),
        })
    }
}

impl FromNative<Abort> for scabbard_v3::Abort {
    fn from_native(source: Abort) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::Abort::new();
        proto_msg.set_epoch(source.epoch);
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::DecisionRequest> for DecisionRequest {
    fn from_proto(source: scabbard_v3::DecisionRequest) -> Result<Self, ProtoConversionError> {
        Ok(DecisionRequest {
            epoch: source.get_epoch(),
        })
    }
}

impl FromNative<DecisionRequest> for scabbard_v3::DecisionRequest {
    fn from_native(source: DecisionRequest) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::DecisionRequest::new();
        proto_msg.set_epoch(source.epoch);
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::DecisionAck> for DecisionAck {
    fn from_proto(source: scabbard_v3::DecisionAck) -> Result<Self, ProtoConversionError> {
        Ok(DecisionAck {
            epoch: source.get_epoch(),
        })
    }
}

impl FromNative<DecisionAck> for scabbard_v3::DecisionAck {
    fn from_native(source: DecisionAck) -> Result<Self, ProtoConversionError> {
        let mut proto_msg = scabbard_v3::DecisionAck::new();
        proto_msg.set_epoch(source.epoch);
        Ok(proto_msg)
    }
}

impl FromProto<scabbard_v3::TwoPhaseCommitMessage> for TwoPhaseCommitMessage {
    fn from_proto(
        mut source: scabbard_v3::TwoPhaseCommitMessage,
    ) -> Result<Self, ProtoConversionError> {
        use scabbard_v3::TwoPhaseCommitMessage_Type::*;
        match source.get_message_type() {
            VOTE_REQUEST => Ok(TwoPhaseCommitMessage::VoteRequest(VoteRequest::from_proto(
                source.take_vote_request(),
            )?)),
            VOTE_RESPONSE => Ok(TwoPhaseCommitMessage::VoteResponse(
                VoteResponse::from_proto(source.take_vote_response())?,
            )),
            COMMIT => Ok(TwoPhaseCommitMessage::Commit(Commit::from_proto(
                source.take_commit(),
            )?)),
            ABORT => Ok(TwoPhaseCommitMessage::Abort(Abort::from_proto(
                source.take_abort(),
            )?)),
            DECISION_REQUEST => Ok(TwoPhaseCommitMessage::DecisionRequest(
                DecisionRequest::from_proto(source.take_decision_request())?,
            )),
            DECISION_ACK => Ok(TwoPhaseCommitMessage::DecisionAck(DecisionAck::from_proto(
                source.take_decision_ack(),
            )?)),
            UNSET => Err(ProtoConversionError::InvalidTypeError(
                "no message type was set".into(),
            )),
        }
    }
}

impl FromNative<TwoPhaseCommitMessage> for scabbard_v3::TwoPhaseCommitMessage {
    fn from_native(source: TwoPhaseCommitMessage) -> Result<Self, ProtoConversionError> {
        use scabbard_v3::TwoPhaseCommitMessage_Type::*;
        let mut proto_msg = scabbard_v3::TwoPhaseCommitMessage::new();

        match source {
            TwoPhaseCommitMessage::VoteRequest(msg) => {
                proto_msg.set_message_type(VOTE_REQUEST);
                proto_msg.set_vote_request(scabbard_v3::VoteRequest::from_native(msg)?)
            }
            TwoPhaseCommitMessage::VoteResponse(msg) => {
                proto_msg.set_message_type(VOTE_RESPONSE);
                proto_msg.set_vote_response(scabbard_v3::VoteResponse::from_native(msg)?)
            }
            TwoPhaseCommitMessage::Commit(msg) => {
                proto_msg.set_message_type(COMMIT);
                proto_msg.set_commit(scabbard_v3::Commit::from_native(msg)?)
            }
            TwoPhaseCommitMessage::Abort(msg) => {
                proto_msg.set_message_type(ABORT);
                proto_msg.set_abort(scabbard_v3::Abort::from_native(msg)?)
            }
            TwoPhaseCommitMessage::DecisionRequest(msg) => {
                proto_msg.set_message_type(DECISION_REQUEST);
                proto_msg.set_decision_request(scabbard_v3::DecisionRequest::from_native(msg)?)
            }
            TwoPhaseCommitMessage::DecisionAck(msg) => {
                proto_msg.set_message_type(DECISION_ACK);
                proto_msg.set_decision_ack(scabbard_v3::DecisionAck::from_native(msg)?)
            }
        }

        Ok(proto_msg)
    }
}
