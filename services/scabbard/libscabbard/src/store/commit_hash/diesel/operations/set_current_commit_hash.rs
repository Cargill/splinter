// Copyright 2021 Cargill Incorporated
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

#[cfg(feature = "postgres")]
use diesel::insert_into;
use diesel::prelude::*;
#[cfg(feature = "sqlite")]
use diesel::replace_into;
use splinter::error::InternalError;

use crate::store::{
    diesel::{models::NewCommitHash, schema::scabbard_commit_hash},
    CommitHashStoreError,
};

use super::CommitHashStoreOperations;

pub(in crate::store::commit_hash::diesel) trait CommitHashStoreSetCurrentCommitHashOperation {
    fn set_current_commit_hash(
        &self,
        circuit_id: &str,
        service_id: &str,
        commit_hash: &str,
    ) -> Result<(), CommitHashStoreError>;
}

#[cfg(feature = "sqlite")]
impl<'a> CommitHashStoreSetCurrentCommitHashOperation
    for CommitHashStoreOperations<'a, diesel::sqlite::SqliteConnection>
{
    fn set_current_commit_hash(
        &self,
        circuit_id: &str,
        service_id: &str,
        commit_hash: &str,
    ) -> Result<(), CommitHashStoreError> {
        replace_into(scabbard_commit_hash::table)
            .values(NewCommitHash {
                circuit_id,
                service_id,
                commit_hash,
            })
            .execute(self.conn)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(())
    }
}

#[cfg(feature = "postgres")]
impl<'a> CommitHashStoreSetCurrentCommitHashOperation
    for CommitHashStoreOperations<'a, diesel::pg::PgConnection>
{
    fn set_current_commit_hash(
        &self,
        circuit_id: &str,
        service_id: &str,
        commit_hash: &str,
    ) -> Result<(), CommitHashStoreError> {
        let new_commit_hash = NewCommitHash {
            circuit_id,
            service_id,
            commit_hash,
        };

        insert_into(scabbard_commit_hash::table)
            .values(&new_commit_hash)
            .on_conflict((
                scabbard_commit_hash::circuit_id,
                scabbard_commit_hash::service_id,
            ))
            .do_update()
            .set(&new_commit_hash)
            .execute(self.conn)
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(())
    }
}
