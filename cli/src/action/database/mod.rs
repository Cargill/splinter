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

#[cfg(feature = "sqlite")]
mod sqlite;

use std::str::FromStr;

use clap::ArgMatches;
use diesel::{connection::Connection as _, pg::PgConnection};

use splinter::migrations::run_postgres_migrations;

use crate::error::CliError;

#[cfg(feature = "sqlite")]
use self::sqlite::{get_default_database, sqlite_migrations};

use super::Action;

pub struct MigrateAction;

impl Action for MigrateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = if let Some(args) = arg_matches {
            args.value_of("connect")
                .map(ToOwned::to_owned)
                .unwrap_or(get_default_database()?)
        } else {
            get_default_database()?
        };

        match ConnectionUri::from_str(&url).map_err(CliError::ActionError)? {
            ConnectionUri::Postgres(url) => {
                let connection = PgConnection::establish(&url).map_err(|err| {
                    CliError::ActionError(format!(
                        "Failed to establish database connection to '{}': {}",
                        url, err
                    ))
                })?;
                info!("Running migrations against PostgreSQL database: {}", url);
                run_postgres_migrations(&connection).map_err(|err| {
                    CliError::ActionError(format!("Unable to run Postgres migrations: {}", err))
                })?;
            }
            #[cfg(feature = "sqlite")]
            ConnectionUri::Sqlite(connection_string) => sqlite_migrations(connection_string)?,
        }

        Ok(())
    }
}

#[cfg(not(feature = "sqlite"))]
fn get_default_database() -> Result<String, CliError> {
    Ok("postgres://admin:admin@localhost:5432/splinterd".to_string())
}

/// The possible connection types and identifiers passed to the migrate command
pub enum ConnectionUri {
    #[cfg(feature = "postgres")]
    Postgres(String),
    #[cfg(feature = "sqlite")]
    Sqlite(String),
}

impl FromStr for ConnectionUri {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // check specifically so it does not pass to sqlite
            "memory" => Err(format!("No compatible connection type: {}", s)),
            #[cfg(feature = "postgres")]
            _ if s.starts_with("postgres://") => Ok(ConnectionUri::Postgres(s.into())),
            #[cfg(feature = "sqlite")]
            _ => Ok(ConnectionUri::Sqlite(s.into())),
            #[cfg(not(feature = "sqlite"))]
            _ => Err(format!("No compatible connection type: {}", s)),
        }
    }
}
