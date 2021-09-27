// Copyright 2018-2021 Cargill Incorporated
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

//! Transact-backed CommitHashStore implementations.

use splinter::error::{InternalError, InvalidArgumentError, InvalidStateError};
use transact::database::{lmdb::LmdbDatabase, Database, DatabaseError};

use crate::hex;

use super::{CommitHashStore, CommitHashStoreError};

const CURRENT_STATE_ROOT_INDEX: &str = "current_state_root";

/// Provides an LMDB-backed CommitHashStore.
pub type LmdbCommitHashStore = TransactCommitHashStore<LmdbDatabase>;

/// Provides commit log storage using an Transact database in a legacy configuration.
///
/// The database configuration requires a index "current_state_root" configured on the database
/// instance.  This is expected externally. It also doesn't support multiple services per store, as
/// it expects a unique DB instance per store.
#[derive(Clone)]
pub struct TransactCommitHashStore<D>
where
    D: Database + Clone + 'static,
{
    db: D,
}

impl<D: Database + Clone> TransactCommitHashStore<D> {
    /// Constructs a new commit log store around an Transact database instance.
    pub fn new(db: D) -> Self {
        Self { db }
    }
}

impl<D: Database + Clone> CommitHashStore for TransactCommitHashStore<D> {
    fn get_current_commit_hash(&self) -> Result<Option<String>, CommitHashStoreError> {
        let reader = self
            .db
            .get_reader()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        match reader.index_get(CURRENT_STATE_ROOT_INDEX, b"HEAD") {
            Ok(current_commit_hash) => Ok(current_commit_hash.map(|bytes| hex::to_hex(&bytes))),
            Err(DatabaseError::ReaderError(msg)) if msg.starts_with("Not an index") => Err(
                CommitHashStoreError::InvalidState(InvalidStateError::with_message(
                    "Missing current_state_root index in LMDB database".into(),
                )),
            ),
            Err(err) => Err(CommitHashStoreError::Internal(InternalError::from_source(
                Box::new(err),
            ))),
        }
    }

    fn set_current_commit_hash(&self, commit_hash: &str) -> Result<(), CommitHashStoreError> {
        let current_root_bytes = hex::parse_hex(commit_hash).map_err(|e| {
            InvalidArgumentError::new(
                "commit_hash".into(),
                format!("The commit hash provided is invalid: {}", e),
            )
        })?;

        let mut writer = self
            .db
            .get_writer()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        match writer.index_put(CURRENT_STATE_ROOT_INDEX, b"HEAD", &current_root_bytes) {
            Ok(()) => (),

            Err(DatabaseError::WriterError(msg)) if msg.starts_with("Not an index") => {
                return Err(CommitHashStoreError::InvalidState(
                    InvalidStateError::with_message(
                        "Missing current_state_root index in LMDB database".into(),
                    ),
                ))
            }
            Err(err) => {
                return Err(CommitHashStoreError::Internal(InternalError::from_source(
                    Box::new(err),
                )))
            }
        }

        writer
            .commit()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        Ok(())
    }

    fn clone_boxed(&self) -> Box<dyn CommitHashStore> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::error::Error;
    use std::fs::remove_file;
    use std::panic;
    use std::path::Path;
    use std::thread;

    use transact::{
        database::{
            lmdb::{LmdbContext, LmdbDatabase},
            DatabaseError,
        },
        state::merkle::INDEXES,
    };

    /// Test that a hash can be stored with an LMDB back-end
    /// (Note, this test only uses a single service ID, as the LMDB implementation assumes that
    /// there is single file representing all of the scabbard state, with a single index table
    /// supporting the commit hash).
    #[test]
    fn test_lmdb_store() -> Result<(), Box<dyn Error>> {
        run_lmdb_test(|dbpath| {
            let mut indexes = INDEXES.to_vec();
            indexes.push(CURRENT_STATE_ROOT_INDEX);
            let db = make_lmdb(&indexes, dbpath)?;

            let commit_log_store = LmdbCommitHashStore::new(db);

            assert_eq!(None, commit_log_store.get_current_commit_hash()?);

            commit_log_store.set_current_commit_hash("abcdef0123456789")?;

            assert_eq!(
                Some("abcdef0123456789".to_string()),
                commit_log_store.get_current_commit_hash()?
            );

            Ok(())
        })
    }

    /// Test that the LMDB implementation returns an error on get or set if the index table is not
    /// present.
    #[test]
    fn test_lmdb_store_missing_index() -> Result<(), Box<dyn Error>> {
        run_lmdb_test(|dbpath| {
            let db = make_lmdb(&INDEXES, dbpath)?;

            let commit_log_store = LmdbCommitHashStore::new(db);

            let res = commit_log_store.get_current_commit_hash();

            assert!(
                matches!(res, Err(CommitHashStoreError::InvalidState(_))),
                "Expected invalid state error, got {:?}",
                res
            );

            let res = commit_log_store.set_current_commit_hash("abcdef0123456789");

            assert!(
                matches!(res, Err(CommitHashStoreError::InvalidState(_))),
                "Expected invalid state error, got {:?}",
                res
            );

            Ok(())
        })
    }

    pub fn run_lmdb_test<T>(test: T) -> Result<(), Box<dyn Error>>
    where
        T: FnOnce(&str) -> Result<(), Box<dyn Error>> + panic::UnwindSafe,
    {
        let dbpath = temp_db_path()?;

        let testpath = dbpath.clone();
        let result = panic::catch_unwind(move || test(&testpath));

        remove_file(dbpath)?;

        match result {
            Ok(_) => Ok(()),
            Err(err) => {
                panic::resume_unwind(err);
            }
        }
    }

    fn make_lmdb(indexes: &[&str], merkle_path: &str) -> Result<LmdbDatabase, Box<dyn Error>> {
        let ctx = LmdbContext::new(
            Path::new(merkle_path),
            indexes.len(),
            Some(120 * 1024 * 1024),
        )
        .map_err(|err| DatabaseError::InitError(format!("{}", err)))?;

        Ok(LmdbDatabase::new(ctx, indexes)
            .map_err(|err| DatabaseError::InitError(format!("{}", err)))?)
    }

    fn temp_db_path() -> Result<String, Box<dyn Error>> {
        let mut temp_dir = env::temp_dir();

        let thread_id = thread::current().id();
        temp_dir.push(format!("merkle-{:?}.lmdb", thread_id));
        Ok(temp_dir
            .to_str()
            .ok_or_else(|| InternalError::with_message("Unable to convert path to string".into()))?
            .to_string())
    }
}
