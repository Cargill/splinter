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
) -> Result<WarningEmitted, CliError> {
    match (from.get_node_id(), to.get_node_id()) {
        (Ok(Some(id)), Ok(None)) => to
            .set_node_id(id)
            .map_err(|e| CliError::ActionError(format!("{}", e)))
            .map(|_| WarningEmitted::No),
        (Ok(Some(_)), Ok(Some(_))) => Err(CliError::ActionError(
            "Skipping node_id import: destination store already has node_id set".to_string(),
        )),
        (Ok(None), _) => {
            warn!("Skipping node_id import: node_id file is empty");
            Ok(WarningEmitted::Yes)
        }
        (Err(err), _) => {
            warn!("Skipping node_id import");
            debug!("{}", err);
            Ok(WarningEmitted::Yes)
        }
        (_, Err(err)) => {
            warn!("Skipping node_id import");
            debug!("{}", err);
            Ok(WarningEmitted::Yes)
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

    if let Ok(WarningEmitted::No) = result {
        info!(
            "Renaming {} to {}",
            filename.to_string_lossy(),
            new_filename.to_string_lossy()
        );
        std::fs::rename(filename, new_filename)?;
    }
    result.map(|_| ())
}

enum WarningEmitted {
    Yes,
    No,
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs::File;
    use std::io::Write;

    use splinter::error::InternalError;
    use splinter::node_id::store::{error::NodeIdStoreError, NodeIdStore};
    use tempfile::Builder;

    use super::*;

    const NODE_ID: &str = "qwerty";
    const ALT_NODE_ID: &str = "yuiop";

    struct MockNodeIdStore {
        pub value: RefCell<Option<String>>,
    }

    impl NodeIdStore for MockNodeIdStore {
        fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
            Ok(self.value.borrow().to_owned())
        }
        fn set_node_id(&self, node_id: String) -> Result<(), NodeIdStoreError> {
            self.value.replace(Some(node_id));
            Ok(())
        }
    }

    impl MockNodeIdStore {
        fn new(value: Option<String>) -> Self {
            Self {
                value: RefCell::new(value),
            }
        }
    }

    struct MockErrorNodeIdStore {}

    impl NodeIdStore for MockErrorNodeIdStore {
        fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
            Err(NodeIdStoreError::InternalError(
                InternalError::with_message(String::from(
                    "This is an intentional error, please disregard",
                )),
            ))
        }
        fn set_node_id(&self, _node_id: String) -> Result<(), NodeIdStoreError> {
            Err(NodeIdStoreError::InternalError(
                InternalError::with_message(String::from(
                    "This is an intentional error, please disregard",
                )),
            ))
        }
    }

    #[test]
    // Happy path test for importing a node_id from one store to another empty one.
    // Simply tests that in the ideal case there are no errors or debug messages posted.
    fn test_import_to_empty_store() {
        let empty_store = MockNodeIdStore::new(None);
        assert!(empty_store.value.borrow().is_none());
        let non_empty_store = MockNodeIdStore::new(Some(NODE_ID.to_string()));
        assert!(non_empty_store.value.borrow().as_ref().is_some());
        assert!(non_empty_store.value.borrow().as_ref().unwrap() == NODE_ID);
        let import_result = import_store(&empty_store, &non_empty_store);
        assert!(import_result.is_ok());
        assert!(matches!(import_result.unwrap(), WarningEmitted::No));
        assert!(non_empty_store.get_node_id().is_ok());
        assert!(non_empty_store.get_node_id().unwrap() == Some(NODE_ID.to_string()));
    }

    #[test]
    // Test that importing a node_id to a store that already has a node_id set will return an
    // error.
    fn test_import_to_non_empty_store() {
        let main_store = MockNodeIdStore::new(Some(String::from(NODE_ID)));
        let alt_store = MockNodeIdStore::new(Some(String::from(ALT_NODE_ID)));
        assert!(main_store.get_node_id().is_ok());
        assert!(main_store.get_node_id().unwrap().is_some());
        assert!(main_store.get_node_id().unwrap() == Some(String::from(NODE_ID)));

        assert!(alt_store.get_node_id().is_ok());
        assert!(alt_store.get_node_id().unwrap().is_some());
        assert!(alt_store.get_node_id().unwrap() == Some(String::from(ALT_NODE_ID)));
        let import_result = import_store(&main_store, &alt_store);
        assert!(import_result.is_err());
        assert!(main_store.get_node_id().unwrap() == Some(String::from(NODE_ID)));
    }

    #[test]
    // Test that a debug message is shown if the store being written too has some sort of an error.
    fn test_import_store_write_error() {
        let error_store = MockErrorNodeIdStore {};
        let normal_store = MockNodeIdStore::new(Some(String::from(NODE_ID)));
        let import_result = import_store(&error_store, &normal_store);
        assert!(import_result.is_ok());
        assert!(matches!(import_result.unwrap(), WarningEmitted::Yes));
    }

    #[test]
    // Test that a debug message is shown if the store being read from has some sort of an error.
    fn test_import_store_read_error() {
        let error_store = MockErrorNodeIdStore {};
        let normal_store = MockNodeIdStore::new(Some(String::from(NODE_ID)));
        let import_result = import_store(&normal_store, &error_store);
        assert!(import_result.is_ok());
        assert!(matches!(import_result.unwrap(), WarningEmitted::Yes));
    }

    #[test]
    // Test reading from the node_id file works as intended.
    fn test_migrate_to_db_with_file() {
        let directory = Builder::new()
            .prefix("test")
            .tempdir()
            .expect("could not create temp directory");
        let path = directory.path();
        let mut file = File::create(path.join("node_id")).expect("could not open node_id file");
        write!(file, "{}", NODE_ID).expect("could not write to node_id file");
        let empty_store = MockNodeIdStore::new(None);
        let migrate_result = migrate_node_id_to_db(path.to_path_buf(), &empty_store);
        let new_file = File::open(path.join("node_id.old"));
        let old_file = File::open(path.join("node_id"));
        assert!(old_file.is_err());
        assert!(new_file.is_ok());
        assert!(migrate_result.is_ok());
        assert!(empty_store.get_node_id().is_ok());
        assert!(empty_store.get_node_id().unwrap().is_some());
        assert!(empty_store.get_node_id().unwrap().unwrap() == NODE_ID);
    }

    #[test]
    // Test reading from an empty node_id file fails how we expect it to.
    fn test_migrate_from_empty_file() {
        let directory = Builder::new()
            .prefix("test")
            .tempdir()
            .expect("could not create temp directory");
        let path = directory.path();
        File::create(path.join("node_id")).expect("could not open node_id file");
        let empty_store = MockNodeIdStore::new(None);
        let migrate_result = migrate_node_id_to_db(path.to_path_buf(), &empty_store);
        let new_file = File::open(path.join("node_id.old"));
        let old_file = File::open(path.join("node_id"));
        assert!(old_file.is_ok());
        assert!(new_file.is_err());
        assert!(migrate_result.is_ok());
        assert!(empty_store.get_node_id().is_ok());
        assert!(empty_store.get_node_id().unwrap().is_none());
    }

    #[test]
    // Test migrating to a store with values doesn't overwrite its contained value or move the
    // node_id file.
    fn test_migrate_to_store_with_value() {
        let directory = Builder::new()
            .prefix("test")
            .tempdir()
            .expect("could not create temp directory");
        let path = directory.path();
        let mut file = File::create(path.join("node_id")).expect("could not open node_id file");
        write!(file, "{}", NODE_ID).expect("could not write to node_id file");
        let store = MockNodeIdStore::new(Some(String::from(ALT_NODE_ID)));
        let migrate_result = migrate_node_id_to_db(path.to_path_buf(), &store);
        let new_file = File::open(path.join("node_id.old"));
        let old_file = File::open(path.join("node_id"));
        assert!(old_file.is_ok());
        assert!(new_file.is_err());
        assert!(migrate_result.is_err());
        assert!(store.get_node_id().is_ok());
        assert!(store.get_node_id().unwrap().is_some());
        assert!(store.get_node_id().unwrap().unwrap() == ALT_NODE_ID);
    }
}
