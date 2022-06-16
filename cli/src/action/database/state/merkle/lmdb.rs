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

use std::cell::RefCell;
use std::collections::HashMap;

use scabbard::store::transact::factory::LmdbDatabaseFactory;
use splinter::error::InternalError;
use transact::{
    database::btree::BTreeDatabase,
    state::{
        merkle::kv::{MerkleRadixTree, MerkleState, INDEXES},
        Committer, DryRunCommitter, Pruner, Reader, State, StateChange, StateError, ValueIter,
        ValueIterResult,
    },
};

pub struct LazyLmdbMerkleState {
    factory: LmdbDatabaseFactory,
    circuit_id: Box<str>,
    service_id: Box<str>,
    initial_state_root_hash: Box<str>,
    inner: RefCell<Option<MerkleState>>,
}

impl LazyLmdbMerkleState {
    pub fn new(
        factory: LmdbDatabaseFactory,
        circuit_id: &str,
        service_id: &str,
        create_tree: bool,
    ) -> Result<Self, InternalError> {
        if !create_tree {
            let path = factory
                .compute_path(circuit_id, service_id)
                .map_err(|e| InternalError::with_message(format!("{}", e)))?
                .with_extension("lmdb");

            if !path.is_file() {
                return Err(InternalError::with_message(format!(
                    "LMDB file for service {}::{} ({:?}) does not exist",
                    circuit_id, service_id, path
                )));
            }
        }
        // Use a BTreeDatabase to produce initial_state_root_hash, which is identical.
        let initial_state_root_hash =
            MerkleRadixTree::new(Box::new(BTreeDatabase::new(&INDEXES)), None)
                .map_err(|e| InternalError::with_message(format!("{}", e)))?
                .get_merkle_root();

        Ok(Self {
            factory,
            circuit_id: circuit_id.into(),
            service_id: service_id.into(),
            initial_state_root_hash: initial_state_root_hash.into(),
            inner: RefCell::new(None),
        })
    }

    pub fn get_state_root(&self) -> String {
        self.initial_state_root_hash.to_string()
    }

    pub fn delete(self) -> Result<(), InternalError> {
        if self.inner.borrow().is_some() {
            self.factory
                .get_database_purge_handle(&*self.circuit_id, &*self.service_id)
                .map_err(|e| InternalError::with_message(format!("{}", e)))?
                .purge()
                .map_err(|e| InternalError::with_message(format!("{}", e)))
        } else {
            Ok(())
        }
    }

    fn get_state(&self) -> Result<MerkleState, transact::error::InternalError> {
        let mut inner = self.inner.borrow_mut();
        if let Some(state) = &*inner {
            return Ok(state.clone());
        }

        let state_db = self
            .factory
            .get_database(&*self.circuit_id, &*self.service_id)
            .map_err(|e| transact::error::InternalError::with_message(format!("{}", e)))?;

        let state = MerkleState::new(Box::new(state_db.clone()));

        // We recompute this, otherwise the tree does not have the correct initial state root
        // nodes persisted, if the tree is new.
        let initial_state_root_hash = MerkleRadixTree::new(Box::new(state_db), None)
            .map_err(|e| transact::error::InternalError::with_message(format!("{}", e)))?
            .get_merkle_root();

        if initial_state_root_hash != *self.initial_state_root_hash {
            return Err(transact::error::InternalError::with_message(format!(
                "BTreeDatabase did not produce the same initial state root hash as \
                 a LmdbDatabase: {} != {}",
                self.initial_state_root_hash, initial_state_root_hash
            )));
        }

        *inner = Some(state.clone());

        Ok(state)
    }
}

impl State for LazyLmdbMerkleState {
    type StateId = String;
    type Key = String;
    type Value = Vec<u8>;
}

impl Committer for LazyLmdbMerkleState {
    type StateChange = StateChange;

    fn commit(
        &self,
        state_id: &Self::StateId,
        state_changes: &[Self::StateChange],
    ) -> Result<Self::StateId, StateError> {
        self.get_state()?.commit(state_id, state_changes)
    }
}

impl DryRunCommitter for LazyLmdbMerkleState {
    type StateChange = StateChange;

    fn dry_run_commit(
        &self,
        state_id: &Self::StateId,
        state_changes: &[Self::StateChange],
    ) -> Result<Self::StateId, StateError> {
        self.get_state()?.commit(state_id, state_changes)
    }
}

impl Reader for LazyLmdbMerkleState {
    /// The filter used for the iterating over state values.
    type Filter = str;

    fn get(
        &self,
        state_id: &Self::StateId,
        keys: &[Self::Key],
    ) -> Result<HashMap<Self::Key, Self::Value>, StateError> {
        self.get_state()?.get(state_id, keys)
    }

    fn filter_iter(
        &self,
        state_id: &Self::StateId,
        filter: Option<&Self::Filter>,
    ) -> ValueIterResult<ValueIter<(Self::Key, Self::Value)>> {
        self.get_state()?.filter_iter(state_id, filter)
    }
}

impl Pruner for LazyLmdbMerkleState {
    fn prune(&self, state_ids: Vec<Self::StateId>) -> Result<Vec<Self::Key>, StateError> {
        self.get_state()?.prune(state_ids)
    }
}
