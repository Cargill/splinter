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

use diesel::prelude::*;
use splinter::error::InternalError;
use splinter::service::FullyQualifiedServiceId;

use crate::store::scabbard_store::commit::CommitEntry;
use crate::store::scabbard_store::diesel::{
    models::CommitEntryModel, schema::scabbard_v3_commit_history,
};
use crate::store::scabbard_store::ScabbardStoreError;

use super::ScabbardStoreOperations;

const OPERATION_NAME: &str = "get_last_commit_entry";

pub(in crate::store::scabbard_store::diesel) trait GetLastCommitEntryOperation {
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError>;
}

impl<'a, C> GetLastCommitEntryOperation for ScabbardStoreOperations<'a, C>
where
    C: diesel::Connection,
    i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, C::Backend>,
    String: diesel::deserialize::FromSql<diesel::sql_types::Text, C::Backend>,
{
    fn get_last_commit_entry(
        &self,
        service_id: &FullyQualifiedServiceId,
    ) -> Result<Option<CommitEntry>, ScabbardStoreError> {
        self.conn.transaction::<_, _, _>(|| {
            scabbard_v3_commit_history::table
                .filter(
                    scabbard_v3_commit_history::circuit_id
                        .eq(service_id.circuit_id().to_string())
                        .and(
                            scabbard_v3_commit_history::service_id
                                .eq(service_id.service_id().to_string()),
                        ),
                )
                .order(scabbard_v3_commit_history::epoch.desc())
                .first::<CommitEntryModel>(self.conn)
                .optional()
                .map_err(|err| {
                    ScabbardStoreError::from_source_with_operation(err, OPERATION_NAME.to_string())
                })?
                .map(CommitEntry::try_from)
                .transpose()
                .map_err(|err| {
                    ScabbardStoreError::Internal(InternalError::from_source(Box::new(err)))
                })
        })
    }
}
