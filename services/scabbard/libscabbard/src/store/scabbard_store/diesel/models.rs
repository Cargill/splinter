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
    commit::{CommitEntry, CommitEntryBuilder, ConsensusDecision},
    context::{
        Context, ContextBuilder, CoordinatorContext, CoordinatorState, Participant,
        ParticipantContext, ParticipantState, ScabbardContext,
    },
    service::{ConsensusType, ScabbardService, ServiceStatus},
    two_phase::{
        action::ConsensusActionNotification, event::Scabbard2pcEvent, message::Scabbard2pcMessage,
        state::Scabbard2pcState,
    },
};

use super::schema::{
    consensus_2pc_action, consensus_2pc_consensus_coordinator_context,
    consensus_2pc_consensus_coordinator_context_participant,
    consensus_2pc_coordinator_notification_action, consensus_2pc_coordinator_send_message_action,
    consensus_2pc_participant_context, consensus_2pc_participant_context_participant,
    consensus_2pc_participant_notification_action, consensus_2pc_participant_send_message_action,
    consensus_2pc_update_coordinator_context_action,
    consensus_2pc_update_coordinator_context_action_participant,
    consensus_2pc_update_participant_context_action,
    consensus_2pc_update_participant_context_action_participant, scabbard_peer, scabbard_service,
    scabbard_v3_commit_history, two_pc_consensus_deliver_event, two_pc_consensus_event,
    two_pc_consensus_start_event, two_pc_consensus_vote_event,
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

impl From<&CommitEntry> for CommitEntryModel {
    fn from(entry: &CommitEntry) -> Self {
        CommitEntryModel {
            service_id: entry.service_id().to_string(),
            epoch: entry.epoch(),
            value: entry.value().to_string(),
            decision: entry
                .decision()
                .clone()
                .map(|decision| String::from(&decision)),
        }
    }
}

impl TryFrom<CommitEntryModel> for CommitEntry {
    type Error = InternalError;

    fn try_from(entry: CommitEntryModel) -> Result<Self, Self::Error> {
        let service_id = FullyQualifiedServiceId::new_from_string(entry.service_id)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut builder = CommitEntryBuilder::default()
            .with_service_id(&service_id)
            .with_epoch(entry.epoch)
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
#[table_name = "consensus_2pc_consensus_coordinator_context"]
#[primary_key(service_id, epoch)]
pub struct Consensus2pcCoordinatorContextModel {
    pub service_id: String,
    pub alarm: Option<i64>, // timestamp, when to wake up
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote_timeout_start: Option<i64>,
}

impl
    TryFrom<(
        &Consensus2pcCoordinatorContextModel,
        Vec<Consensus2pcCoordinatorContextParticipantModel>,
    )> for ScabbardContext
{
    type Error = InternalError;

    fn try_from(
        (context, participants): (
            &Consensus2pcCoordinatorContextModel,
            Vec<Consensus2pcCoordinatorContextParticipantModel>,
        ),
    ) -> Result<Self, Self::Error> {
        let epoch = u64::try_from(context.epoch)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let alarm = if let Some(alarm) = context.alarm {
            Some(
                SystemTime::UNIX_EPOCH
                    .checked_add(Duration::from_secs(alarm as u64))
                    .ok_or_else(|| {
                        InternalError::with_message(
                            "'alarm' timestamp could not be represented as a `SystemTime`"
                                .to_string(),
                        )
                    })?,
            )
        } else {
            None
        };
        let coordinator = ServiceId::new(&context.coordinator)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        let last_commit_epoch = context
            .last_commit_epoch
            .map(u64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let participants = CoordinatorParticipantList::try_from(participants)?.inner;

        let state = match context.state.as_str() {
            "WAITINGFORSTART" => Scabbard2pcState::WaitingForStart,
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
                        "Failed to convert to ScabbardContext, context has state 'voting' but 
                        no vote timeout start time set"
                            .to_string(),
                    ));
                };
                Scabbard2pcState::Voting { vote_timeout_start }
            }
            "WAITINGFORVOTE" => Scabbard2pcState::WaitingForVote,
            "ABORT" => Scabbard2pcState::Abort,
            "COMMIT" => Scabbard2pcState::Commit,
            _ => {
                return Err(InternalError::with_message(
                    "Failed to convert to ScabbardContext, invalid state value found".to_string(),
                ))
            }
        };

        let mut builder = ContextBuilder::default()
            .with_coordinator(&coordinator)
            .with_epoch(epoch)
            .with_state(state)
            .with_participants(participants)
            .with_this_process(&coordinator);

        if let Some(alarm) = alarm {
            builder = builder.with_alarm(alarm);
        }
        if let Some(last_commit_epoch) = last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }
        let context = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(ScabbardContext::Scabbard2pcContext(context))
    }
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for Consensus2pcCoordinatorContextModel {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        match CoordinatorContext::try_from(context.role_context().clone()) {
            Ok(coordinator_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let last_commit_epoch = context
                    .last_commit_epoch()
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let vote_timeout_start = match coordinator_context.state {
                    CoordinatorState::Voting { vote_timeout_start } => {
                        let time = i64::try_from(
                            vote_timeout_start
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .map_err(|err| InternalError::from_source(Box::new(err)))?
                                .as_secs(),
                        )
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                        Some(time)
                    }
                    _ => None,
                };
                let state = String::from(&coordinator_context.state);
                let alarm = context
                    .alarm()
                    .map(|a| {
                        a.duration_since(SystemTime::UNIX_EPOCH)
                            .map(|r| i64::try_from(r.as_secs()))
                    })
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Ok(Consensus2pcCoordinatorContextModel {
                    service_id: format!("{}", service_id),
                    alarm,
                    coordinator: format!("{}", context.coordinator()),
                    epoch,
                    last_commit_epoch,
                    state,
                    vote_timeout_start,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_consensus_coordinator_context_participant"]
#[primary_key(service_id, epoch, process)]
pub struct Consensus2pcCoordinatorContextParticipantModel {
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct CoordinatorParticipantList {
    pub inner: Vec<Participant>,
}

impl TryFrom<Vec<Consensus2pcCoordinatorContextParticipantModel>> for CoordinatorParticipantList {
    type Error = InternalError;

    fn try_from(
        participants: Vec<Consensus2pcCoordinatorContextParticipantModel>,
    ) -> Result<Self, Self::Error> {
        let mut all_participants = Vec::new();
        for p in participants {
            let vote = if let Some(vote) = p.vote {
                match vote.as_str() {
                    "TRUE" => Some(true),
                    "FALSE" => Some(false),
                    _ => return Err(InternalError::with_message(
                        "Failed to convert coordinator context participant model to participant, 
                        invalid vote value found"
                            .to_string(),
                    )),
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
        Ok(CoordinatorParticipantList {
            inner: all_participants,
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct CoordinatorContextParticipantList {
    pub inner: Vec<Consensus2pcCoordinatorContextParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for CoordinatorContextParticipantList {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        match CoordinatorContext::try_from(context.role_context().clone()) {
            Ok(coordinator_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let mut coordinator_participants = Vec::new();
                for participant in coordinator_context.participants {
                    let vote = participant.vote.map(|vote| match vote {
                        true => "TRUE".to_string(),
                        false => "FALSE".to_string(),
                    });
                    coordinator_participants.push(Consensus2pcCoordinatorContextParticipantModel {
                        service_id: format!("{}", service_id),
                        epoch,
                        process: format!("{}", participant.process),
                        vote,
                    })
                }
                Ok(CoordinatorContextParticipantList {
                    inner: coordinator_participants,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_coordinator_context_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcUpdateCoordinatorContextActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub alarm: Option<i64>,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote_timeout_start: Option<i64>,
    pub coordinator_action_alarm: Option<i64>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64, &Option<i64>)>
    for Consensus2pcUpdateCoordinatorContextActionModel
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
        match CoordinatorContext::try_from(context.role_context().clone()) {
            Ok(coordinator_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let last_commit_epoch = context
                    .last_commit_epoch()
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let vote_timeout_start = match coordinator_context.state {
                    CoordinatorState::Voting { vote_timeout_start } => {
                        let time = i64::try_from(
                            vote_timeout_start
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .map_err(|err| InternalError::from_source(Box::new(err)))?
                                .as_secs(),
                        )
                        .map_err(|err| InternalError::from_source(Box::new(err)))?;
                        Some(time)
                    }
                    _ => None,
                };
                let state = String::from(&coordinator_context.state);
                let alarm = context
                    .alarm()
                    .map(|a| {
                        a.duration_since(SystemTime::UNIX_EPOCH)
                            .map(|r| i64::try_from(r.as_secs()))
                    })
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Ok(Consensus2pcUpdateCoordinatorContextActionModel {
                    action_id: *action_id,
                    service_id: format!("{}", service_id),
                    alarm,
                    coordinator: format!("{}", context.coordinator()),
                    epoch,
                    last_commit_epoch,
                    state,
                    vote_timeout_start,
                    coordinator_action_alarm: *action_alarm,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_coordinator_context_action_participant"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[belongs_to(
    Consensus2pcUpdateCoordinatorContextActionModel,
    foreign_key = "action_id"
)]
#[primary_key(action_id, process)]
pub struct Consensus2pcUpdateCoordinatorContextActionParticipantModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct UpdateCoordinatorContextActionParticipantList {
    pub inner: Vec<Consensus2pcUpdateCoordinatorContextActionParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64)>
    for UpdateCoordinatorContextActionParticipantList
{
    type Error = InternalError;

    fn try_from(
        (context, service_id, action_id): (&Context, &FullyQualifiedServiceId, &i64),
    ) -> Result<Self, Self::Error> {
        match CoordinatorContext::try_from(context.role_context().clone()) {
            Ok(coordinator_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let mut coordinator_participants = Vec::new();
                for participant in coordinator_context.participants {
                    let vote = participant.vote.map(|vote| match vote {
                        true => "TRUE".to_string(),
                        false => "FALSE".to_string(),
                    });
                    coordinator_participants.push(
                        Consensus2pcUpdateCoordinatorContextActionParticipantModel {
                            action_id: *action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            process: format!("{}", participant.process),
                            vote,
                        },
                    )
                }
                Ok(UpdateCoordinatorContextActionParticipantList {
                    inner: coordinator_participants,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_coordinator_send_message_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcCoordinatorSendMessageActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: String,
    pub vote_response: Option<String>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_coordinator_notification_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcCoordinatorNotificationModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub notification_type: String,
    pub dropped_message: Option<String>,
}

impl From<&CoordinatorState> for String {
    fn from(state: &CoordinatorState) -> Self {
        match *state {
            CoordinatorState::WaitingForStart => String::from("WAITINGFORSTART"),
            CoordinatorState::Voting { .. } => String::from("VOTING"),
            CoordinatorState::WaitingForVote => String::from("WAITINGFORVOTE"),
            CoordinatorState::Abort => String::from("ABORT"),
            CoordinatorState::Commit => String::from("COMMIT"),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_action"]
#[belongs_to(Consensus2pcCoordinatorContextModel, foreign_key = "service_id")]
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

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_participant_context"]
#[primary_key(service_id, epoch)]
pub struct Consensus2pcParticipantContextModel {
    pub service_id: String,
    pub alarm: Option<i64>,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
}

impl
    TryFrom<(
        &Consensus2pcParticipantContextModel,
        Vec<Consensus2pcParticipantContextParticipantModel>,
    )> for ScabbardContext
{
    type Error = InternalError;

    fn try_from(
        (context, participants): (
            &Consensus2pcParticipantContextModel,
            Vec<Consensus2pcParticipantContextParticipantModel>,
        ),
    ) -> Result<Self, Self::Error> {
        let epoch = u64::try_from(context.epoch)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let alarm = if let Some(alarm) = context.alarm {
            Some(
                SystemTime::UNIX_EPOCH
                    .checked_add(Duration::from_secs(alarm as u64))
                    .ok_or_else(|| {
                        InternalError::with_message(
                            "'alarm' timestamp could not be represented as a `SystemTime`"
                                .to_string(),
                        )
                    })?,
            )
        } else {
            None
        };
        let coordinator = ServiceId::new(&context.coordinator)
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        let last_commit_epoch = context
            .last_commit_epoch
            .map(u64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let state = match context.state.as_str() {
            "WAITINGFORVOTEREQUEST" => Scabbard2pcState::WaitingForVoteRequest,
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
                        "Failed to convert to ScabbardContext, context has state 'voted' but vote
                        is unset"
                        .to_string(),
                    )
                    })?
                    .ok_or_else(|| {
                        InternalError::with_message(
                            "Failed to convert to ScabbardContext, context has state 'voted' but an
                        invalid vote response was found"
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
                        "Failed to convert to ScabbardContext, context has state 'voted' but 
                        'decision_timeout_start' is unset"
                            .to_string(),
                    ));
                };
                Scabbard2pcState::Voted {
                    vote,
                    decision_timeout_start,
                }
            }
            "WAITINGFORVOTE" => Scabbard2pcState::WaitingForVote,
            "ABORT" => Scabbard2pcState::Abort,
            "COMMIT" => Scabbard2pcState::Commit,
            _ => {
                return Err(InternalError::with_message(
                    "Failed to convert to ScabbardContext, invalid state value found".to_string(),
                ))
            }
        };
        let participant_processes = participants
            .into_iter()
            .map(|p| ServiceId::new(p.process))
            .collect::<Result<Vec<ServiceId>, _>>()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        let mut builder = ContextBuilder::default()
            .with_coordinator(&coordinator)
            .with_epoch(epoch)
            .with_state(state)
            .with_participant_processes(participant_processes)
            .with_this_process(
                FullyQualifiedServiceId::new_from_string(&context.service_id)
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .service_id(),
            );

        if let Some(alarm) = alarm {
            builder = builder.with_alarm(alarm);
        }
        if let Some(last_commit_epoch) = last_commit_epoch {
            builder = builder.with_last_commit_epoch(last_commit_epoch);
        }
        let context = builder
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        Ok(ScabbardContext::Scabbard2pcContext(context))
    }
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for Consensus2pcParticipantContextModel {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        match ParticipantContext::try_from(context.role_context().clone()) {
            Ok(participant_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let last_commit_epoch = context
                    .last_commit_epoch()
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let (vote, decision_timeout_start) = match participant_context.state {
                    ParticipantState::Voted {
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
                        match vote {
                            true => (Some("TRUE".to_string()), Some(time)),
                            false => (Some("FALSE".to_string()), Some(time)),
                        }
                    }
                    _ => (None, None),
                };
                let state = String::from(&participant_context.state);
                let alarm = context
                    .alarm()
                    .map(|a| {
                        a.duration_since(SystemTime::UNIX_EPOCH)
                            .map(|r| i64::try_from(r.as_secs()))
                    })
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Ok(Consensus2pcParticipantContextModel {
                    service_id: format!("{}", service_id),
                    alarm,
                    coordinator: format!("{}", context.coordinator()),
                    epoch,
                    last_commit_epoch,
                    state,
                    vote,
                    decision_timeout_start,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_participant_context_participant"]
#[primary_key(service_id, epoch, process)]
pub struct Consensus2pcParticipantContextParticipantModel {
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
}

#[derive(Debug, PartialEq)]
pub struct ParticipantContextParticipantList {
    pub inner: Vec<Consensus2pcParticipantContextParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId)> for ParticipantContextParticipantList {
    type Error = InternalError;

    fn try_from(
        (context, service_id): (&Context, &FullyQualifiedServiceId),
    ) -> Result<Self, Self::Error> {
        match ParticipantContext::try_from(context.role_context().clone()) {
            Ok(participant_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let mut participants = Vec::new();
                for participant in participant_context.participant_processes {
                    participants.push(Consensus2pcParticipantContextParticipantModel {
                        service_id: format!("{}", service_id),
                        epoch,
                        process: format!("{}", participant),
                    })
                }
                Ok(ParticipantContextParticipantList {
                    inner: participants,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_participant_context_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcUpdateParticipantContextActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub alarm: Option<i64>,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: String,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub participant_action_alarm: Option<i64>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64, &Option<i64>)>
    for Consensus2pcUpdateParticipantContextActionModel
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
        match ParticipantContext::try_from(context.role_context().clone()) {
            Ok(participant_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let last_commit_epoch = context
                    .last_commit_epoch()
                    .map(i64::try_from)
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let (vote, decision_timeout_start) = match participant_context.state {
                    ParticipantState::Voted {
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
                            true => String::from("TRUE"),
                            false => String::from("FALSE"),
                        };
                        (Some(vote), Some(time))
                    }
                    _ => (None, None),
                };
                let alarm = context
                    .alarm()
                    .map(|a| {
                        a.duration_since(SystemTime::UNIX_EPOCH)
                            .map(|r| i64::try_from(r.as_secs()))
                    })
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?
                    .transpose()
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                Ok(Consensus2pcUpdateParticipantContextActionModel {
                    action_id: *action_id,
                    service_id: format!("{}", service_id),
                    alarm,
                    coordinator: format!("{}", context.coordinator()),
                    epoch,
                    last_commit_epoch,
                    state: String::from(&participant_context.state),
                    vote,
                    decision_timeout_start,
                    participant_action_alarm: *action_alarm,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_update_participant_context_action_participant"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[belongs_to(
    Consensus2pcUpdateParticipantContextActionModel,
    foreign_key = "action_id"
)]
#[primary_key(action_id, process)]
pub struct Consensus2pcUpdateParticipantContextActionParticipantModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
}

#[derive(Debug, PartialEq)]
pub struct UpdateParticipantContextActionParticipantList {
    pub inner: Vec<Consensus2pcUpdateParticipantContextActionParticipantModel>,
}

impl TryFrom<(&Context, &FullyQualifiedServiceId, &i64)>
    for UpdateParticipantContextActionParticipantList
{
    type Error = InternalError;

    fn try_from(
        (context, service_id, action_id): (&Context, &FullyQualifiedServiceId, &i64),
    ) -> Result<Self, Self::Error> {
        match ParticipantContext::try_from(context.role_context().clone()) {
            Ok(participant_context) => {
                let epoch = i64::try_from(*context.epoch())
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                let mut participant_participants = Vec::new();
                for participant in participant_context.participant_processes {
                    participant_participants.push(
                        Consensus2pcUpdateParticipantContextActionParticipantModel {
                            action_id: *action_id,
                            service_id: format!("{}", service_id),
                            epoch,
                            process: format!("{}", participant),
                        },
                    )
                }
                Ok(UpdateParticipantContextActionParticipantList {
                    inner: participant_participants,
                })
            }
            Err(e) => Err(InternalError::from_source(Box::new(e))),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_participant_send_message_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcParticipantSendMessageActionModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: String,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_participant_notification_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcParticipantNotificationModel {
    pub action_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub notification_type: String,
    pub dropped_message: Option<String>,
    pub request_for_vote_value: Option<Vec<u8>>,
}

impl From<&ParticipantState> for String {
    fn from(state: &ParticipantState) -> Self {
        match *state {
            ParticipantState::WaitingForVoteRequest => String::from("WAITINGFORVOTEREQUEST"),
            ParticipantState::Voted { .. } => String::from("VOTED"),
            ParticipantState::WaitingForVote => String::from("WAITINGFORVOTE"),
            ParticipantState::Abort => String::from("ABORT"),
            ParticipantState::Commit => String::from("COMMIT"),
        }
    }
}

impl From<&ConsensusActionNotification> for String {
    fn from(notification: &ConsensusActionNotification) -> Self {
        match *notification {
            ConsensusActionNotification::RequestForStart() => String::from("REQUESTFORSTART"),
            ConsensusActionNotification::CoordinatorRequestForVote() => {
                String::from("COORDINATORREQUESTFORVOTE")
            }
            ConsensusActionNotification::ParticipantRequestForVote(_) => {
                String::from("PARTICIPANTREQUESTFORVOTE")
            }
            ConsensusActionNotification::Commit() => String::from("COMMIT"),
            ConsensusActionNotification::Abort() => String::from("ABORT"),
            ConsensusActionNotification::MessageDropped(_) => String::from("MESSAGEDROPPED"),
        }
    }
}

impl From<&Scabbard2pcMessage> for String {
    fn from(message: &Scabbard2pcMessage) -> Self {
        match *message {
            Scabbard2pcMessage::VoteRequest(..) => String::from("VOTEREQUEST"),
            Scabbard2pcMessage::DecisionRequest(_) => String::from("DECISIONREQUEST"),
            Scabbard2pcMessage::VoteResponse(..) => String::from("VOTERESPONSE"),
            Scabbard2pcMessage::Commit(_) => String::from("COMMIT"),
            Scabbard2pcMessage::Abort(_) => String::from("ABORT"),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "two_pc_consensus_event"]
#[primary_key(id)]
pub struct TwoPcConsensusEventModel {
    pub id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
    pub position: i32,
    pub event_type: String,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "two_pc_consensus_event"]
pub struct InsertableTwoPcConsensusEventModel {
    pub service_id: String,
    pub epoch: i64,
    pub executed_at: Option<i64>,
    pub position: i32,
    pub event_type: String,
}

impl From<&Scabbard2pcEvent> for String {
    fn from(event: &Scabbard2pcEvent) -> Self {
        match *event {
            Scabbard2pcEvent::Alarm() => "ALARM".into(),
            Scabbard2pcEvent::Deliver(..) => "DELIVER".into(),
            Scabbard2pcEvent::Start(..) => "START".into(),
            Scabbard2pcEvent::Vote(..) => "VOTE".into(),
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "two_pc_consensus_deliver_event"]
#[belongs_to(TwoPcConsensusEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct TwoPcConsensusDeliverEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: String,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "two_pc_consensus_start_event"]
#[belongs_to(TwoPcConsensusEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct TwoPcConsensusStartEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub value: Vec<u8>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "two_pc_consensus_vote_event"]
#[belongs_to(TwoPcConsensusEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct TwoPcConsensusVoteEventModel {
    pub event_id: i64,
    pub service_id: String,
    pub epoch: i64,
    pub vote: String, // TRUE or FALSE
}
