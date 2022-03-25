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

//! Provides a factory to produce LMDB database instances.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use openssl::hash::{hash, MessageDigest};
use splinter::error::InternalError;
use transact::{
    database::lmdb::{LmdbContext, LmdbDatabase},
    state::merkle::INDEXES,
};

use super::to_hex;
use super::CURRENT_STATE_ROOT_INDEX;

// Linux, with a 64bit CPU supports sparse files of a large size
#[cfg(target_os = "linux")]
const DEFAULT_DB_SIZE: usize = 1 << 40; // 1024 ** 4
#[cfg(any(target_arch = "x86", target_arch = "arm", not(target_os = "linux")))]
const DEFAULT_DB_SIZE: usize = 1 << 30; // 1024 ** 3

#[derive(Clone)]
pub struct LmdbDatabaseFactory {
    db_dir: Arc<Path>,
    db_size: usize,
    db_suffix: Arc<str>,

    indexes: Arc<[&'static str]>,

    databases: Arc<Mutex<HashMap<Box<str>, LmdbDatabase>>>,
}

impl LmdbDatabaseFactory {
    pub fn new_state_db_factory(db_dir: &Path, db_size: Option<usize>) -> Self {
        let mut indexes = INDEXES.to_vec();
        indexes.push(CURRENT_STATE_ROOT_INDEX);
        Self {
            db_dir: db_dir.into(),
            db_size: db_size.unwrap_or(DEFAULT_DB_SIZE),
            db_suffix: "state".into(),
            indexes: indexes.into(),
            databases: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_database(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Result<LmdbDatabase, InternalError> {
        let mut databases = self
            .databases
            .lock()
            .map_err(|_| InternalError::with_message("databases lock has been poisoned".into()))?;

        let key = format!("{}::{}", service_id, circuit_id);
        if let Some(db) = databases.get(&*key) {
            return Ok(db.clone());
        }

        let db_path = self.path_from_key(&key)?.with_extension("lmdb");

        let db = LmdbDatabase::new(
            LmdbContext::new(&db_path, self.indexes.len(), Some(self.db_size))
                .map_err(|e| InternalError::from_source(Box::new(e)))?,
            &*self.indexes,
        )
        .map_err(|e| InternalError::from_source(Box::new(e)))?;

        databases.insert(key.into(), db.clone());

        Ok(db)
    }

    pub fn get_database_purge_handle(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Result<LmdbDatabasePurgeHandle, InternalError> {
        let db_path = self.compute_path(circuit_id, service_id)?;

        Ok(LmdbDatabasePurgeHandle {
            lmdb_path: db_path.into(),
        })
    }

    pub fn get_database_size(&self) -> usize {
        self.db_size
    }

    /// Compute the file path, excluding the extension
    pub fn compute_path(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Result<PathBuf, InternalError> {
        let key = format!("{}::{}", service_id, circuit_id);
        self.path_from_key(&key)
    }

    fn path_from_key(&self, key: &str) -> Result<PathBuf, InternalError> {
        let hash = hash(MessageDigest::sha256(), key.as_bytes())
            .map(|digest| to_hex(&*digest))
            .map_err(|e| InternalError::from_source(Box::new(e)))?;
        let db_path = Path::new(&*self.db_dir)
            .to_path_buf()
            .join(format!("{}-{}", hash, self.db_suffix));

        Ok(db_path)
    }
}

pub struct LmdbDatabasePurgeHandle {
    lmdb_path: Box<Path>,
}

impl LmdbDatabasePurgeHandle {
    pub fn purge(&self) -> Result<(), InternalError> {
        let db_path = self.lmdb_path.with_extension("lmdb");
        let db_lock_file_path = self.lmdb_path.with_extension("lmdb-lock");

        std::fs::remove_file(db_path.as_path())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;
        std::fs::remove_file(db_lock_file_path.as_path())
            .map_err(|err| InternalError::from_source(Box::new(err)))?;

        Ok(())
    }
}
