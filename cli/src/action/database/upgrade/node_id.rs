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

//! Provides the import action for node IDs

use std::path::PathBuf;

use log::info;
use splinter::node_id::store::file::FileNodeIdStore;
use splinter::node_id::store::NodeIdStore;

use crate::error::CliError;

/// Import node_id from one store to another
///
/// # Arguments
///
/// * `to` - the NodeIdStore receiving the node_id
/// * `from` - the NodeIdStore supplying the node_id
fn import_store(
    to: &'_ dyn NodeIdStore,
    from: &'_ dyn NodeIdStore,
) -> Result<WarningEmited, CliError> {
    match (from.get_node_id(), to.get_node_id()) {
        (Ok(Some(id)), Ok(None)) => to
            .set_node_id(id)
            .map_err(|e| CliError::ActionError(format!("{}", e)))
            .map(|_| WarningEmited::No),
        (Ok(Some(_)), Ok(Some(_))) => Err(CliError::ActionError(
            "Skipping node_id import: destination store already has node_id set".to_string(),
        )),
        (Ok(None), _) => {
            warn!("Skipping node_id import: node_id file is empty");
            Ok(WarningEmited::Yes)
        }
        (Err(err), _) => {
            warn!("Skipping node_id import");
            debug!("{}", err);
            Ok(WarningEmited::Yes)
        }
        (_, Err(err)) => {
            warn!("Skipping node_id import");
            debug!("{}", err);
            Ok(WarningEmited::Yes)
        }
    }
}

pub fn migrate_node_id_to_db(
    state_dir: PathBuf,
    db_store: &dyn NodeIdStore,
) -> Result<(), CliError> {
    let mut filename = state_dir.clone();
    filename.push("node_id");
    let mut new_filename = state_dir;
    new_filename.push("node_id.old");
    let file_store = FileNodeIdStore::new(filename.clone());
    info!(
        "Importing node_id from {} to database",
        filename.to_string_lossy()
    );
    let result = import_store(&*db_store, &file_store);

    if let Ok(WarningEmited::No) = result {
        info!(
            "Renaming {} to {}",
            filename.to_string_lossy(),
            new_filename.to_string_lossy()
        );
        std::fs::rename(filename, new_filename)?;
    }
    result.map(|_| ())
}

enum WarningEmited {
    Yes,
    No,
}
