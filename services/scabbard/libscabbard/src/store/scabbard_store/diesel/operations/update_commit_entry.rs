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

use diesel::{delete, dsl::insert_into, prelude::*};
use splinter::error::{InternalError, InvalidStateError};

use crate::store::scabbard_store::commit::CommitEntry;
use crate::store::scabbard_store::diesel::{
    models::CommitEntryModel, schema::scabbard_v3_commit_history,
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

pub(in crate::store::scabbard_store::diesel) trait UpdateCommitEntryOperation {
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> UpdateCommitEntryOperation for ScabbardStoreOperations<'a, SqliteConnection> {
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(commit_entry.epoch().ok_or_else(|| {
                ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                    "Commit entry does not have an epoch",
                )))
            })?)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
            // check to see if a commit entry with the given service_id and epoch exists
            scabbard_v3_commit_history::table
                .filter(
                    scabbard_v3_commit_history::service_id
                        .eq(format!("{}", commit_entry.service_id()))
                        .and(scabbard_v3_commit_history::epoch.eq(epoch)),
                )
                .first::<CommitEntryModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Commit entry does not exist",
                    )))
                })?;

            delete(
                scabbard_v3_commit_history::table
                    .find((format!("{}", commit_entry.service_id()), epoch)),
            )
            .execute(self.conn)?;

            insert_into(scabbard_v3_commit_history::table)
                .values(vec![CommitEntryModel::try_from(&commit_entry)?])
                .execute(self.conn)?;

            Ok(())
        })
    }
}

#[cfg(feature = "postgres")]
impl<'a> UpdateCommitEntryOperation for ScabbardStoreOperations<'a, PgConnection> {
    fn update_commit_entry(&self, commit_entry: CommitEntry) -> Result<(), ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            let epoch = i64::try_from(commit_entry.epoch().ok_or_else(|| {
                ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                    "Commit entry does not have an epoch",
                )))
            })?)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
            // check to see if a commit entry with the given service_id and epoch exists
            scabbard_v3_commit_history::table
                .filter(
                    scabbard_v3_commit_history::service_id
                        .eq(format!("{}", commit_entry.service_id()))
                        .and(scabbard_v3_commit_history::epoch.eq(epoch)),
                )
                .first::<CommitEntryModel>(self.conn)
                .optional()?
                .ok_or_else(|| {
                    ScabbardStoreError::InvalidState(InvalidStateError::with_message(String::from(
                        "Commit entry does not exist",
                    )))
                })?;

            delete(
                scabbard_v3_commit_history::table
                    .find((format!("{}", commit_entry.service_id()), epoch)),
            )
            .execute(self.conn)?;

            insert_into(scabbard_v3_commit_history::table)
                .values(vec![CommitEntryModel::try_from(&commit_entry)?])
                .execute(self.conn)?;

            Ok(())
        })
    }
}
