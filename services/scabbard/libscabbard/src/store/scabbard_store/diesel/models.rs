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
use std::time::{Duration, SystemTime};

use splinter::error::InternalError;
use splinter::service::{FullyQualifiedServiceId, ServiceId};

use crate::store::scabbard_store::{
    alarm::AlarmType,
    commit::{CommitEntry, CommitEntryBuilder, ConsensusDecision},
    service::{ConsensusType, ScabbardService, ServiceStatus},
    two_phase_commit::{Context, ContextBuilder, Event, Message, Notification, Participant, State},
    ConsensusContext,
};

use super::schema::{
    consensus_2pc_action, consensus_2pc_context, consensus_2pc_context_participant,
    consensus_2pc_deliver_event, consensus_2pc_event, consensus_2pc_notification_action,
    consensus_2pc_send_message_action, consensus_2pc_start_event,
    consensus_2pc_update_context_action, consensus_2pc_update_context_action_participant,
    consensus_2pc_vote_event, scabbard_alarm, scabbard_peer, scabbard_service,
    scabbard_v3_commit_history,
};

/// Database model representation of `ScabbardService`
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_service"]
#[primary_key(service_id)]
pub struct ScabbardServiceModel {
    pub service_id: String,
    pub consensus: String,
    pub status: String,
}

impl From<&ScabbardService> for ScabbardServiceModel {
    fn from(service: &ScabbardService) -> Self {
        ScabbardServiceModel {
            service_id: service.service_id().to_string(),
            consensus: service.consensus().into(),
            status: service.status().into(),
        }
    }
}

/// Database model representation of `ScabbardService` peer
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_peer"]
#[primary_key(service_id, peer_service_id)]
pub struct ScabbardPeerModel {
    pub service_id: String,
    pub peer_service_id: String,
}

impl From<&ScabbardService> for Vec<ScabbardPeerModel> {
    fn from(service: &ScabbardService) -> Self {
        service
            .peers()
            .iter()
            .map(|service_id| ScabbardPeerModel {
                service_id: service.service_id().to_string(),
                peer_service_id: service_id.to_string(),
            })
            .collect::<Vec<ScabbardPeerModel>>()
    }
}

/// Database model representation of `ScabbardService` commit entry
#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_v3_commit_history"]
#[primary_key(service_id, epoch)]
pub struct CommitEntryModel {
    pub service_id: String,
    pub epoch: i64,
    pub value: String,
    pub decision: Option<String>,
}

impl TryFrom<&CommitEntry> for CommitEntryModel {
    type Error = InternalError;

    fn try_from(entry: &CommitEntry) -> Result<Self, Self::Error> {
        Ok(CommitEntryModel {
            service_id: entry.service_id().to_string(),
            epoch: i64::try_from(entry.epoch())
                .map_err(|err| InternalError::from_source(Box::new(err)))?,
            value: entry.value().to_string(),
            decision: entry
                .decision()
                .clone()
                .map(|decision| String::from(&decision)),
        })
    }
}

impl TryFrom<CommitEntryModel> for CommitEntry {
    type Error = InternalError;

    fn try_from(entry: CommitEntryModel) -> Result<Self, Self::Error> {
        let service_id = FullyQualifiedServiceId::new_from_string(entry.service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut builder = CommitEntryBuilder::default()
            .with_service_id(&service_id)
            .with_epoch(
                u64::try_from(entry.epoch)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?,
            )
            .with_value(&entry.value);

        if let Some(d) = entry.decision {
            let decision = ConsensusDecision::try_from(d.as_str())?;
            builder = builder.with_decision(&decision);
        }

        builder
            .build()
            .map_err(|err| InternalError::from_source(Box::new(err)))
    }
}

impl TryFrom<&str> for ServiceStatus {
    type Error = InternalError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "PREPARED" => Ok(ServiceStatus::Prepared),
            "FINALIZED" => Ok(ServiceStatus::Finalized),
            "RETIRED" => Ok(ServiceStatus::Retired),
            _ => Err(InternalError::with_message(format!(
                "Unknown status {}",
                status
            ))),
        }
    }
}

impl From<&ServiceStatus> for String {
    fn from(status: &ServiceStatus) -> Self {
        match *status {
            ServiceStatus::Prepared => "PREPARED".into(),
            ServiceStatus::Finalized => "FINALIZED".into(),
            ServiceStatus::Retired => "RETIRED".into(),
        }
    }
}

impl TryFrom<&str> for ConsensusType {
    type Error = InternalError;

    fn try_from(consensus: &str) -> Result<Self, Self::Error> {
        match consensus {
            "2PC" => Ok(ConsensusType::TwoPC),
            _ => Err(InternalError::with_message(format!(
                "Unknown consensus {}",
                consensus
            ))),
        }
    }
}

impl From<&ConsensusType> for String {
    fn from(consensus: &ConsensusType) -> Self {
        match *consensus {
            ConsensusType::TwoPC => "2PC".into(),
        }
    }
}

impl TryFrom<&str> for ConsensusDecision {
    type Error = InternalError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "ABORT" => Ok(ConsensusDecision::Abort),
            "COMMIT" => Ok(ConsensusDecision::Commit),
            _ => Err(InternalError::with_message(format!(
                "Unknown decision {}",
                status
            ))),
        }
    }
}

impl From<&ConsensusDecision> for String {
    fn from(status: &ConsensusDecision) -> Self {
        match *status {
            ConsensusDecision::Abort => "ABORT".into(),
            ConsensusDecision::Commit => "COMMIT".into(),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "scabbard_alarm"]
#[primary_key(service_id, alarm_type)]
pub struct ScabbardAlarmModel {
    pub service_id: String,
    pub alarm_type: String,
    pub alarm: i64, // timestamp, when to wake up
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context"]
#[primary_key(service_id)]
pub struct Consensus2pcContextModel {
    pub service_id: String,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
}

impl
    TryFrom<(
        &Consensus2pcContextModel,
        Vec<Consensus2pcContextParticipantModel>,
    )> for ConsensusContext
{
    type Error = InternalError;

    fn try_from(
        (context, participants): (
            &Consensus2pcContextModel,
            Vec<Consensus2pcContextParticipantModel>,
        ),
    ) -> Result<Self, Self::Error> {
        let epoch = u64::try_from(context.epoch)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let coordinator = ServiceId::new(&context.coordinator)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        let last_commit_epoch = context
            .last_commit_epoch
            .map(u64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let participants = ParticipantList::try_from(participants)?.inner;

        let state = match context.state.as_str() {
            "WAITINGFORSTART" => State::WaitingForStart,
            "VOTING" => {
                let vote_timeout_start = if let Some(t) = context.vote_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert vote timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voting' but \
                        no vote timeout start time set"
                            .to_string(),
                    ));
                };
                State::Voting { vote_timeout_start }
            }
            "WAITINGFORVOTE" => State::WaitingForVote,
            "ABORT" => State::Abort,
            "COMMIT" => State::Commit,
            "WAITINGFORVOTEREQUEST" => State::WaitingForVoteRequest,
            "VOTED" => {
                let vote = context
                    .vote
                    .as_ref()
                    .map(|v| match v.as_str() {
                        "TRUE" => Some(true),
                        "FALSE" => Some(false),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voted' but vote \
                        is unset"
                        .to_string(),
                    )
                    })?
                    .ok_or_else(|| {
                        InternalError::with_message(
                            "Failed to convert to ConsensusContext, context has state 'voted' but \
                        an invalid vote response was found"
                                .to_string(),
                        )
                    })?;
                let decision_timeout_start = if let Some(t) = context.decision_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert decision timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state 'voted' but \
                        'decision_timeout_start' is unset"
                            .to_string(),
                    ));
                };
                State::Voted {
                    vote,
                    decision_timeout_start,
                }
            }
            _ => {
                return Err(InternalError::with_message(
                    "Failed to convert to ConsensusContext, invalid state value found".to_string(),
                ))
            }
        };

        let mut builder = ContextBuilder::default()
            .with_coordinator(&coordinator)
            .with_epoch(epoch)
            .with_state(state)
            .with_participants(participants)
            .with_this_process(
                FullyQualifiedServiceId::new_from_string(&context.service_id)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .service_id(),
            );

        if let Some(last_commit_epoch) = last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }
        let context = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(ConsensusContext::TwoPhaseCommit(context))
    }
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for Consensus2pcContextModel {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let last_commit_epoch = context
            .last_commit_epoch()
            .map(i64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let (vote_timeout_start, vote, decision_timeout_start) = match context.state() {
            State::Voting { vote_timeout_start } => {
                let time = i64::try_from(
                    vote_timeout_start
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                (Some(time), None, None)
            }
            State::Voted {
                vote,
                decision_timeout_start,
            } => {
                let time = i64::try_from(
                    decision_timeout_start
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let vote = match vote {
                    true => "TRUE",
                    false => "FALSE",
                };
                (None, Some(vote.to_string()), Some(time))
            }
            _ => (None, None, None),
        };
        let state = String::from(context.state());
        Ok(Consensus2pcContextModel {
            service_id: format!("{}", service_id),
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context_participant"]
#[primary_key(service_id, process)]
pub struct Consensus2pcContextParticipantModel {
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct ParticipantList {
    pub inner: Vec<Participant>,
}

impl TryFrom<Vec<Consensus2pcContextParticipantModel>> for ParticipantList {
    type Error = InternalError;

    fn try_from(
        participants: Vec<Consensus2pcContextParticipantModel>,
    ) -> Result<Self, Self::Error> {
        let mut all_participants = Vec::new();
        for p in participants {
            let vote = if let Some(vote) = p.vote {
                match vote.as_str() {
                    "TRUE" => Some(true),
                    "FALSE" => Some(false),
                    _ => {
                        return Err(InternalError::with_message(format!(
                        "Failed to convert context participant model to participant, invalid vote \
                        value found: {}",
                        vote,
                    )))
                    }
                }
            } else {
                None
            };
            all_participants.push(Participant {
                process: ServiceId::new(p.process)
                    .map_err(|e| InternalError::from_source(Box::new(e)))?,
                vote,
            });
        }
        Ok(ParticipantList {
            inner: all_participants,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct ContextParticipantList {
    pub inner: Vec<Consensus2pcContextParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for ContextParticipantList {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let mut participants = Vec::new();
        for participant in context.participants() {
            let vote = participant.vote.map(|vote| match vote {
                true => "TRUE".to_string(),
                false => "FALSE".to_string(),
            });
            participants.push(Consensus2pcContextParticipantModel {
                service_id: format!("{}", service_id),
                epoch,
                process: format!("{}", participant.process),
                vote,
            })
        }
        Ok(ContextParticipantList {
            inner: participants,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_context_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcUpdateContextActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub action_alarm: Option<i64>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64, &Option<i64>)>
    for Consensus2pcUpdateContextActionModel
{
    type Error = InternalError;

    fn try_from(
        (context, service_id, action_id, action_alarm): (
            &Context,
            &FullyQualifiedServiceId,
            &i64,
            &Option<i64>,
        ),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let last_commit_epoch = context
            .last_commit_epoch()
            .map(i64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let (vote_timeout_start, vote, decision_timeout_start) = match context.state() {
            State::Voting { vote_timeout_start } => {
                let time = i64::try_from(
                    vote_timeout_start
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                (Some(time), None, None)
            }
            State::Voted {
                vote,
                decision_timeout_start,
            } => {
                let time = i64::try_from(
                    decision_timeout_start
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?
                        .as_secs(),
                )
                .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let vote = match vote {
                    true => "TRUE",
                    false => "FALSE",
                };
                (None, Some(vote.to_string()), Some(time))
            }
            _ => (None, None, None),
        };
        let state = String::from(context.state());
        Ok(Consensus2pcUpdateContextActionModel {
            action_id: *action_id,
            service_id: format!("{}", service_id),
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
            action_alarm: *action_alarm,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_context_action_participant"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[belongs_to(Consensus2pcUpdateContextActionModel, foreign_key = "action_id")]
#[primary_key(action_id, process)]
pub struct Consensus2pcUpdateContextActionParticipantModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct UpdateContextActionParticipantList {
    pub inner: Vec<Consensus2pcUpdateContextActionParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64)> for UpdateContextActionParticipantList {
    type Error = InternalError;

    fn try_from(
        (context, service_id, action_id): (&Context, &FullyQualifiedServiceId, &i64),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let mut participants = Vec::new();
        for participant in context.participants() {
            let vote = participant.vote.map(|vote| match vote {
                true => "TRUE".to_string(),
                false => "FALSE".to_string(),
            });
            participants.push(Consensus2pcUpdateContextActionParticipantModel {
                action_id: *action_id,
                service_id: format!("{}", service_id),
                epoch,
                process: format!("{}", participant.process),
                vote,
            })
        }
        Ok(UpdateContextActionParticipantList {
            inner: participants,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_send_message_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcSendMessageActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: String,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_notification_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcNotificationModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub notification_type: String,
    pub dropped_message: Option<String>,
    pub request_for_vote_value: Option<Vec<u8>>,
}

impl From<&State> for String {
    fn from(state: &State) -> Self {
        match *state {
            State::WaitingForStart => String::from("WAITINGFORSTART"),
            State::Voting { .. } => String::from("VOTING"),
            State::WaitingForVote => String::from("WAITINGFORVOTE"),
            State::Abort => String::from("ABORT"),
            State::Commit => String::from("COMMIT"),
            State::WaitingForVoteRequest => String::from("WAITINGFORVOTEREQUEST"),
            State::Voted { .. } => String::from("VOTED"),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_action"]
#[belongs_to(Consensus2pcContextModel, foreign_key = "service_id")]
#[primary_key(id)]
pub struct Consensus2pcActionModel {
    pub id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
    pub position: i32,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_action"]
pub struct InsertableConsensus2pcActionModel {
    pub service_id: String,
    pub epoch: i64,
    pub executed_at: Option<i64>,
    pub position: i32,
}

impl From<&Notification> for String {
    fn from(notification: &Notification) -> Self {
        match *notification {
            Notification::RequestForStart() => String::from("REQUESTFORSTART"),
            Notification::CoordinatorRequestForVote() => String::from("COORDINATORREQUESTFORVOTE"),
            Notification::ParticipantRequestForVote(_) => String::from("PARTICIPANTREQUESTFORVOTE"),
            Notification::Commit() => String::from("COMMIT"),
            Notification::Abort() => String::from("ABORT"),
            Notification::MessageDropped(_) => String::from("MESSAGEDROPPED"),
        }
    }
}

impl From<&Message> for String {
    fn from(message: &Message) -> Self {
        match *message {
            Message::VoteRequest(..) => String::from("VOTEREQUEST"),
            Message::DecisionRequest(_) => String::from("DECISIONREQUEST"),
            Message::VoteResponse(..) => String::from("VOTERESPONSE"),
            Message::Commit(_) => String::from("COMMIT"),
            Message::Abort(_) => String::from("ABORT"),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_event"]
#[primary_key(id)]
pub struct Consensus2pcEventModel {
    pub id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
    pub position: i32,
    pub event_type: String,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_event"]
pub struct InsertableConsensus2pcEventModel {
    pub service_id: String,
    pub epoch: i64,
    pub executed_at: Option<i64>,
    pub position: i32,
    pub event_type: String,
}

impl From<&Event> for String {
    fn from(event: &Event) -> Self {
        match *event {
            Event::Alarm() => "ALARM".into(),
            Event::Deliver(..) => "DELIVER".into(),
            Event::Start(..) => "START".into(),
            Event::Vote(..) => "VOTE".into(),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_deliver_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcDeliverEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: String,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_start_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcStartEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub value: Vec<u8>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_vote_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcVoteEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub vote: String, // TRUE or FALSE
}

impl From<&AlarmType> for String {
    fn from(status: &AlarmType) -> Self {
        match *status {
            AlarmType::TwoPhaseCommit => "TWOPHASECOMMIT".into(),
        }
    }
}
