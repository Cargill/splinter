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

#[cfg(feature = "postgres")]
use diesel::pg::PgConnection;
#[cfg(feature = "sqlite")]
use diesel::sqlite::SqliteConnection;
use diesel::{dsl::insert_into, prelude::*};
use splinter::error::{InternalError, InvalidStateError};

use crate::store::scabbard_store::commit::CommitEntry;
use crate::store::scabbard_store::diesel::{
    models::{CommitEntryModel, Consensus2pcContextModel, ScabbardServiceModel},
    schema::{consensus_2pc_context, scabbard_service, scabbard_v3_commit_history},
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait AddCommitEntryOperation {
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> AddCommitEntryOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given service_id exists
            scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", commit_entry.service_id())))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Service does not exist",
                    )))
                })?;

            // check to see if a context with the given service_id exists
            let context = consensus_2pc_context::table
                .filter(
                    consensus_2pc_context::service_id.eq(format!("{}", commit_entry.service_id())),
                )
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(format!(
                        "Cannot add commit entry, context with service ID {} does not exist",
                        commit_entry.service_id()
                    )))
                })?;

            let commit_entry = commit_entry
                .into_builder()
                .with_epoch(
                    u64::try_from(context.epoch)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?,
                )
                .build()
                .map_err(ScabbardStoreError::InvalidState)?;

            insert_into(scabbard_v3_commit_history::table)
                .values(vec![CommitEntryModel::try_from(&commit_entry)?])
                .execute(self.conn)?;

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> AddCommitEntryOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn add_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            // check to see if a service with the given epoch and service_id exists
            scabbard_service::table
                .filter(scabbard_service::service_id.eq(format!("{}", commit_entry.service_id())))
                .first::<ScabbardServiceModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Service does not exist",
                    )))
                })?;

            // check to see if a context with the given service_id exists
            let context = consensus_2pc_context::table
                .filter(
                    consensus_2pc_context::service_id.eq(format!("{}", commit_entry.service_id())),
                )
                .first::<Consensus2pcContextModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(format!(
                        "Cannot add commit entry, context with service ID {} does not exist",
                        commit_entry.service_id()
                    )))
                })?;

            let commit_entry = commit_entry
                .into_builder()
                .with_epoch(
                    u64::try_from(context.epoch)
                        .map_err(|err| InternalError::from_source(Box::new(err)))?,
                )
                .build()
                .map_err(ScabbardStoreError::InvalidState)?;

            insert_into(scabbard_v3_commit_history::table)
                .values(vec![CommitEntryModel::try_from(&commit_entry)?])
                .execute(self.conn)?;

            Ok(())
        })
    }
}
