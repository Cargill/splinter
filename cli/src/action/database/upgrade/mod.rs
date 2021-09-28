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

//! Provides database upgrade functionality

#[cfg(feature = "node-id-upgrade")]
mod node_id;
mod yaml;

use std::path::PathBuf;
use std::str::FromStr;

use clap::ArgMatches;
use splinter::store::ConnectionUri;

#[cfg(feature = "sqlite")]
use crate::action::database::sqlite::{get_database_at_state_path, get_default_database};
use crate::action::database::SplinterEnvironment;
use crate::diesel::{pg::PgConnection, Connection};
use crate::error::CliError;

use super::Action;

/// The overarching Action possibly containing multiple upgrade actions
pub struct UpgradeAction;

impl Action for UpgradeAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let state_dir = get_state_dir(arg_matches)?;
        let database_uri = get_database_uri(arg_matches)?;
        let store_factory = splinter::store::create_store_factory(database_uri).map_err(|err| {
            CliError::ActionError(format!("failed to initialized store factory: {}", err))
        })?;
        info!("Upgrading splinterd state");

        #[cfg(feature = "node-id-upgrade")]
        {
            let db_store = store_factory.get_node_id_store();
            node_id::migrate_node_id_to_db(state_dir.clone(), db_store)?;
        }
        info!(
            "Source yaml state directory: {}",
            state_dir.to_string_lossy()
        );
        let database_uri = get_database_uri(arg_matches)?;
        info!("Destination database uri: {}", database_uri);
        info!("Loading YAML datastore... ");
        let db_store = store_factory.get_admin_service_store();
        yaml::import_yaml_state_to_database(state_dir, &*db_store)?;
        Ok(())
    }
}

/// Gets the path of splinterd's state directory
///
///
/// # Arguments
///
/// * `arg_matches` - an option of clap ['ArgMatches'](https://docs.rs/clap/2.33.3/clap/struct.ArgMatches.html).
///
/// # Returns
///
/// * PathBuf to state_dir if present in arg_matches, otherwise just the default from
/// SplinterEnvironment
fn get_state_dir(arg_matches: Option<&ArgMatches>) -> Result<PathBuf, CliError> {
    if let Some(arg_matches) = arg_matches {
        match arg_matches.value_of("state_dir") {
            Some(state_dir) => {
                let state_dir = PathBuf::from(state_dir.to_string());
                Ok(
                    std::fs::canonicalize(state_dir.as_path())
                        .unwrap_or_else(|_| state_dir.clone()),
                )
            }
            None => Ok(SplinterEnvironment::load().get_state_path()),
        }
    } else {
        Ok(SplinterEnvironment::load().get_state_path())
    }
}

/// Gets the configured database_uri
///
///
/// # Arguments
///
/// * `arg_matches` - an option of clap ['ArgMatches'](https://docs.rs/clap/2.33.3/clap/struct.ArgMatches.html).
fn get_database_uri(arg_matches: Option<&ArgMatches>) -> Result<ConnectionUri, CliError> {
    let database_uri = if let Some(arg_matches) = arg_matches {
        match arg_matches.value_of("connect") {
            Some(database_uri) => database_uri.to_string(),
            #[cfg(feature = "sqlite")]
            None => get_database_at_state_path(get_state_dir(Some(arg_matches))?)?,
            #[cfg(not(feature = "sqlite"))]
            None => get_default_database(),
        }
    } else if cfg!(feature = "sqlite") {
        get_database_at_state_path(get_state_dir(arg_matches)?)?
    } else {
        get_default_database()?
    };
    let parsed_uri = ConnectionUri::from_str(&database_uri)
        .map_err(|e| CliError::ActionError(format!("database uri could not be parsed: {}", e)))?;
    if let ConnectionUri::Postgres(_) = parsed_uri {
        // Verify database connection.
        // If the connection is faulty, we want to abort here instead of
        // creating the store, as the store would perform reconnection attempts.
        PgConnection::establish(&database_uri[..]).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to establish database connection to '{}': {}",
                database_uri, err
            ))
        })?;
    }
    Ok(parsed_uri)
}
