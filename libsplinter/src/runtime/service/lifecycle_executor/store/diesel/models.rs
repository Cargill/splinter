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

use crate::error::InternalError;
use crate::runtime::service::{
    LifecycleCommand, LifecycleService, LifecycleStatus, LifecycleStoreError,
};

use super::schema::{service_lifecycle_argument, service_lifecycle_status};

/// Database model representation of `LifecycleService`
#[derive(Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable)]
#[table_name = "service_lifecycle_status"]
#[primary_key(circuit_id, service_id)]
pub struct ServiceLifecycleStatusModel {
    pub circuit_id: String,
    pub service_id: String,
    pub service_type: String,
    pub command: CommandTypeModel,
    pub status: StatusTypeModel,
}

impl From<&LifecycleService> for ServiceLifecycleStatusModel {
    fn from(service: &LifecycleService) -> Self {
        ServiceLifecycleStatusModel {
            circuit_id: service.service_id().circuit_id().as_str().into(),
            service_id: service.service_id().service_id().as_str().into(),
            service_type: service.service_type().to_string(),
            command: service.command().into(),
            status: service.status().into(),
        }
    }
}

/// Database model representation of the arguments in a `LifecycleService`
#[derive(
    Debug, PartialEq, Eq, Associations, Identifiable, Insertable, Queryable, QueryableByName,
)]
#[table_name = "service_lifecycle_argument"]
#[primary_key(circuit_id, service_id, key)]
pub struct ServiceLifecycleArgumentModel {
    pub circuit_id: String,
    pub service_id: String,
    pub key: String,
    pub value: String,
    pub position: i32,
}

impl TryFrom<&LifecycleService> for Vec<ServiceLifecycleArgumentModel> {
    type Error = LifecycleStoreError;

    fn try_from(service: &LifecycleService) -> Result<Self, Self::Error> {
        let mut service_arguments = Vec::new();
        service_arguments.extend(
            service
                .arguments()
                .iter()
                .enumerate()
                .map(|(idx, (key, value))| {
                    Ok(ServiceLifecycleArgumentModel {
                        circuit_id: service.service_id().circuit_id().as_str().into(),
                        service_id: service.service_id().service_id().as_str().into(),
                        key: key.clone(),
                        value: value.clone(),
                        position: i32::try_from(idx).map_err(|_| {
                            LifecycleStoreError::Internal(InternalError::with_message(
                                "Unable to convert index into i32".to_string(),
                            ))
                        })?,
                    })
                })
                .collect::<Result<Vec<ServiceLifecycleArgumentModel>, LifecycleStoreError>>()?,
        );
        Ok(service_arguments)
    }
}

impl From<&LifecycleCommand> for String {
    fn from(command: &LifecycleCommand) -> Self {
        match *command {
            LifecycleCommand::Prepare => "PREPARE".into(),
            LifecycleCommand::Finalize => "FINALIZE".into(),
            LifecycleCommand::Retire => "RETIRE".into(),
            LifecycleCommand::Purge => "PURGE".into(),
        }
    }
}

impl TryFrom<&str> for LifecycleCommand {
    type Error = LifecycleStoreError;

    fn try_from(command: &str) -> Result<Self, Self::Error> {
        match command {
            "PREPARE" => Ok(LifecycleCommand::Prepare),
            "FINALIZE" => Ok(LifecycleCommand::Finalize),
            "RETIRE" => Ok(LifecycleCommand::Retire),
            "PURGE" => Ok(LifecycleCommand::Purge),
            _ => Err(LifecycleStoreError::Internal(InternalError::with_message(
                format!("Unknown command {}", command),
            ))),
        }
    }
}

impl From<&LifecycleCommand> for CommandTypeModel {
    fn from(command: &LifecycleCommand) -> Self {
        match *command {
            LifecycleCommand::Prepare => CommandTypeModel::Prepare,
            LifecycleCommand::Finalize => CommandTypeModel::Finalize,
            LifecycleCommand::Retire => CommandTypeModel::Retire,
            LifecycleCommand::Purge => CommandTypeModel::Purge,
        }
    }
}

impl From<CommandTypeModel> for LifecycleCommand {
    fn from(command: CommandTypeModel) -> Self {
        match command {
            CommandTypeModel::Prepare => LifecycleCommand::Prepare,
            CommandTypeModel::Finalize => LifecycleCommand::Finalize,
            CommandTypeModel::Retire => LifecycleCommand::Retire,
            CommandTypeModel::Purge => LifecycleCommand::Purge,
        }
    }
}

impl TryFrom<&str> for LifecycleStatus {
    type Error = LifecycleStoreError;

    fn try_from(status: &str) -> Result<Self, Self::Error> {
        match status {
            "NEW" => Ok(LifecycleStatus::New),
            "COMPLETE" => Ok(LifecycleStatus::Complete),
            _ => Err(LifecycleStoreError::Internal(InternalError::with_message(
                format!("Unknown status {}", status),
            ))),
        }
    }
}

impl From<&LifecycleStatus> for String {
    fn from(status: &LifecycleStatus) -> Self {
        match *status {
            LifecycleStatus::New => "NEW".into(),
            LifecycleStatus::Complete => "COMPLETE".into(),
        }
    }
}

impl From<StatusTypeModel> for LifecycleStatus {
    fn from(status: StatusTypeModel) -> Self {
        match status {
            StatusTypeModel::New => LifecycleStatus::New,
            StatusTypeModel::Complete => LifecycleStatus::Complete,
        }
    }
}

impl From<&LifecycleStatus> for StatusTypeModel {
    fn from(status: &LifecycleStatus) -> Self {
        match *status {
            LifecycleStatus::New => StatusTypeModel::New,
            LifecycleStatus::Complete => StatusTypeModel::Complete,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StatusTypeModel {
    New,
    Complete,
}

pub struct StatusTypeModelMapping;

impl QueryId for StatusTypeModelMapping {
    type QueryId = StatusTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for StatusTypeModelMapping {}

impl SingleValue for StatusTypeModelMapping {}

impl AsExpression<StatusTypeModelMapping> for StatusTypeModel {
    type Expression = Bound<StatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<StatusTypeModelMapping>> for StatusTypeModel {
    type Expression = Bound<Nullable<StatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<StatusTypeModelMapping> for &'a StatusTypeModel {
    type Expression = Bound<StatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<StatusTypeModelMapping>> for &'a StatusTypeModel {
    type Expression = Bound<Nullable<StatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<StatusTypeModelMapping> for &'a &'b StatusTypeModel {
    type Expression = Bound<StatusTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<StatusTypeModelMapping>> for &'a &'b StatusTypeModel {
    type Expression = Bound<Nullable<StatusTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<StatusTypeModelMapping, DB> for StatusTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            StatusTypeModel::New => out.write_all(b"NEW")?,
            StatusTypeModel::Complete => out.write_all(b"COMPLETE")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<StatusTypeModelMapping>, DB> for StatusTypeModel
where
    DB: Backend,
    Self: ToSql<StatusTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<StatusTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<StatusTypeModelMapping, DB> for StatusTypeModel
where
    DB: Backend + HasSqlType<StatusTypeModelMapping>,
    StatusTypeModel: FromSql<StatusTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<StatusTypeModelMapping, DB> for StatusTypeModel
where
    DB: Backend,
    StatusTypeModel: FromSql<StatusTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<StatusTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<StatusTypeModelMapping, Pg> for StatusTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"NEW") => Ok(StatusTypeModel::New),
            Some(b"COMPLETE") => Ok(StatusTypeModel::Complete),
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
impl HasSqlType<StatusTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("status_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<StatusTypeModelMapping, Sqlite> for StatusTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"NEW") => Ok(StatusTypeModel::New),
            Some(b"COMPLETE") => Ok(StatusTypeModel::Complete),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<StatusTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CommandTypeModel {
    Prepare,
    Finalize,
    Retire,
    Purge,
}

pub struct CommandTypeModelMapping;

impl QueryId for CommandTypeModelMapping {
    type QueryId = CommandTypeModelMapping;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl NotNull for CommandTypeModelMapping {}

impl SingleValue for CommandTypeModelMapping {}

impl AsExpression<CommandTypeModelMapping> for CommandTypeModel {
    type Expression = Bound<CommandTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl AsExpression<Nullable<CommandTypeModelMapping>> for CommandTypeModel {
    type Expression = Bound<Nullable<CommandTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<CommandTypeModelMapping> for &'a CommandTypeModel {
    type Expression = Bound<CommandTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a> AsExpression<Nullable<CommandTypeModelMapping>> for &'a CommandTypeModel {
    type Expression = Bound<Nullable<CommandTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<CommandTypeModelMapping> for &'a &'b CommandTypeModel {
    type Expression = Bound<CommandTypeModelMapping, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<'a, 'b> AsExpression<Nullable<CommandTypeModelMapping>> for &'a &'b CommandTypeModel {
    type Expression = Bound<Nullable<CommandTypeModelMapping>, Self>;

    fn as_expression(self) -> Self::Expression {
        Bound::new(self)
    }
}

impl<DB: Backend> ToSql<CommandTypeModelMapping, DB> for CommandTypeModel {
    fn to_sql<W: Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        match self {
            CommandTypeModel::Prepare => out.write_all(b"PREPARE")?,
            CommandTypeModel::Finalize => out.write_all(b"FINALIZE")?,
            CommandTypeModel::Retire => out.write_all(b"RETIRE")?,
            CommandTypeModel::Purge => out.write_all(b"PURGE")?,
        }
        Ok(IsNull::No)
    }
}

impl<DB> ToSql<Nullable<CommandTypeModelMapping>, DB> for CommandTypeModel
where
    DB: Backend,
    Self: ToSql<CommandTypeModelMapping, DB>,
{
    fn to_sql<W: ::std::io::Write>(&self, out: &mut Output<W, DB>) -> serialize::Result {
        ToSql::<CommandTypeModelMapping, DB>::to_sql(self, out)
    }
}

impl<DB> Queryable<CommandTypeModelMapping, DB> for CommandTypeModel
where
    DB: Backend + HasSqlType<CommandTypeModelMapping>,
    CommandTypeModel: FromSql<CommandTypeModelMapping, DB>,
{
    type Row = Self;

    fn build(row: Self::Row) -> Self {
        row
    }
}

impl<DB> FromSqlRow<CommandTypeModelMapping, DB> for CommandTypeModel
where
    DB: Backend,
    CommandTypeModel: FromSql<CommandTypeModelMapping, DB>,
{
    fn build_from_row<T: Row<DB>>(row: &mut T) -> deserialize::Result<Self> {
        FromSql::<CommandTypeModelMapping, DB>::from_sql(row.take())
    }
}

#[cfg(feature = "postgres")]
impl FromSql<CommandTypeModelMapping, Pg> for CommandTypeModel {
    fn from_sql(bytes: Option<&<Pg as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes {
            Some(b"PREPARE") => Ok(CommandTypeModel::Prepare),
            Some(b"FINALIZE") => Ok(CommandTypeModel::Finalize),
            Some(b"RETIRE") => Ok(CommandTypeModel::Retire),
            Some(b"PURGE") => Ok(CommandTypeModel::Purge),
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
impl HasSqlType<CommandTypeModelMapping> for Pg {
    fn metadata(lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        lookup.lookup_type("command_type")
    }
}

#[cfg(feature = "sqlite")]
impl FromSql<CommandTypeModelMapping, Sqlite> for CommandTypeModel {
    fn from_sql(bytes: Option<&<Sqlite as Backend>::RawValue>) -> deserialize::Result<Self> {
        match bytes.map(|v| v.read_blob()) {
            Some(b"PREPARE") => Ok(CommandTypeModel::Prepare),
            Some(b"FINALIZE") => Ok(CommandTypeModel::Finalize),
            Some(b"RETIRE") => Ok(CommandTypeModel::Retire),
            Some(b"PURGE") => Ok(CommandTypeModel::Purge),
            Some(blob) => {
                Err(format!("Unexpected variant: {}", String::from_utf8_lossy(blob)).into())
            }
            None => Err("Unexpected null for non-null column".into()),
        }
    }
}

#[cfg(feature = "sqlite")]
impl HasSqlType<CommandTypeModelMapping> for Sqlite {
    fn metadata(_lookup: &Self::MetadataLookup) -> Self::TypeMetadata {
        diesel::sqlite::SqliteType::Text
    }
}
