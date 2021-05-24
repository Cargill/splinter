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

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "upgrade")]
mod upgrade;

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{env, fs};

use clap::ArgMatches;
use diesel::{connection::Connection as _, pg::PgConnection};

use splinter::migrations::run_postgres_migrations;

use crate::error::CliError;

#[cfg(feature = "sqlite")]
use self::sqlite::{get_default_database, sqlite_migrations};

use super::Action;

const SPLINTER_HOME_ENV: &str = "SPLINTER_HOME";
const SPLINTER_STATE_DIR_ENV: &str = "SPLINTER_STATE_DIR";
const DEFAULT_STATE_DIR: &str = "/var/lib/splinter";
pub struct MigrateAction;

impl Action for MigrateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let url = if let Some(args) = arg_matches {
            match args.value_of("connect") {
                Some(url) => url.to_owned(),
                None => get_default_database()?,
            }
        } else {
            get_default_database()?
        };

        match ConnectionUri::from_str(&url)? {
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
    type Err = CliError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // check specifically so it does not pass to sqlite
            "memory" => Err(CliError::ActionError(format!(
                "No compatible connection type: {}",
                s
            ))),
            #[cfg(feature = "postgres")]
            _ if s.starts_with("postgres://") => Ok(ConnectionUri::Postgres(s.into())),
            #[cfg(feature = "sqlite")]
            _ => Ok(ConnectionUri::Sqlite(s.into())),
            #[cfg(not(feature = "sqlite"))]
            _ => Err(CliError::ActionError(format!(
                "No compatible connection type: {}",
                s
            ))),
        }
    }
}

/// Represents the SplinterEnvironment data
struct SplinterEnvironment {
    state_dir: Option<String>,
    home_dir: Option<String>,
    default_dir: &'static str,
}

impl SplinterEnvironment {
    pub fn load() -> Self {
        SplinterEnvironment {
            state_dir: env::var(SPLINTER_STATE_DIR_ENV).ok(),
            home_dir: env::var(SPLINTER_HOME_ENV).ok(),
            default_dir: DEFAULT_STATE_DIR,
        }
    }

    fn try_canonicalize<P: Into<PathBuf>>(dir: P) -> PathBuf {
        let dir: PathBuf = dir.into();
        fs::canonicalize(dir.clone()).unwrap_or(dir)
    }

    /// Returns the path to the state directory
    ///
    /// If `SPLINTER_STATE_DIR` is set, returns `SPLINTER_STATE_DIR`.
    /// If `SPLINTER_HOME` is set, returns `SPLINTER_HOME/data`.
    /// Otherwise, returns the default directory `/var/lib/splinter`
    pub fn get_state_path(&self) -> PathBuf {
        if let Some(state_dir) = self.state_dir.as_ref() {
            Self::try_canonicalize(PathBuf::from(&state_dir))
        } else if let Some(home_dir) = self.home_dir.as_ref() {
            Self::try_canonicalize(Path::new(&home_dir).join("data"))
        } else {
            Self::try_canonicalize(PathBuf::from(&self.default_dir))
        }
    }
}

#[cfg(test)]
mod splinter_env_tests {
    use super::*;

    const TEST_STATE_DIR: &str = "/test/state/dir";
    const TEST_HOME: &str = "/test/home";
    const TEST_DEFAULT_DIR: &str = "/test/default";

    #[test]
    fn splinter_environment_get_state_path_with_state_dir_and_home_defaults_to_state() {
        assert_eq!(
            PathBuf::from(TEST_STATE_DIR),
            SplinterEnvironment {
                state_dir: Some(TEST_STATE_DIR.to_string()),
                home_dir: Some(TEST_HOME.to_string()),
                default_dir: TEST_DEFAULT_DIR,
            }
            .get_state_path()
        );
    }

    #[test]
    fn splinter_environment_get_state_path_with_home_without_state_dir_returns_home_plus_data() {
        assert_eq!(
            PathBuf::from(TEST_HOME.to_string() + "/data"),
            SplinterEnvironment {
                state_dir: None,
                home_dir: Some(TEST_HOME.to_string()),
                default_dir: TEST_DEFAULT_DIR,
            }
            .get_state_path()
        );
    }

    #[test]
    fn splinter_environment_get_state_path_with_only_state_dir_returns_state_dir() {
        assert_eq!(
            PathBuf::from(TEST_STATE_DIR),
            SplinterEnvironment {
                state_dir: Some(TEST_STATE_DIR.to_string()),
                home_dir: None,
                default_dir: TEST_DEFAULT_DIR,
            }
            .get_state_path()
        );
    }

    #[test]
    fn splinter_environment_get_state_path_with_nothing_returns_default() {
        assert_eq!(
            PathBuf::from(TEST_DEFAULT_DIR),
            SplinterEnvironment {
                state_dir: None,
                home_dir: None,
                default_dir: TEST_DEFAULT_DIR,
            }
            .get_state_path()
        );
    }
}
