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

//! A NodeIdStore backed by a file.

use std::fs;
use std::path::PathBuf;

use super::NodeIdStore;
use super::NodeIdStoreError;

/// A [NodeIdStore] backed by a file.
/// The 0.4 node_id file is soft-deprecated, this exists to help migrate the node_id.
pub struct FileNodeIdStore {
    filename: PathBuf,
}

impl FileNodeIdStore {
    pub fn new(filename: PathBuf) -> Self {
        Self { filename }
    }
}

impl NodeIdStore for FileNodeIdStore {
    fn get_node_id(&self) -> Result<Option<String>, NodeIdStoreError> {
        fs::read_to_string(&self.filename)
            .map_err(|e| e.into())
            .map(|s| {
                let id = s.trim_end().to_string();
                if !id.is_empty() {
                    Some(id)
                } else {
                    None
                }
            })
    }

    fn set_node_id(&self, node_id: String) -> Result<(), NodeIdStoreError> {
        fs::write(&self.filename, node_id).map_err(|e| e.into())
    }
}
