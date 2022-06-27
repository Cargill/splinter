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
use std::io::Write;
use std::time::{Duration, SystemTime};

use diesel::{
    backend::Backend,
    deserialize::FromSqlRow,
    expression::{bound::Bound, AsExpression},
    query_builder::QueryId,
    serialize::{self, IsNull, Output, ToSql},
    sql_types::{HasSqlType, NotNull, Nullable, SingleValue},
    Queryable,
};
#[cfg(feature = "diesel")]
use diesel::{
    deserialize::{self, FromSql},
    row::Row,
};

#[cfg(feature = "postgres")]
use diesel::pg::Pg;
#[cfg(feature = "sqlite")]
use diesel::sqlite::Sqlite;

use splinter::error::InternalError;
use splinter::service::{FullyQualifiedServiceId, ServiceId};

use crate::store::scabbard_store::{
    two_phase_commit::{Context, ContextBuilder, Event, Message, Notification, Participant, State},
    ConsensusContext,
};

use crate::store::scabbard_store::diesel::schema::{
    consensus_2pc_action, consensus_2pc_context, consensus_2pc_context_participant,
    consensus_2pc_deliver_event, consensus_2pc_event, consensus_2pc_notification_action,
    consensus_2pc_send_message_action, consensus_2pc_start_event,
    consensus_2pc_update_context_action, consensus_2pc_update_context_action_participant,
    consensus_2pc_vote_event,
};

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context"]
#[primary_key(circuit_id, service_id)]
pub struct Consensus2pcContextModel {
    pub circuit_id: String,
    pub service_id: String,
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: ContextStateModel,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub ack_timeout_start: Option<i64>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContextStateModel {
    Abort,
    Commit,
    Voted,
    Voting,
    WaitingForStart,
    WaitingForVoteRequest,
    WaitingForVote,
    WaitingForDecisionAck,
}

pub struct ContextStateModelMapping;

impl QueryId for ContextStateModelMapping {
    type QueryId = ContextStateModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for ContextStateModelMapping {}

impl SingleValue for ContextStateModelMapping {}

impl AsExpression<ContextStateModelMapping> for ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<ContextStateModelMapping>> for ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<ContextStateModelMapping> for &'a ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<ContextStateModelMapping>> for &'a ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<ContextStateModelMapping> for &'a &'b ContextStateModel {
    type Expression = Bound<ContextStateModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<ContextStateModelMapping>> for &'a &'b ContextStateModel {
    type Expression = Bound<Nullable<ContextStateModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<ContextStateModelMapping, DB> for ContextStateModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            ContextStateModel::Abort => out.write_all(b"ABORT")?,
            ContextStateModel::Commit => out.write_all(b"COMMIT")?,
            ContextStateModel::Voted => out.write_all(b"VOTED")?,
            ContextStateModel::Voting => out.write_all(b"VOTING")?,
            ContextStateModel::WaitingForStart => out.write_all(b"WAITINGFORSTART")?,
            ContextStateModel::WaitingForVoteRequest => out.write_all(b"WAITINGFORVOTEREQUEST")?,
            ContextStateModel::WaitingForVote => out.write_all(b"WAITINGFORVOTE")?,
            ContextStateModel::WaitingForDecisionAck => {
                out.write_all(b"WAITING_FOR_DECISION_ACK")?
            }
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<ContextStateModelMapping>, DB> for ContextStateModel
where
    DB: Backend,
    Self: ToSql<ContextStateModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<ContextStateModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<ContextStateModelMapping, DB> for ContextStateModel
where
    DB: Backend + HasSqlType<ContextStateModelMapping>,
    ContextStateModel: FromSql<ContextStateModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<ContextStateModelMapping, DB> for ContextStateModel
where
    DB: Backend,
    ContextStateModel: FromSql<ContextStateModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<ContextStateModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<ContextStateModelMapping, Pg> for ContextStateModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"ABORT") => Ok(ContextStateModel::Abort),
            Some(b"COMMIT") => Ok(ContextStateModel::Commit),
            Some(b"VOTED") => Ok(ContextStateModel::Voted),
            Some(b"VOTING") => Ok(ContextStateModel::Voting),
            Some(b"WAITINGFORSTART") => Ok(ContextStateModel::WaitingForStart),
            Some(b"WAITINGFORVOTEREQUEST") => Ok(ContextStateModel::WaitingForVoteRequest),
            Some(b"WAITINGFORVOTE") => Ok(ContextStateModel::WaitingForVote),
            Some(b"WAITING_FOR_DECISION_ACK") => Ok(ContextStateModel::WaitingForDecisionAck),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<ContextStateModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("context_state")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<ContextStateModelMapping, Sqlite> for ContextStateModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"ABORT") => Ok(ContextStateModel::Abort),
            Some(b"COMMIT") => Ok(ContextStateModel::Commit),
            Some(b"VOTED") => Ok(ContextStateModel::Voted),
            Some(b"VOTING") => Ok(ContextStateModel::Voting),
            Some(b"WAITINGFORSTART") => Ok(ContextStateModel::WaitingForStart),
            Some(b"WAITINGFORVOTEREQUEST") => Ok(ContextStateModel::WaitingForVoteRequest),
            Some(b"WAITINGFORVOTE") => Ok(ContextStateModel::WaitingForVote),
            Some(b"WAITING_FOR_DECISION_ACK") => Ok(ContextStateModel::WaitingForDecisionAck),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<ContextStateModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
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

        let state = match context.state {
            ContextStateModel::WaitingForStart => State::WaitingForStart,
            ContextStateModel::Voting => {
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
            ContextStateModel::WaitingForVote => State::WaitingForVote,
            ContextStateModel::Abort => State::Abort,
            ContextStateModel::Commit => State::Commit,
            ContextStateModel::WaitingForVoteRequest => State::WaitingForVoteRequest,
            ContextStateModel::Voted => {
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
            ContextStateModel::WaitingForDecisionAck => {
                let ack_timeout_start = if let Some(t) = context.ack_timeout_start {
                    SystemTime::UNIX_EPOCH
                        .checked_add(Duration::from_secs(t as u64))
                        .ok_or_else(|| {
                            InternalError::with_message(
                                "Failed to convert ack timeout start timestamp to SystemTime"
                                    .to_string(),
                            )
                        })?
                } else {
                    return Err(InternalError::with_message(
                        "Failed to convert to ConsensusContext, context has state \
                        'WaitingForDecisionAck' but 'decision_timeout_start' is unset"
                            .to_string(),
                    ));
                };
                State::WaitingForDecisionAck { ack_timeout_start }
            }
        };

        let mut builder = ContextBuilder::default()
            .with_coordinator(&coordinator)
            .with_epoch(epoch)
            .with_state(state)
            .with_participants(participants)
            .with_this_process(
                FullyQualifiedServiceId::new_from_string(format!(
                    "{}::{}",
                    &context.circuit_id, &context.service_id
                ))
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
        let (vote_timeout_start, vote, decision_timeout_start, ack_timeout_start) =
            match context.state() {
                State::Voting { vote_timeout_start } => {
                    let time = i64::try_from(
                        vote_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (Some(time), None, None, None)
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
                    (None, Some(vote.to_string()), Some(time), None)
                }
                State::WaitingForDecisionAck { ack_timeout_start } => {
                    let time = i64::try_from(
                        ack_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (None, None, None, Some(time))
                }
                _ => (None, None, None, None),
            };
        let state = ContextStateModel::from(context.state());
        Ok(Consensus2pcContextModel {
            circuit_id: service_id.circuit_id().to_string(),
            service_id: service_id.service_id().to_string(),
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
            ack_timeout_start,
        })
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_context_participant"]
#[primary_key(circuit_id, service_id, process)]
pub struct Consensus2pcContextParticipantModel {
    pub circuit_id: String,
    pub service_id: String,
    pub epoch: i64,
    pub process: String,
    pub vote: Option<String>,
    pub decision_ack: bool,
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
                decision_ack: p.decision_ack,
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
                circuit_id: service_id.circuit_id().to_string(),
                service_id: service_id.service_id().to_string(),
                epoch,
                process: format!("{}", participant.process),
                vote,
                decision_ack: participant.decision_ack,
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
    pub coordinator: String,
    pub epoch: i64,
    pub last_commit_epoch: Option<i64>,
    pub state: ContextStateModel,
    pub vote_timeout_start: Option<i64>,
    pub vote: Option<String>,
    pub decision_timeout_start: Option<i64>,
    pub action_alarm: Option<i64>,
    pub ack_timeout_start: Option<i64>,
}

impl TryFrom<(&Context, &i64, &Option<i64>)> for Consensus2pcUpdateContextActionModel {
    type Error = InternalError;

    fn try_from(
        (context, action_id, action_alarm): (&Context, &i64, &Option<i64>),
    ) -> Result<Self, Self::Error> {
        let epoch = i64::try_from(*context.epoch())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let last_commit_epoch = context
            .last_commit_epoch()
            .map(i64::try_from)
            .transpose()
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        let (vote_timeout_start, vote, decision_timeout_start, ack_timeout_start) =
            match context.state() {
                State::Voting { vote_timeout_start } => {
                    let time = i64::try_from(
                        vote_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (Some(time), None, None, None)
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
                    (None, Some(vote.to_string()), Some(time), None)
                }
                State::WaitingForDecisionAck { ack_timeout_start } => {
                    let time = i64::try_from(
                        ack_timeout_start
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .map_err(|err| InternalError::from_source(Box::new(err)))?
                            .as_secs(),
                    )
                    .map_err(|err| InternalError::from_source(Box::new(err)))?;
                    (None, None, None, Some(time))
                }
                _ => (None, None, None, None),
            };
        let state = ContextStateModel::from(context.state());
        Ok(Consensus2pcUpdateContextActionModel {
            action_id: *action_id,
            coordinator: format!("{}", context.coordinator()),
            epoch,
            last_commit_epoch,
            state,
            vote_timeout_start,
            vote,
            decision_timeout_start,
            action_alarm: *action_alarm,
            ack_timeout_start,
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
    pub process: String,
    pub vote: Option<String>,
    pub decision_ack: bool,
}

#[derive(Debug, PartialEq)]
pub struct UpdateContextActionParticipantList {
    pub inner: Vec<Consensus2pcUpdateContextActionParticipantModel>,
}

impl TryFrom<(&Context, &i64)> for UpdateContextActionParticipantList {
    type Error = InternalError;

    fn try_from((context, action_id): (&Context, &i64)) -> Result<Self, Self::Error> {
        let mut participants = Vec::new();
        for participant in context.participants() {
            let vote = participant.vote.map(|vote| match vote {
                true => "TRUE".to_string(),
                false => "FALSE".to_string(),
            });
            participants.push(Consensus2pcUpdateContextActionParticipantModel {
                action_id: *action_id,
                process: format!("{}", participant.process),
                vote,
                decision_ack: participant.decision_ack,
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
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: MessageTypeModel,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum MessageTypeModel {
    VoteResponse,
    DecisionRequest,
    VoteRequest,
    Commit,
    Abort,
    DecisionAck,
}

pub struct MessageTypeModelMapping;

impl QueryId for MessageTypeModelMapping {
    type QueryId = MessageTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for MessageTypeModelMapping {}

impl SingleValue for MessageTypeModelMapping {}

impl AsExpression<MessageTypeModelMapping> for MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<MessageTypeModelMapping>> for MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<MessageTypeModelMapping> for &'a MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<MessageTypeModelMapping>> for &'a MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<MessageTypeModelMapping> for &'a &'b MessageTypeModel {
    type Expression = Bound<MessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<MessageTypeModelMapping>> for &'a &'b MessageTypeModel {
    type Expression = Bound<Nullable<MessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<MessageTypeModelMapping, DB> for MessageTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            MessageTypeModel::VoteResponse => out.write_all(b"VOTERESPONSE")?,
            MessageTypeModel::DecisionRequest => out.write_all(b"DECISIONREQUEST")?,
            MessageTypeModel::VoteRequest => out.write_all(b"VOTEREQUEST")?,
            MessageTypeModel::Commit => out.write_all(b"COMMIT")?,
            MessageTypeModel::Abort => out.write_all(b"ABORT")?,
            MessageTypeModel::DecisionAck => out.write_all(b"DECISION_ACK")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<MessageTypeModelMapping>, DB> for MessageTypeModel
where
    DB: Backend,
    Self: ToSql<MessageTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<MessageTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<MessageTypeModelMapping, DB> for MessageTypeModel
where
    DB: Backend + HasSqlType<MessageTypeModelMapping>,
    MessageTypeModel: FromSql<MessageTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<MessageTypeModelMapping, DB> for MessageTypeModel
where
    DB: Backend,
    MessageTypeModel: FromSql<MessageTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<MessageTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<MessageTypeModelMapping, Pg> for MessageTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"VOTERESPONSE") => Ok(MessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(MessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(MessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(MessageTypeModel::Commit),
            Some(b"ABORT") => Ok(MessageTypeModel::Abort),
            Some(b"DECISION_ACK") => Ok(MessageTypeModel::DecisionAck),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<MessageTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("message_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<MessageTypeModelMapping, Sqlite> for MessageTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"VOTERESPONSE") => Ok(MessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(MessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(MessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(MessageTypeModel::Commit),
            Some(b"ABORT") => Ok(MessageTypeModel::Abort),
            Some(b"DECISION_ACK") => Ok(MessageTypeModel::DecisionAck),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<MessageTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_notification_action"]
#[belongs_to(Consensus2pcActionModel, foreign_key = "action_id")]
#[primary_key(action_id)]
pub struct Consensus2pcNotificationModel {
    pub action_id: i64,
    pub notification_type: NotificationTypeModel,
    pub dropped_message: Option<String>,
    pub request_for_vote_value: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum NotificationTypeModel {
    RequestForStart,
    CoordinatorRequestForVote,
    ParticipantRequestForVote,
    Commit,
    Abort,
    MessageDropped,
}

pub struct NotificationTypeModelMapping;

impl QueryId for NotificationTypeModelMapping {
    type QueryId = NotificationTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for NotificationTypeModelMapping {}

impl SingleValue for NotificationTypeModelMapping {}

impl AsExpression<NotificationTypeModelMapping> for NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<NotificationTypeModelMapping>> for NotificationTypeModel {
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<NotificationTypeModelMapping> for &'a NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<NotificationTypeModelMapping>> for &'a NotificationTypeModel {
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<NotificationTypeModelMapping> for &'a &'b NotificationTypeModel {
    type Expression = Bound<NotificationTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<NotificationTypeModelMapping>>
    for &'a &'b NotificationTypeModel
{
    type Expression = Bound<Nullable<NotificationTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<NotificationTypeModelMapping, DB> for NotificationTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            NotificationTypeModel::RequestForStart => out.write_all(b"REQUESTFORSTART")?,
            NotificationTypeModel::CoordinatorRequestForVote => {
                out.write_all(b"COORDINATORREQUESTFORVOTE")?
            }
            NotificationTypeModel::ParticipantRequestForVote => {
                out.write_all(b"PARTICIPANTREQUESTFORVOTE")?
            }
            NotificationTypeModel::Commit => out.write_all(b"COMMIT")?,
            NotificationTypeModel::Abort => out.write_all(b"ABORT")?,
            NotificationTypeModel::MessageDropped => out.write_all(b"MESSAGEDROPPED")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<NotificationTypeModelMapping>, DB> for NotificationTypeModel
where
    DB: Backend,
    Self: ToSql<NotificationTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<NotificationTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<NotificationTypeModelMapping, DB> for NotificationTypeModel
where
    DB: Backend + HasSqlType<NotificationTypeModelMapping>,
    NotificationTypeModel: FromSql<NotificationTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<NotificationTypeModelMapping, DB> for NotificationTypeModel
where
    DB: Backend,
    NotificationTypeModel: FromSql<NotificationTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<NotificationTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<NotificationTypeModelMapping, Pg> for NotificationTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"REQUESTFORSTART") => Ok(NotificationTypeModel::RequestForStart),
            Some(b"COORDINATORREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANTREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(NotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(NotificationTypeModel::Abort),
            Some(b"MESSAGEDROPPED") => Ok(NotificationTypeModel::MessageDropped),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<NotificationTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("notification_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<NotificationTypeModelMapping, Sqlite> for NotificationTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"REQUESTFORSTART") => Ok(NotificationTypeModel::RequestForStart),
            Some(b"COORDINATORREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::CoordinatorRequestForVote)
            }
            Some(b"PARTICIPANTREQUESTFORVOTE") => {
                Ok(NotificationTypeModel::ParticipantRequestForVote)
            }
            Some(b"COMMIT") => Ok(NotificationTypeModel::Commit),
            Some(b"ABORT") => Ok(NotificationTypeModel::Abort),
            Some(b"MESSAGEDROPPED") => Ok(NotificationTypeModel::MessageDropped),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<NotificationTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
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
            State::WaitingForDecisionAck { .. } => String::from("WAITING_FOR_DECISION_ACK"),
        }
    }
}

impl From<&State> for ContextStateModel {
    fn from(state: &State) -> Self {
        match *state {
            State::WaitingForStart => ContextStateModel::WaitingForStart,
            State::Voting { .. } => ContextStateModel::Voting,
            State::WaitingForVote => ContextStateModel::WaitingForVote,
            State::Abort => ContextStateModel::Abort,
            State::Commit => ContextStateModel::Commit,
            State::WaitingForVoteRequest => ContextStateModel::WaitingForVoteRequest,
            State::Voted { .. } => ContextStateModel::Voted,
            State::WaitingForDecisionAck { .. } => ContextStateModel::WaitingForDecisionAck,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_action"]
#[belongs_to(Consensus2pcContextModel, foreign_key = "service_id")]
#[primary_key(id)]
pub struct Consensus2pcActionModel {
    pub id: i64,
    pub circuit_id: String,
    pub service_id: String,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_action"]
pub struct InsertableConsensus2pcActionModel {
    pub circuit_id: String,
    pub service_id: String,
    pub executed_at: Option<i64>,
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

impl From<&Notification> for NotificationTypeModel {
    fn from(notification: &Notification) -> Self {
        match *notification {
            Notification::RequestForStart() => NotificationTypeModel::RequestForStart,
            Notification::CoordinatorRequestForVote() => {
                NotificationTypeModel::CoordinatorRequestForVote
            }
            Notification::ParticipantRequestForVote(_) => {
                NotificationTypeModel::ParticipantRequestForVote
            }
            Notification::Commit() => NotificationTypeModel::Commit,
            Notification::Abort() => NotificationTypeModel::Abort,
            Notification::MessageDropped(_) => NotificationTypeModel::MessageDropped,
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
            Message::DecisionAck(_) => String::from("DECISION_ACK"),
        }
    }
}

impl From<&Message> for MessageTypeModel {
    fn from(message: &Message) -> Self {
        match *message {
            Message::VoteRequest(..) => MessageTypeModel::VoteRequest,
            Message::DecisionRequest(_) => MessageTypeModel::DecisionRequest,
            Message::VoteResponse(..) => MessageTypeModel::VoteResponse,
            Message::Commit(_) => MessageTypeModel::Commit,
            Message::Abort(_) => MessageTypeModel::Abort,
            Message::DecisionAck(_) => MessageTypeModel::DecisionAck,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_event"]
#[primary_key(id)]
pub struct Consensus2pcEventModel {
    pub id: i64,
    pub circuit_id: String,
    pub service_id: String,
    pub created_at: SystemTime,
    pub executed_at: Option<i64>,
    pub event_type: EventTypeModel,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "consensus_2pc_event"]
pub struct InsertableConsensus2pcEventModel {
    pub circuit_id: String,
    pub service_id: String,
    pub executed_at: Option<i64>,
    pub event_type: EventTypeModel,
}

impl From<&Event> for EventTypeModel {
    fn from(event: &Event) -> Self {
        match *event {
            Event::Alarm() => EventTypeModel::Alarm,
            Event::Deliver(..) => EventTypeModel::Deliver,
            Event::Start(..) => EventTypeModel::Start,
            Event::Vote(..) => EventTypeModel::Vote,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EventTypeModel {
    Alarm,
    Deliver,
    Start,
    Vote,
}

// This has to be pub, due to its use in the table macro execution for IdentityModel
pub struct EventTypeModelMapping;

impl QueryId for EventTypeModelMapping {
    type QueryId = EventTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for EventTypeModelMapping {}

impl SingleValue for EventTypeModelMapping {}

impl AsExpression<EventTypeModelMapping> for EventTypeModel {
    type Expression = Bound<EventTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<EventTypeModelMapping>> for EventTypeModel {
    type Expression = Bound<Nullable<EventTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<EventTypeModelMapping> for &'a EventTypeModel {
    type Expression = Bound<EventTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<EventTypeModelMapping>> for &'a EventTypeModel {
    type Expression = Bound<Nullable<EventTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<EventTypeModelMapping> for &'a &'b EventTypeModel {
    type Expression = Bound<EventTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<EventTypeModelMapping>> for &'a &'b EventTypeModel {
    type Expression = Bound<Nullable<EventTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<EventTypeModelMapping, DB> for EventTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            EventTypeModel::Alarm => out.write_all(b"ALARM")?,
            EventTypeModel::Deliver => out.write_all(b"DELIVER")?,
            EventTypeModel::Start => out.write_all(b"START")?,
            EventTypeModel::Vote => out.write_all(b"VOTE")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<EventTypeModelMapping>, DB> for EventTypeModel
where
    DB: Backend,
    Self: ToSql<EventTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<EventTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<EventTypeModelMapping, DB> for EventTypeModel
where
    DB: Backend + HasSqlType<EventTypeModelMapping>,
    EventTypeModel: FromSql<EventTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<EventTypeModelMapping, DB> for EventTypeModel
where
    DB: Backend,
    EventTypeModel: FromSql<EventTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<EventTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<EventTypeModelMapping, Pg> for EventTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"ALARM") => Ok(EventTypeModel::Alarm),
            Some(b"DELIVER") => Ok(EventTypeModel::Deliver),
            Some(b"START") => Ok(EventTypeModel::Start),
            Some(b"VOTE") => Ok(EventTypeModel::Vote),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<EventTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("event_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<EventTypeModelMapping, Sqlite> for EventTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"ALARM") => Ok(EventTypeModel::Alarm),
            Some(b"DELIVER") => Ok(EventTypeModel::Deliver),
            Some(b"START") => Ok(EventTypeModel::Start),
            Some(b"VOTE") => Ok(EventTypeModel::Vote),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<EventTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_deliver_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcDeliverEventModel {
    pub event_id: i64,
    pub epoch: i64,
    pub receiver_service_id: String,
    pub message_type: DeliverMessageTypeModel,
    pub vote_response: Option<String>,
    pub vote_request: Option<Vec<u8>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum DeliverMessageTypeModel {
    VoteResponse,
    DecisionRequest,
    VoteRequest,
    Commit,
    Abort,
    DecisionAck,
}

pub struct DeliverMessageTypeModelMapping;

impl QueryId for DeliverMessageTypeModelMapping {
    type QueryId = DeliverMessageTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for DeliverMessageTypeModelMapping {}

impl SingleValue for DeliverMessageTypeModelMapping {}

impl AsExpression<DeliverMessageTypeModelMapping> for DeliverMessageTypeModel {
    type Expression = Bound<DeliverMessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<DeliverMessageTypeModelMapping>> for DeliverMessageTypeModel {
    type Expression = Bound<Nullable<DeliverMessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<DeliverMessageTypeModelMapping> for &'a DeliverMessageTypeModel {
    type Expression = Bound<DeliverMessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<DeliverMessageTypeModelMapping>> for &'a DeliverMessageTypeModel {
    type Expression = Bound<Nullable<DeliverMessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<DeliverMessageTypeModelMapping> for &'a &'b DeliverMessageTypeModel {
    type Expression = Bound<DeliverMessageTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<DeliverMessageTypeModelMapping>>
    for &'a &'b DeliverMessageTypeModel
{
    type Expression = Bound<Nullable<DeliverMessageTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<DeliverMessageTypeModelMapping, DB> for DeliverMessageTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            DeliverMessageTypeModel::VoteResponse => out.write_all(b"VOTERESPONSE")?,
            DeliverMessageTypeModel::DecisionRequest => out.write_all(b"DECISIONREQUEST")?,
            DeliverMessageTypeModel::VoteRequest => out.write_all(b"VOTEREQUEST")?,
            DeliverMessageTypeModel::Commit => out.write_all(b"COMMIT")?,
            DeliverMessageTypeModel::Abort => out.write_all(b"ABORT")?,
            DeliverMessageTypeModel::DecisionAck => out.write_all(b"DECISION_ACK")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<DeliverMessageTypeModelMapping>, DB> for DeliverMessageTypeModel
where
    DB: Backend,
    Self: ToSql<DeliverMessageTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<DeliverMessageTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<DeliverMessageTypeModelMapping, DB> for DeliverMessageTypeModel
where
    DB: Backend + HasSqlType<DeliverMessageTypeModelMapping>,
    DeliverMessageTypeModel: FromSql<DeliverMessageTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<DeliverMessageTypeModelMapping, DB> for DeliverMessageTypeModel
where
    DB: Backend,
    DeliverMessageTypeModel: FromSql<DeliverMessageTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<DeliverMessageTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<DeliverMessageTypeModelMapping, Pg> for DeliverMessageTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"VOTERESPONSE") => Ok(DeliverMessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(DeliverMessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(DeliverMessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(DeliverMessageTypeModel::Commit),
            Some(b"ABORT") => Ok(DeliverMessageTypeModel::Abort),
            Some(b"DECISION_ACK") => Ok(DeliverMessageTypeModel::DecisionAck),
            Some(v) => Err(format!(
                "Unrecognized enum variant: '{}'",
                String::from_utf8_lossy(v)
            )
            .into()),
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "postgres")]
impl HasSqlType<DeliverMessageTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("deliver_event_message_typ")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<DeliverMessageTypeModelMapping, Sqlite> for DeliverMessageTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"VOTERESPONSE") => Ok(DeliverMessageTypeModel::VoteResponse),
            Some(b"DECISIONREQUEST") => Ok(DeliverMessageTypeModel::DecisionRequest),
            Some(b"VOTEREQUEST") => Ok(DeliverMessageTypeModel::VoteRequest),
            Some(b"COMMIT") => Ok(DeliverMessageTypeModel::Commit),
            Some(b"ABORT") => Ok(DeliverMessageTypeModel::Abort),
            Some(b"DECISION_ACK") => Ok(DeliverMessageTypeModel::DecisionAck),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<DeliverMessageTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

impl From<&Message> for DeliverMessageTypeModel {
    fn from(message: &Message) -> Self {
        match *message {
            Message::VoteRequest(..) => DeliverMessageTypeModel::VoteRequest,
            Message::DecisionRequest(_) => DeliverMessageTypeModel::DecisionRequest,
            Message::VoteResponse(..) => DeliverMessageTypeModel::VoteResponse,
            Message::Commit(_) => DeliverMessageTypeModel::Commit,
            Message::Abort(_) => DeliverMessageTypeModel::Abort,
            Message::DecisionAck(_) => DeliverMessageTypeModel::DecisionAck,
        }
    }
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_start_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcStartEventModel {
    pub event_id: i64,
    pub value: Vec<u8>,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Insertable, Queryable, QueryableByName)]
#[table_name = "consensus_2pc_vote_event"]
#[belongs_to(Consensus2pcEventModel, foreign_key = "event_id")]
#[primary_key(event_id)]
pub struct Consensus2pcVoteEventModel {
    pub event_id: i64,
    pub vote: String, // TRUE or FALSE
}
