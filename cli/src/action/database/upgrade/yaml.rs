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

//! Provides the import action for yaml

use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use splinter::admin::store::error::AdminServiceStoreError;
use splinter::admin::store::yaml::YamlAdminServiceStore;
use splinter::admin::store::AdminServiceStore;
use splinter::admin::store::CircuitNodeBuilder;

use crate::error::CliError;

const CIRCUITS_FILE: &str = "circuits.yaml";
const PROPOSALS_FILE: &str = "circuit_proposals.yaml";

/// Import all the data from one store to another store
fn import_store(
    to: &'_ dyn AdminServiceStore,
    from: &'_ dyn AdminServiceStore,
) -> Result<ImportResult, ImportError> {
    let mut import_result = ImportResult::default();
    let predicates = &[];

    let nodes = from.list_nodes().map_err(ImportError::Store)?;
    let endpoints: HashMap<String, Vec<String>> = nodes
        .map(|node| (node.node_id().to_string(), node.endpoints().to_vec()))
        .collect();

    let circuits = from.list_circuits(predicates).map_err(ImportError::Store)?;

    for circuit in circuits {
        let id = circuit.circuit_id().to_string();

        // Yaml circuits do not store the endpoints, so we're adding them from the nodes
        // definition. Not doing this will cause the database to enter an invalid state,
        // and listing the circuits will fail
        let members = circuit
            .members()
            .iter()
            .map(|member| {
                let node_id = member.node_id();
                if let Some(endpoints) = endpoints.get(node_id) {
                    CircuitNodeBuilder::new()
                        .with_node_id(node_id)
                        .with_endpoints(endpoints)
                        .build()
                        .map_err(|e| {
                            ImportError::Endpoint(format!("could not build node endpoint: {}", e))
                        })
                } else {
                    Err(ImportError::Endpoint(node_id.to_string()))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        to.add_circuit(circuit, members)
            .map_err(|e| ImportError::Circuit(id, e))?;
        import_result.circuits += 1;
    }

    let proposals = from
        .list_proposals(predicates)
        .map_err(ImportError::Store)?;
    for proposal in proposals {
        let requester = proposal.requester_node_id().to_string();
        to.add_proposal(proposal)
            .map_err(|e| ImportError::Proposal(requester, e))?;
        import_result.proposals += 1;
    }

    Ok(import_result)
}

/// Import yaml state from the specified directory to a database
pub fn import_yaml_state_to_database(
    state_dir: &Path,
    db_store: &'_ dyn AdminServiceStore,
) -> Result<(), CliError> {
    fn invalid_utf8() -> CliError {
        CliError::ActionError("'state_dir' is not a valid UTF-8 string".to_string())
    }

    let state_dir: PathBuf = state_dir.into();
    let circuits_location = state_dir.join(CIRCUITS_FILE);
    let proposals_location = state_dir.join(PROPOSALS_FILE);

    if !(circuits_location.exists() || proposals_location.exists()) {
        warn!("Skipping yaml state import: no yaml state files found");
        return Ok(());
    }

    let yaml_admin_service_store = YamlAdminServiceStore::new(
        circuits_location
            .to_str()
            .ok_or_else(invalid_utf8)?
            .to_string(),
        proposals_location
            .to_str()
            .ok_or_else(invalid_utf8)?
            .to_string(),
    )
    .map_err(|err| {
        CliError::ActionError(format!("unable to create YamlAdminServiceStore: {}", err))
    })?;

    info!("Processing import data... ");
    let result = import_store(db_store, &yaml_admin_service_store).map_err(|e| {
        CliError::ActionError(match e.source() {
            Some(source) => format!("{}: {}", e.to_string(), source),
            None => e.to_string(),
        })
    })?;

    info!("Backing up state files... ");
    let new_circuits_location = circuits_location.with_extension("yaml.old");
    let new_proposals_location = proposals_location.with_extension("yaml.old");
    fs::rename(circuits_location, new_circuits_location)
        .map_err(|e| CliError::ActionError(format!("could not move circuits file: {}", e)))?;
    fs::rename(proposals_location, new_proposals_location)
        .map_err(|e| CliError::ActionError(format!("could not move proposals file: {}", e)))?;

    info!(
        "Successfully imported {} circuit(s) and {} proposal(s)",
        result.circuits, result.proposals
    );

    Ok(())
}

/// Represents errors that may occur during the import process
#[derive(Debug)]
enum ImportError {
    Circuit(String, AdminServiceStoreError),
    Proposal(String, AdminServiceStoreError),
    Store(AdminServiceStoreError),
    Endpoint(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Circuit(id, _) => write!(f, "error importing circuit id {}", id),
            ImportError::Proposal(id, _) => {
                write!(f, "error importing proposal from node id {}", id)
            }
            ImportError::Store(_) => write!(f, "error with circuit store"),
            ImportError::Endpoint(node_id) => {
                write!(f, "could not get endpoint for node: {}", node_id)
            }
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ImportError::Circuit(_, admin_store_error) => Some(admin_store_error),
            ImportError::Proposal(_, admin_store_error) => Some(admin_store_error),
            ImportError::Store(admin_store_error) => Some(admin_store_error),
            ImportError::Endpoint(_) => None,
        }
    }
}

struct ImportResult {
    circuits: usize,
    proposals: usize,
}

impl Default for ImportResult {
    fn default() -> Self {
        ImportResult {
            circuits: 0,
            proposals: 0,
        }
    }
}

#[cfg(test)]
mod action_tests {
    use super::*;

    use std::fs::File;
    use std::io::Write;
    use std::path::Path;

    use tempdir::TempDir;

    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::sqlite::SqliteConnection;

    use splinter::admin::store::diesel::DieselAdminServiceStore;
    use splinter::migrations::run_sqlite_migrations;

    const CIRCUIT_STATE: &[u8] = b"---
nodes:
    acme-node-000:
        id: acme-node-000
        endpoints:
          - \"tcps://splinterd-node-acme:8044\"
    bubba-node-000:
        id: bubba-node-000
        endpoints:
          - \"tcps://splinterd-node-bubba:8044\"
circuits:
    WBKLF-AAAAA:
        id: WBKLF-AAAAA
        auth: Trust
        members:
          - bubba-node-000
          - acme-node-000
        roster:
          - service_id: a000
            service_type: scabbard
            allowed_nodes:
              - acme-node-000
            arguments:
              admin_keys: '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
              peer_services: '[\"a001\"]'
          - service_id: a001
            service_type: scabbard
            allowed_nodes:
              - bubba-node-000
            arguments:
              admin_keys: '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
              peer_services: '[\"a000\"]'
        persistence: Any
        durability: NoDurability
        routes: Any
        circuit_management_type: gameroom
        circuit_status: Active";

    const PROPOSAL_STATE: &[u8] = b"---
proposals:
    WBKLF-BBBBB:
        proposal_type: Create
        circuit_id: WBKLF-BBBBB
        circuit_hash: 7ddc426972710adc0b2ecd49e89a9dd805fb9206bf516079724c887bedbcdf1d
        circuit:
            circuit_id: WBKLF-BBBBB
            roster:
            - service_id: a000
              service_type: scabbard
              allowed_nodes:
                - acme-node-000
              arguments:
                - - peer_services
                  - '[\"a001\"]'
                - - admin_keys
                  - '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
            - service_id: a001
              service_type: scabbard
              allowed_nodes:
                - bubba-node-000
              arguments:
                - - peer_services
                  - '[\"a000\"]'
                - - admin_keys
                  - '[\"035724d11cae47c8907f8bfdf510488f49df8494ff81b63825bad923733c4ac550\"]'
            members:
            - node_id: bubba-node-000
              endpoints:
                - \"tcps://splinterd-node-bubba:8044\"
            - node_id: acme-node-000
              endpoints:
                - \"tcps://splinterd-node-acme:8044\"
            authorization_type: Trust
            persistence: Any
            durability: NoDurability
            routes: Any
            circuit_management_type: gameroom
            display_name: \"test_display\"
            circuit_status: Active
        votes: []
        requester: 0283a14e0a17cb7f665311e9b5560f4cde2b502f17e2d03223e15d90d9318d7482
        requester_node_id: acme-node-000";

    // Validate that the import process can successfully import state
    // from existing YAML files
    //
    // 1. Creates a temp directory with existing circuit and proposals yaml files
    // 2. Creates a test database to import data to
    // 3. Tests the output of the import command to validate that it's successful
    #[test]
    fn test_import_command_success() {
        let TempData {
            temp_dir,
            circuits_path,
            proposals_path,
        } = create_temp_files_from_data(CIRCUIT_STATE, PROPOSAL_STATE);

        let pool = create_connection_pool_and_migrate();
        let db_store = DieselAdminServiceStore::new(pool);

        // Verify the store is empty
        assert_eq!(db_store.list_circuits(&[]).unwrap().count(), 0);
        assert_eq!(db_store.list_proposals(&[]).unwrap().count(), 0);

        let result = import_yaml_state_to_database(&temp_dir.path(), &db_store);

        if result.is_err() {
            panic!("received unexpected error: {:?}", result);
        }

        // Test that the old filenames no longer exist
        assert!(!Path::new(&circuits_path).exists());
        assert!(!Path::new(&proposals_path).exists());

        // Test that the new filenames exist
        assert!(Path::new(&circuits_path)
            .with_file_name("circuits.yaml.old")
            .exists());
        assert!(Path::new(&proposals_path)
            .with_file_name("circuit_proposals.yaml.old")
            .exists());

        // Test that the circuits and proposals now exist in the db store
        assert_eq!(db_store.list_circuits(&[]).unwrap().count(), 1);
        assert_eq!(db_store.list_proposals(&[]).unwrap().count(), 1);
    }

    #[test]
    fn test_import_command_files_do_not_exist_aborts() {
        // Create only the temporary directory, but no state files
        let temp_dir = TempDir::new("test_no_files").expect("Failed to create temp dir");

        let pool = create_connection_pool_and_migrate();
        let db_store = DieselAdminServiceStore::new(pool);

        let result = import_yaml_state_to_database(&temp_dir.path(), &db_store);

        // The function returns Ok(()) and logs a message, checking the message is logged doesn't
        // seem to be possible currently.
        match result {
            Err(_) => panic!("received unexpected result"),
            Ok(()) => (),
        }
    }

    fn write_file(data: &[u8], file_path: &str) {
        let mut file = File::create(file_path).expect("Error creating test yaml file.");
        file.write_all(data)
            .expect("unable to write test file to temp dir")
    }

    struct TempData {
        temp_dir: TempDir,
        circuits_path: String,
        proposals_path: String,
    }

    fn create_temp_files_from_data(circuits: &[u8], proposals: &[u8]) -> TempData {
        let temp_dir = TempDir::new("test_read_existing_files").expect("Failed to create temp dir");
        let circuits_path = temp_dir
            .path()
            .join(CIRCUITS_FILE)
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let proposals_path = temp_dir
            .path()
            .join(PROPOSALS_FILE)
            .to_str()
            .expect("Failed to get path")
            .to_string();

        // write yaml files to temp_dir
        write_file(circuits, &circuits_path);
        write_file(proposals, &proposals_path);

        TempData {
            temp_dir,
            circuits_path,
            proposals_path,
        }
    }

    /// Creates a connection pool for an in-memory SQLite database with only a single connection
    /// available. Each connection is backed by a different in-memory SQLite database, so limiting
    /// the pool to a single connection ensures that the same DB is used for all operations.
    fn create_connection_pool_and_migrate() -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }
}
