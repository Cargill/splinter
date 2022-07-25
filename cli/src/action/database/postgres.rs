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

use diesel::{pg::PgConnection, Connection};
use splinter::migrations::run_postgres_migrations;

use crate::error::CliError;

pub fn postgres_migrations(url: &str) -> Result<(), CliError> {
    let connection = PgConnection::establish(url).map_err(|err| {
        CliError::ActionError(format!(
            "Failed to establish database connection to '{}': {}",
            url, err
        ))
    })?;

    info!("Running migrations against PostgreSQL database: {}", url);
    run_postgres_migrations(&connection).map_err(|err| {
        CliError::ActionError(format!("Unable to run Postgres migrations: {}", err))
    })?;

    scabbard::migrations::run_postgres_migrations(&connection).map_err(|err| {
        CliError::ActionError(format!(
            "Unable to run Postgres migrations for scabbard: {}",
            err
        ))
    })?;

    #[cfg(feature = "echo")]
    splinter_echo::migrations::run_postgres_migrations(&connection).map_err(|err| {
        CliError::ActionError(format!(
            "Unable to run Postgres migrations for echo: {}",
            err
        ))
    })?;

    Ok(())
}

#[cfg(not(feature = "sqlite"))]
pub fn get_default_database() -> Result<String, CliError> {
    Ok("postgres://admin:admin@localhost:5432/splinterd".to_string())
}
