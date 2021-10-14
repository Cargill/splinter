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

//! Provides sqlite migration support to the database action

use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    sqlite::SqliteConnection,
};

use splinter::migrations::run_sqlite_migrations;

use super::SplinterEnvironment;
use crate::error::CliError;

const DEFAULT_SQLITE: &str = "splinter_state.db";
const MEMORY: &str = ":memory:";

/// Run the sqlite migrations against the provided connection string
pub fn sqlite_migrations(connection_string: String) -> Result<(), CliError> {
    if connection_string != MEMORY {
        if let Err(err) = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&connection_string)
        {
            match err.kind() {
                std::io::ErrorKind::NotFound => (),
                _ => {
                    return Err(CliError::ActionError(format!(
                        "While opening: {} received {}",
                        &connection_string, err
                    )))
                }
            }
        }
    }
    let connection_manager = ConnectionManager::<SqliteConnection>::new(&connection_string);
    let mut pool_builder = Pool::builder();

    if connection_string == MEMORY {
        pool_builder = pool_builder.max_size(1)
    }

    let pool = pool_builder
        .build(connection_manager)
        .map_err(|_| CliError::ActionError("Failed to build connection pool".to_string()))?;

    if connection_string != MEMORY {
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
        #[cfg(feature = "scabbard-receipt-store")]
        info!(
            "Running migrations against SQLite database for receipt store: {}",
            full_path.display()
        );
    } else {
        info!("Running migrations against SQLite database: :memory: ");
        #[cfg(feature = "scabbard-receipt-store")]
        info!("Running migrations against SQLite database: :memory: for receipt store");
    };

    run_sqlite_migrations(&*pool.get().map_err(|_| {
        CliError::ActionError("Failed to get connection for migrations".to_string())
    })?)
    .map_err(|err| CliError::ActionError(format!("Unable to run Sqlite migrations: {}", err)))?;

    #[cfg(feature = "scabbard-migrations")]
    {
        scabbard::migrations::run_sqlite_migrations(&*pool.get().map_err(|_| {
            CliError::ActionError("Failed to get connection for migrations".to_string())
        })?)
        .map_err(|err| {
            CliError::ActionError(format!(
                "Unable to run Sqlite migrations for scabbard: {}",
                err
            ))
        })?;
    }

    Ok(())
}

/// Creates and returns the path to the default sqlite database
///
/// Gets the splinter default state path, creating it if it does not exist. Creates a db file with
/// the name splinter_state.db.
pub fn get_default_database() -> Result<String, CliError> {
    let state_path = SplinterEnvironment::load().get_state_path();
    if !state_path.is_dir() {
        fs::create_dir_all(&state_path).map_err(|_| {
            CliError::ActionError(format!(
                "Unable to create directory: {}",
                state_path.display()
            ))
        })?;
    }

    get_database_at_state_path(state_path)
}

/// Gets the path to the sqlite database given the specified state path
pub fn get_database_at_state_path<P: Into<PathBuf>>(state_path: P) -> Result<String, CliError> {
    let state_path: PathBuf = state_path.into();
    let opt_path = state_path.join(DEFAULT_SQLITE);
    let database_file = opt_path.to_str().map(ToOwned::to_owned).ok_or_else(|| {
        CliError::ActionError(format!(
            "Unable get database default database file: {}",
            opt_path.display()
        ))
    })?;

    Ok(database_file)
}
