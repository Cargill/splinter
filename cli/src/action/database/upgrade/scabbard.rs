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

//! Provides scabbard state upgrade functionality

use std::path::Path;

use scabbard::store::{
    transact::{factory::LmdbDatabaseFactory, TransactCommitHashStore},
    CommitHashStore,
};
use splinter::error::InternalError;

use super::error::UpgradeError;

use crate::action::database::{stores::new_upgrade_stores, ConnectionUri};

/// Migrate all of the service state's current commit hashes to the [`CommitHashStore`].
pub(super) fn upgrade_scabbard_commit_hash_state(
    state_dir: &Path,
    database_uri: &ConnectionUri,
) -> Result<(), UpgradeError> {
    let lmdb_db_factory = LmdbDatabaseFactory::new_state_db_factory(state_dir, None);
    let upgrade_stores = new_upgrade_stores(database_uri)?;

    let node_id = if let Some(node_id) = upgrade_stores
        .new_node_id_store()
        .get_node_id()
        .map_err(|e| InternalError::from_source(Box::new(e)))?
    {
        node_id
    } else {
        // This node has not even set a node id, so it cannot have any circuits.
        info!("Skipping scabbard commit hash store upgrade, no local node ID found");
        return Ok(());
    };

    let circuits = upgrade_stores
        .new_admin_service_store()
        .list_circuits(&[])
        .map_err(|e| InternalError::from_source(Box::new(e)))?;

    let local_services = circuits
        .into_iter()
        .map(|circuit| {
            circuit
                .roster()
                .iter()
                .filter_map(|svc| {
                    if svc.node_id() == node_id && svc.service_type() == "scabbard" {
                        Some((
                            circuit.circuit_id().to_string(),
                            svc.service_id().to_string(),
                        ))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .flatten();

    for (circuit_id, service_id) in local_services {
        let lmdb_commit_hash_store =
            TransactCommitHashStore::new(lmdb_db_factory.get_database(&circuit_id, &service_id)?);
        let db_commit_hash_store = upgrade_stores.new_commit_hash_store(&circuit_id, &service_id);

        if let Some(current_commit_hash) = lmdb_commit_hash_store
            .get_current_commit_hash()
            .map_err(|e| InternalError::from_source(Box::new(e)))?
        {
            db_commit_hash_store
                .set_current_commit_hash(&current_commit_hash)
                .map_err(|e| InternalError::from_source(Box::new(e)))?;
            info!("Upgraded scabbard service {}::{}", circuit_id, service_id);
        } else {
            debug!(
                "No commit hash found for service {}::{}",
                circuit_id, service_id
            );
        }
    }

    Ok(())
}
