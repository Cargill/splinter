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

//! Builder for Scabbard configuration

use std::path::PathBuf;

use splinter::error::InternalError;

const DEFAULT_TEST_DB_SIZE: usize = 120 * 1024 * 1024;

/// Builder for scabbard configuration
#[derive(Default)]
pub struct ScabbardConfigBuilder {
    data_dir: Option<PathBuf>,
    database_size: Option<usize>,
}

impl ScabbardConfigBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the directory where service data will be stored.
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = Some(path);
        self
    }

    /// Sets the size of the LMDB databases that will be created per scabbard service.
    pub fn with_database_size(mut self, database_size: usize) -> Self {
        self.database_size = Some(database_size);
        self
    }

    /// Constructs the ScabbardConfig.
    ///
    /// # Errors
    ///
    /// Returns an InternalError if the data directory has been ommitted.
    pub fn build(self) -> Result<ScabbardConfig, InternalError> {
        let database_size = self.database_size.unwrap_or(DEFAULT_TEST_DB_SIZE);
        let data_dir = self
            .data_dir
            .ok_or_else(|| InternalError::with_message("A data directory is required.".into()))?;

        Ok(ScabbardConfig {
            database_size,
            data_dir,
        })
    }
}

/// Configuration for the use of Scabbard service
pub struct ScabbardConfig {
    /// The directory where service data will be stored.
    pub(crate) data_dir: PathBuf,
    /// The size of the LMDB databases that will be generated per scabbard service instance.
    pub(crate) database_size: usize,
}
