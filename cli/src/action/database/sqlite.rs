// Copyright 2018-2020 Cargill Incorporated
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

//! Provides sqlite migration support to the database action

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use diesel::{
    r2d2::{ConnectionManager, Pool},
    sqlite::SqliteConnection,
};

use splinter::migrations::run_sqlite_migrations;

use crate::error::CliError;

const SPLINTER_HOME_ENV: &str = "SPLINTER_HOME";
const SPLINTER_STATE_DIR_ENV: &str = "SPLINTER_STATE_DIR";
const DEFAULT_STATE_DIR: &str = "/var/lib/splinter";
const DEFAULT_SQLITE: &str = "splinter_state.db";

/// Run the sqlite migrations against the provided connection string
pub fn sqlite_migrations(connection_string: String) -> Result<(), CliError> {
    let connection_manager = ConnectionManager::<SqliteConnection>::new(&connection_string);
    let mut pool_builder = Pool::builder();

    if connection_string == ":memory:" {
        pool_builder = pool_builder.max_size(1)
    }

    let pool = pool_builder
        .build(connection_manager)
        .map_err(|_| CliError::ActionError("Failed to build connection pool".to_string()))?;

    if connection_string != ":memory:" {
        let path = PathBuf::from(&connection_string);
        let full_path = fs::canonicalize(&path).map_err(|err| {
            CliError::ActionError(format!(
                "Unable to get absolute path for connection string {}: {}",
                connection_string, err,
            ))
        })?;

        info!(
            "Running migrations against SQLite database: {}",
            full_path.display()
        );
    } else {
        info!("Running migrations against SQLite database: :memory: ");
    };

    run_sqlite_migrations(&*pool.get().map_err(|_| {
        CliError::ActionError("Failed to get connection for migrations".to_string())
    })?)
    .map_err(|err| CliError::ActionError(format!("Unable to run Sqlite migrations: {}", err)))?;

    Ok(())
}

/// Returns the path to the default sqlite database
///
/// If `SPLINTER_STATE_DIR` is set, returns `SPLINTER_STATE_DIR/splinter_state.db`.
/// If `SPLINTER_HOME` is set, returns `SPLINTER_HOME/data/splinter_state.db`.
/// Otherwise, returns `/var/lib/splinter/splinter_state.db`
pub fn get_default_database() -> Result<String, CliError> {
    let mut opt_path = {
        if let Ok(state_dir) = env::var(SPLINTER_STATE_DIR_ENV) {
            let opt_path = PathBuf::from(&state_dir);
            if !opt_path.is_dir() {
                fs::create_dir_all(&opt_path).map_err(|_| {
                    CliError::ActionError(format!(
                        "Unable to create directory: {}",
                        opt_path.display()
                    ))
                })?;
            }
            opt_path
        } else if let Ok(splinter_home) = env::var(SPLINTER_HOME_ENV) {
            let opt_path = Path::new(&splinter_home).join("data");
            if !opt_path.is_dir() {
                fs::create_dir_all(&opt_path).map_err(|_| {
                    CliError::ActionError(format!(
                        "Unable to create directory: {}",
                        opt_path.display()
                    ))
                })?;
            }
            opt_path
        } else {
            let opt_path = PathBuf::from(&DEFAULT_STATE_DIR);
            if !opt_path.is_dir() {
                fs::create_dir_all(&opt_path).map_err(|_| {
                    CliError::ActionError(format!(
                        "Unable to create directory: {}",
                        opt_path.display()
                    ))
                })?;
            }
            opt_path
        }
    };

    opt_path = opt_path.join(DEFAULT_SQLITE);
    let database_file = opt_path.to_str().map(ToOwned::to_owned).ok_or_else(|| {
        CliError::ActionError(format!(
            "Unable get database default database file: {}",
            opt_path.display()
        ))
    })?;

    Ok(database_file)
}
