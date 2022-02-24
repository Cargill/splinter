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

//! Provides scabbard receipt store upgrade functionality

use std::fmt::Write;
use std::path::Path;

use openssl::hash::{hash, MessageDigest};
use sawtooth::receipt::store::{lmdb::LmdbReceiptStore, ReceiptStore};

use crate::action::database::{stores::new_upgrade_stores, ConnectionUri};
use crate::error::CliError;

/// Migrate all of the transaction receipts to the `ReceiptStore`.
pub(super) fn upgrade_scabbard_receipt_store(
    receipt_db_dir: &Path,
    database_uri: &ConnectionUri,
) -> Result<(), CliError> {
    let upgrade_stores =
        new_upgrade_stores(database_uri).map_err(|e| CliError::ActionError(format!("{}", e)))?;

    let node_id = if let Some(node_id) = upgrade_stores
        .new_node_id_store()
        .get_node_id()
        .map_err(|e| CliError::ActionError(format!("{}", e)))?
    {
        node_id
    } else {
        // This node has not even set a node id, so it cannot have any circuits.
        info!("Skipping scabbard receipt store upgrade, no local node ID found");
        return Ok(());
    };

    let circuits = upgrade_stores
        .new_admin_service_store()
        .list_circuits(&[])
        .map_err(|e| CliError::ActionError(format!("{}", e)))?;

    if circuits.len() == 0 {
        info!("Skipping scabbard receipt store upgrade, no circuits found");
        Ok(())
    } else {
        let local_services = circuits.into_iter().flat_map(|circuit| {
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
        });

        let local_services_with_file: Vec<(String, String, String)> = local_services
            .map(|(circuit_id, service_id)| {
                match compute_receipt_db_file_name(&circuit_id, &service_id) {
                    Ok(file) => Ok((circuit_id, service_id, file)),
                    Err(e) => Err(CliError::ActionError(format!("{}", e))),
                }
            })
            .collect::<Result<Vec<(_, _, _)>, _>>()?;

        let lmdb_file_names: Vec<String> = local_services_with_file
            .iter()
            .map(|(_, _, file)| file.clone())
            .collect();

        let mut lmdb_receipt_store = LmdbReceiptStore::new(
            receipt_db_dir,
            &lmdb_file_names,
            lmdb_file_names[0].clone(),
            None,
        )
        .map_err(|e| CliError::ActionError(format!("{}", e)))?;

        for (circuit_id, service_id, file) in local_services_with_file {
            let filename = receipt_db_dir.join(&file);
            let new_filename = receipt_db_dir.join(format!("{}.old", &file));
            lmdb_receipt_store
                .set_current_db(file)
                .map_err(|e| CliError::ActionError(format!("{}", e)))?;
            let receipt_iter = lmdb_receipt_store
                .list_receipts_since(None)
                .map_err(|e| CliError::ActionError(format!("{}", e)))?;
            let db_receipt_store = upgrade_stores.new_receipt_store(&circuit_id, &service_id);
            let receipts = receipt_iter
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| CliError::ActionError(format!("{}", e)))?;
            db_receipt_store
                .add_txn_receipts(receipts)
                .map_err(|e| CliError::ActionError(format!("{}", e)))?;
            std::fs::rename(filename, new_filename)?;
        }
        Ok(())
    }
}

/// Compute the LMDB file name for a circuit_id service_id pair.
fn compute_receipt_db_file_name(circuit_id: &str, service_id: &str) -> Result<String, CliError> {
    let hash = hash(
        MessageDigest::sha256(),
        format!("{}::{}", service_id, circuit_id).as_bytes(),
    )
    .map(|digest| to_hex(&*digest))
    .map_err(|e| CliError::ActionError(format!("{}", e)))?;
    let db_file = format!("{}-receipts.lmdb", hash);
    Ok(db_file)
}

fn to_hex(bytes: &[u8]) -> String {
    let mut buf = String::new();
    for b in bytes {
        write!(&mut buf, "{:02x}", b).expect("Unable to write to string");
    }

    buf
}
