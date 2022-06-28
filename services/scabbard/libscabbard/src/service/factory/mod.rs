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

#[cfg(feature = "rest-api")]
mod endpoint_provider;

use std::collections::HashMap;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use std::collections::HashSet;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use std::convert::TryFrom;
#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
use std::path::Path;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use std::time::Duration;

use cylinder::VerifierFactory;
#[cfg(feature = "diesel")]
use diesel::r2d2::{ConnectionManager, Pool};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use sawtooth::receipt::store::diesel::DieselReceiptStore;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use sawtooth::receipt::store::ReceiptStore;
#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
use splinter::error::InternalError;
use splinter::error::{InvalidArgumentError, InvalidStateError};
#[cfg(feature = "rest-api-actix-web-1")]
use splinter::service::instance::EndpointFactory;
use splinter::service::instance::{
    FactoryCreateError, ServiceArgValidator, ServiceFactory, ServiceInstance,
};
use splinter::service::instance::{OrchestratableService, OrchestratableServiceFactory};
#[cfg(feature = "rest-api")]
use splinter::service::rest_api::ServiceEndpointProvider;
#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
use transact::database::Database;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use transact::state::merkle::sql;

use crate::hex::parse_hex;
#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
use crate::service::ScabbardStatePurgeHandler;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use crate::service::{
    error::ScabbardError,
    state::merkle_state::{self, MerkleState, MerkleStateConfig},
    Scabbard, ScabbardVersion, SERVICE_TYPE,
};
#[cfg(feature = "diesel")]
use crate::store::diesel::DieselCommitHashStore;
#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
use crate::store::transact::factory::{LmdbDatabaseFactory, LmdbDatabasePurgeHandle};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use crate::store::CommitHashStore;

#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
const DEFAULT_LMDB_DIR: &str = "/var/lib/splinter";

/// A connection URI to a database instance.
#[derive(Clone)]
pub enum ConnectionUri {
    /// A Postgres connection URI.
    #[cfg(feature = "postgres")]
    Postgres(Box<str>),
    /// A SQLite connection string.
    #[cfg(feature = "sqlite")]
    Sqlite(Box<str>),
    /// An unknown, unsupported connection URI.
    #[cfg(not(feature = "sqlite"))]
    Unknown(Box<str>),
}

#[cfg(feature = "lmdb")]
#[derive(Default)]
pub struct ScabbardLmdbStateConfiguration {
    db_dir: Option<String>,
    db_size: Option<usize>,
    enable_lmdb: bool,
}

/// Configuration for underlying storage that will be enabled for each service produced by the
/// resulting ScabbardFactory.
#[derive(Clone)]
pub enum ScabbardStorageConfiguration {
    /// Configure scabbard storage via a connection URI.
    ConnectionUri { connection_uri: ConnectionUri },
    /// Configure scabbard storage using a shared Postgres connection pool.
    #[cfg(feature = "postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    },
    /// Configure scabbard storage using a shared SQLite connection pool.
    #[cfg(feature = "sqlite")]
    Sqlite {
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    },
    #[cfg(feature = "sqlite")]
    SqliteExclusiveWrites {
        pool: Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>,
    },
}

impl From<String> for ScabbardStorageConfiguration {
    fn from(connection_uri: String) -> Self {
        connection_uri.as_str().into()
    }
}

impl From<&str> for ScabbardStorageConfiguration {
    fn from(connection_uri: &str) -> Self {
        let connection_uri = match connection_uri {
            #[cfg(feature = "postgres")]
            s if s.starts_with("postgres://") => ConnectionUri::Postgres(s.into()),
            #[cfg(feature = "sqlite")]
            s => ConnectionUri::Sqlite(s.into()),
            #[cfg(not(feature = "sqlite"))]
            s => ConnectionUri::Unknown(s.into()),
        };
        Self::ConnectionUri { connection_uri }
    }
}

#[cfg(feature = "postgres")]
impl From<Pool<ConnectionManager<diesel::pg::PgConnection>>> for ScabbardStorageConfiguration {
    fn from(pool: Pool<ConnectionManager<diesel::pg::PgConnection>>) -> Self {
        Self::Postgres { pool }
    }
}

#[cfg(feature = "sqlite")]
impl From<Pool<ConnectionManager<diesel::SqliteConnection>>> for ScabbardStorageConfiguration {
    fn from(pool: Pool<ConnectionManager<diesel::SqliteConnection>>) -> Self {
        Self::Sqlite { pool }
    }
}

#[cfg(feature = "sqlite")]
impl From<Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>>
    for ScabbardStorageConfiguration
{
    fn from(pool: Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>) -> Self {
        Self::SqliteExclusiveWrites { pool }
    }
}

/// Builds new ScabbardFactory instances.
#[derive(Default)]
pub struct ScabbardFactoryBuilder {
    #[cfg(feature = "lmdb")]
    state_storage_configuration: Option<ScabbardLmdbStateConfiguration>,
    storage_configuration: Option<ScabbardStorageConfiguration>,
    signature_verifier_factory: Option<Arc<Mutex<Box<dyn VerifierFactory>>>>,
    enable_state_autocleanup: Option<bool>,
}

impl ScabbardFactoryBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the services to be constructed using LMDB for storing transaction state.
    #[cfg(feature = "lmdb")]
    pub fn with_lmdb_state_defaults(mut self) -> Self {
        self.state_storage_configuration = Some(ScabbardLmdbStateConfiguration::default());
        self
    }

    /// Sets the state db directory to be used by the resulting factory.
    #[cfg(feature = "lmdb")]
    pub fn with_lmdb_state_db_dir(mut self, state_db_dir: String) -> Self {
        self.state_storage_configuration = self
            .state_storage_configuration
            .take()
            .or_else(|| Some(ScabbardLmdbStateConfiguration::default()))
            .map(|mut config| {
                config.db_dir = Some(state_db_dir);
                config
            });
        self
    }

    /// Sets the state db size to be used by the resulting factory.
    #[cfg(feature = "lmdb")]
    pub fn with_lmdb_state_db_size(mut self, state_db_size: usize) -> Self {
        self.state_storage_configuration = self
            .state_storage_configuration
            .take()
            .or_else(|| Some(ScabbardLmdbStateConfiguration::default()))
            .map(|mut config| {
                config.db_size = Some(state_db_size);
                config
            });
        self
    }

    /// Enables LMDB state storage for services created by the resulting factory.
    ///
    /// While all other service state will be stored in a database, when this is enabled, the
    /// merkle state will be stored in LMDB database files.
    #[cfg(feature = "lmdb")]
    pub fn with_lmdb_state_enabled(mut self, enable: bool) -> Self {
        self.state_storage_configuration = self
            .state_storage_configuration
            .take()
            .or_else(|| Some(ScabbardLmdbStateConfiguration::default()))
            .map(|mut config| {
                config.enable_lmdb = enable;
                config
            });

        self
    }

    pub fn with_state_autocleanup_enabled(mut self, enable: bool) -> Self {
        self.enable_state_autocleanup = Some(enable);
        self
    }

    pub fn with_storage_configuration(
        mut self,
        storage_configuration: ScabbardStorageConfiguration,
    ) -> Self {
        self.storage_configuration = Some(storage_configuration);
        self
    }

    /// Set the signature verifier factory to be used by the resulting factory.  This is a required
    /// value, and omitting it will result in an [splinter::error::InvalidStateError] at build-time.
    pub fn with_signature_verifier_factory(
        mut self,
        signature_verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
    ) -> Self {
        self.signature_verifier_factory = Some(signature_verifier_factory);
        self
    }

    /// Build the final [ScabbardFactory] instance.
    ///
    /// # Errors
    ///
    /// Returns an InvalidStateError if a signature_verifier_factory or a storage_configuration have
    /// not been set.
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    pub fn build(self) -> Result<ScabbardFactory, InvalidStateError> {
        let signature_verifier_factory = self.signature_verifier_factory.ok_or_else(|| {
            splinter::error::InvalidStateError::with_message(
                "A scabbard factory requires a signature verifier factory".into(),
            )
        })?;

        let storage_configuration = self.storage_configuration.ok_or_else(|| {
            InvalidStateError::with_message("A storage configuration must be provided".into())
        })?;

        #[cfg(feature = "lmdb")]
        let state_storage_configuration = self.state_storage_configuration.unwrap_or_default();
        #[cfg(feature = "lmdb")]
        let lmdb_path = Path::new(
            state_storage_configuration
                .db_dir
                .as_deref()
                .unwrap_or(DEFAULT_LMDB_DIR),
        );

        let store_factory_config = match storage_configuration {
            #[cfg(feature = "postgres")]
            ScabbardStorageConfiguration::ConnectionUri {
                connection_uri: ConnectionUri::Postgres(url),
            } => ScabbardFactoryStorageConfig::Postgres {
                pool: get_postgres_pool(&*url)?,
            },
            #[cfg(feature = "sqlite")]
            ScabbardStorageConfiguration::ConnectionUri {
                connection_uri: ConnectionUri::Sqlite(conn_str),
            } => ScabbardFactoryStorageConfig::Sqlite {
                pool: get_sqlite_pool(&*conn_str)?,
            },
            #[cfg(feature = "postgres")]
            ScabbardStorageConfiguration::Postgres { pool } => {
                ScabbardFactoryStorageConfig::Postgres { pool }
            }
            #[cfg(feature = "sqlite")]
            ScabbardStorageConfiguration::Sqlite { pool } => {
                ScabbardFactoryStorageConfig::Sqlite { pool }
            }
            #[cfg(feature = "sqlite")]
            ScabbardStorageConfiguration::SqliteExclusiveWrites { pool } => {
                ScabbardFactoryStorageConfig::SqliteExclusiveWrites { pool }
            }
        };

        #[cfg(feature = "lmdb")]
        if !state_storage_configuration.enable_lmdb {
            check_for_lmdb_files(lmdb_path)?;
        } else {
            check_for_sql_trees(&store_factory_config)?;
        }

        #[cfg(feature = "lmdb")]
        let state_store_factory = LmdbDatabaseFactory::new_state_db_factory(
            lmdb_path,
            state_storage_configuration.db_size,
        );

        let state_autocleanup_enabled = self.enable_state_autocleanup.unwrap_or_default();

        Ok(ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            #[cfg(feature = "lmdb")]
            state_store_factory,
            #[cfg(feature = "lmdb")]
            enable_lmdb_state: state_storage_configuration.enable_lmdb,
            state_autocleanup_enabled,
            store_factory_config,
            signature_verifier_factory,
        })
    }

    /// This builder will fail as it has been configured without any database storage.
    #[cfg(not(any(feature = "postgres", feature = "sqlite")))]
    pub fn build(self) -> Result<ScabbardFactory, InvalidStateError> {
        // this makes clippy happy under these conditions:
        let _signature_verifier_factory = self.signature_verifier_factory.ok_or_else(|| {
            splinter::error::InvalidStateError::with_message(
                "A scabbard factory requires a signature verifier factory".into(),
            )
        })?;

        let _storage_configuration = self.storage_configuration.ok_or_else(|| {
            InvalidStateError::with_message("A storage configuration must be provided".into())
        })?;

        Err(InvalidStateError::with_message(
            "This instance of the ScabbardFactory has not been constructed with any database \
             support. Please compile with either \"postgres\" and/or \"sqlite\" \
             features enabled."
                .into(),
        ))
    }
}

#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
fn check_for_lmdb_files(lmdb_path: &Path) -> Result<(), InvalidStateError> {
    if !lmdb_path.is_dir() {
        return Err(InvalidStateError::with_message(format!(
            "{} is not a directory",
            lmdb_path.display(),
        )));
    }

    match std::fs::read_dir(lmdb_path) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|err| {
                    InvalidStateError::with_message(format!(
                        "Unable to list files in {}: {}",
                        lmdb_path.display(),
                        err
                    ))
                })?;
                if entry
                    .path()
                    .extension()
                    .map(|extension| extension == "lmdb")
                    .unwrap_or(false)
                {
                    return Err(InvalidStateError::with_message(
                        "LMDB database files exist, but LMDB storage is not enabled".into(),
                    ));
                }
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(
            InvalidStateError::with_message(format!("{} is not found", lmdb_path.display())),
        ),
        Err(err) => Err(InvalidStateError::with_message(format!(
            "Unable to read {}: {}",
            lmdb_path.display(),
            err
        ))),
    }
}

#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
fn check_for_sql_trees(
    store_factory_config: &ScabbardFactoryStorageConfig,
) -> Result<(), InvalidStateError> {
    match store_factory_config {
        #[cfg(feature = "postgres")]
        ScabbardFactoryStorageConfig::Postgres { pool } => {
            merkle_state::postgres_list_available_trees(pool)
                .map_err(|e| {
                    InvalidStateError::with_message(format!(
                        "Unable to read merkle state trees in postgres: {}",
                        e
                    ))
                })
                .and_then(|trees| {
                    // Check that if any trees exist, it is only the default tree
                    if trees.iter().any(|name| name != "default") {
                        Err(InvalidStateError::with_message(
                            "SQL Merkle Radix trees exist, but LMDB storage is enabled".into(),
                        ))
                    } else {
                        Ok(())
                    }
                })
        }
        #[cfg(feature = "sqlite")]
        ScabbardFactoryStorageConfig::Sqlite { pool } => {
            merkle_state::sqlite_list_available_trees(pool)
                .map_err(|e| {
                    InvalidStateError::with_message(format!(
                        "Unable to read merkle state trees in sqlite: {}",
                        e
                    ))
                })
                .and_then(|trees| {
                    // Check that if any trees exist, it is only the default tree
                    if trees.iter().any(|name| name != "default") {
                        Err(InvalidStateError::with_message(
                            "SQL Merkle Radix trees exist, but LMDB storage is enabled".into(),
                        ))
                    } else {
                        Ok(())
                    }
                })
        }
        #[cfg(feature = "sqlite")]
        ScabbardFactoryStorageConfig::SqliteExclusiveWrites { pool } => {
            let pool = pool.read().map_err(|_e| {
                InvalidStateError::with_message("RwLock on connection pool is poisoned".into())
            })?;
            merkle_state::sqlite_list_available_trees(&pool)
                .map_err(|e| {
                    InvalidStateError::with_message(format!(
                        "Unable to read merkle state trees in sqlite: {}",
                        e
                    ))
                })
                .and_then(|trees| {
                    // Check that if any trees exist, it is only the default tree
                    if trees.iter().any(|name| name != "default") {
                        Err(InvalidStateError::with_message(
                            "SQL Merkle Radix trees exist, but LMDB storage is enabled".into(),
                        ))
                    } else {
                        Ok(())
                    }
                })
        }
    }
}

/// Internal Factory storage configuration.
#[cfg(any(feature = "postgres", feature = "sqlite"))]
enum ScabbardFactoryStorageConfig {
    #[cfg(feature = "postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    },
    #[cfg(feature = "sqlite")]
    SqliteExclusiveWrites {
        pool: Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>,
    },
}

pub struct ScabbardFactory {
    service_types: Vec<String>,
    #[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
    state_store_factory: LmdbDatabaseFactory,
    #[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
    enable_lmdb_state: bool,
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    store_factory_config: ScabbardFactoryStorageConfig,
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    signature_verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    state_autocleanup_enabled: bool,
}

pub struct ScabbardArgValidator;

impl ServiceArgValidator for ScabbardArgValidator {
    fn validate(&self, args: &HashMap<String, String>) -> Result<(), InvalidArgumentError> {
        let peer_services_str = args
            .get("peer_services")
            .ok_or_else(|| InvalidArgumentError::new("peer_services", "argument not provided"))?;

        let peer_services = parse_list(peer_services_str).map_err(|err| {
            InvalidArgumentError::new("peer_services", format!("failed to parse list: {}", err,))
        })?;

        for service in peer_services {
            if service.is_empty() {
                return Err(InvalidArgumentError::new(
                    "peer_services",
                    "must provide at least one service ID",
                ));
            }
        }

        let admin_keys_str = args
            .get("admin_keys")
            .ok_or_else(|| InvalidArgumentError::new("admin_keys", "argument not provided"))?;

        let admin_keys = parse_list(admin_keys_str).map_err(|err| {
            InvalidArgumentError::new("admin_keys", format!("failed to parse list: {}", err,))
        })?;

        for key in admin_keys {
            if key.is_empty() {
                return Err(InvalidArgumentError::new(
                    "admin_keys",
                    "must provide at least one admin key",
                ));
            }

            let key_bytes = parse_hex(&key).map_err(|_| {
                InvalidArgumentError::new(
                    "admin_keys",
                    format!("{:?} is not a valid hex-formatted public key", key,),
                )
            })?;

            if key_bytes.len() != 33 {
                return Err(InvalidArgumentError::new(
                    "admin_keys",
                    format!("{} is not a valid public key: invalid length", key),
                ));
            }
        }

        Ok(())
    }
}

impl ServiceFactory for ScabbardFactory {
    fn available_service_types(&self) -> &[String] {
        self.service_types.as_slice()
    }

    /// `args` must include the following:
    /// - `admin_keys`: list of public keys that are allowed to create and modify sabre contracts,
    ///   formatted as a serialized JSON array of strings
    /// - `peer_services`: list of other scabbard services on the same circuit that this service
    ///   will share state with
    ///
    /// `args` may include the following optional entries:
    /// - `coordinator_timeout`: the length of time (in milliseconds) that the network has to
    ///   commit a proposal before the coordinator rejects it (if not provided, default is 30
    ///   seconds)
    /// - `version`: the protocol version for scabbard (possible values: "1", "2") (default: "1")
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn create(
        &self,
        service_id: String,
        _service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn ServiceInstance>, FactoryCreateError> {
        Ok(Box::new(
            self.create_scabbard(service_id, circuit_id, args)?,
        ))
    }

    #[cfg(not(any(feature = "postgres", feature = "sqlite")))]
    fn create(
        &self,
        _service_id: String,
        _service_type: &str,
        _circuit_id: &str,
        _args: HashMap<String, String>,
    ) -> Result<Box<dyn ServiceInstance>, FactoryCreateError> {
        // As the factory cannot be created under these conditions, this function is not reachable.
        unreachable!()
    }
}

#[cfg(feature = "rest-api")]
impl EndpointFactory for ScabbardFactory {
    /// The `Scabbard` services created by the `ScabbardFactory` provide the following REST API
    /// endpoints as [`ServiceEndpoint`]s:
    ///
    /// * `POST /batches` - Add one or more batches to scabbard's queue
    /// * `GET /batch_statuses` - Get the status of one or more batches
    /// * `GET /ws/subscribe` - Subscribe to scabbard state-delta events
    /// * `GET /state/{address}` - Get a value from scabbard's state
    /// * `GET /state` - Get multiple scabbard state entries
    /// * `GET /state_root` - Get the current state root hash of scabbard's state
    ///
    /// These endpoints are only available if the following REST API backend feature is enabled:
    ///
    /// * `rest-api-actix`
    ///
    /// [`ServiceEndpoint`]: ../rest_api/struct.ServiceEndpoint.html
    fn get_rest_endpoint_provider(&self) -> Box<dyn ServiceEndpointProvider> {
        Box::new(endpoint_provider::ScabbardServiceEndpointProvider::default())
    }
}

impl OrchestratableServiceFactory for ScabbardFactory {
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn create_orchestratable_service(
        &self,
        service_id: String,
        _service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn OrchestratableService>, FactoryCreateError> {
        Ok(Box::new(
            self.create_scabbard(service_id, circuit_id, args)?,
        ))
    }

    #[cfg(not(any(feature = "postgres", feature = "sqlite")))]
    fn create_orchestratable_service(
        &self,
        _service_id: String,
        _service_type: &str,
        _circuit_id: &str,
        _args: HashMap<String, String>,
    ) -> Result<Box<dyn OrchestratableService>, FactoryCreateError> {
        // As the factory cannot be created under these conditions, this function is not reachable.
        unreachable!()
    }
}

impl ScabbardFactory {
    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn create_scabbard(
        &self,
        service_id: String,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Scabbard, FactoryCreateError> {
        let peer_services_str = args.get("peer_services").ok_or_else(|| {
            FactoryCreateError::InvalidArguments("peer_services argument not provided".into())
        })?;

        let peer_services = parse_list(peer_services_str)
            .map_err(|err| {
                FactoryCreateError::InvalidArguments(format!(
                    "failed to parse peer_services list: {}",
                    err,
                ))
            })?
            .into_iter()
            .collect::<HashSet<String>>();

        let admin_keys_str = args.get("admin_keys").ok_or_else(|| {
            FactoryCreateError::InvalidArguments("admin_keys argument not provided".into())
        })?;

        let admin_keys = parse_list(admin_keys_str).map_err(|err| {
            FactoryCreateError::InvalidArguments(format!(
                "failed to parse admin_keys list: {}",
                err,
            ))
        })?;

        let coordinator_timeout = args
            .get("coordinator_timeout")
            .map(|timeout| match timeout.parse::<u64>() {
                Ok(timeout) => Ok(Duration::from_millis(timeout)),
                Err(err) => Err(FactoryCreateError::InvalidArguments(format!(
                    "invalid coordinator_timeout: {}",
                    err
                ))),
            })
            .transpose()?;
        let version = ScabbardVersion::try_from(args.get("version").map(String::as_str))
            .map_err(FactoryCreateError::InvalidArguments)?;

        #[cfg(feature = "lmdb")]
        let (merkle_state, state_purge): (_, Box<dyn ScabbardStatePurgeHandler>) =
            if self.enable_lmdb_state {
                self.sql_state_check(circuit_id, &service_id)?;

                let db = self
                    .state_store_factory
                    .get_database(circuit_id, &service_id)
                    .map_err(|e| FactoryCreateError::Internal(e.to_string()))?;

                let db_purge_handle = self
                    .state_store_factory
                    .get_database_purge_handle(circuit_id, &service_id)
                    .map_err(|e| FactoryCreateError::Internal(e.to_string()))?;

                let merkle_state = MerkleState::new(MerkleStateConfig::key_value(db.clone_box()))
                    .map_err(|e| FactoryCreateError::Internal(e.to_string()))?;

                (
                    merkle_state,
                    Box::new(LmdbScabbardPurgeHandler { db_purge_handle }),
                )
            } else {
                self.lmdb_state_check(circuit_id, &service_id)?;

                (
                    MerkleState::new(self.create_sql_merkle_state_config(circuit_id, &service_id))
                        .map_err(|e| FactoryCreateError::Internal(e.to_string()))?,
                    self.create_sql_merkle_state_purge_handle(circuit_id, &service_id),
                )
            };

        #[cfg(not(feature = "lmdb"))]
        let (merkle_state, state_purge) = (
            MerkleState::new(self.create_sql_merkle_state_config(circuit_id, &service_id))
                .map_err(|e| FactoryCreateError::Internal(e.to_string()))?,
            self.create_sql_merkle_state_purge_handle(circuit_id, &service_id),
        );

        let (receipt_store, commit_hash_store): (
            Arc<dyn ReceiptStore>,
            Arc<dyn CommitHashStore + Sync + Send>,
        ) = match &self.store_factory_config {
            #[cfg(feature = "postgres")]
            ScabbardFactoryStorageConfig::Postgres { pool } => (
                Arc::new(DieselReceiptStore::new(
                    pool.clone(),
                    Some(format!("{}::{}", circuit_id, service_id)),
                )),
                Arc::new(DieselCommitHashStore::new(
                    pool.clone(),
                    circuit_id,
                    &service_id,
                )),
            ),
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::Sqlite { pool } => (
                Arc::new(DieselReceiptStore::new(
                    pool.clone(),
                    Some(format!("{}::{}", circuit_id, service_id)),
                )),
                Arc::new(DieselCommitHashStore::new(
                    pool.clone(),
                    circuit_id,
                    &service_id,
                )),
            ),
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::SqliteExclusiveWrites { pool } => (
                Arc::new(DieselReceiptStore::new_with_write_exclusivity(
                    pool.clone(),
                    Some(format!("{}::{}", circuit_id, service_id)),
                )),
                Arc::new(DieselCommitHashStore::new_with_write_exclusivity(
                    pool.clone(),
                    circuit_id,
                    &service_id,
                )),
            ),
        };

        Scabbard::new(
            service_id,
            circuit_id,
            version,
            peer_services,
            merkle_state,
            self.state_autocleanup_enabled,
            commit_hash_store,
            receipt_store,
            state_purge,
            self.signature_verifier_factory
                .lock()
                .map_err(|_| {
                    FactoryCreateError::CreationFailed(Box::new(ScabbardError::LockPoisoned))
                })?
                .new_verifier(),
            admin_keys,
            coordinator_timeout,
        )
        .map_err(|err| FactoryCreateError::CreationFailed(Box::new(err)))
    }

    /// Check that the LMDB files doesn't exist for the given service.
    #[cfg(feature = "lmdb")]
    #[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
    fn lmdb_state_check(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Result<(), FactoryCreateError> {
        let path = self
            .state_store_factory
            .compute_path(circuit_id, service_id)
            .map_err(|e| FactoryCreateError::Internal(e.to_string()))?;
        if path.with_extension("lmdb").exists() {
            return Err(InvalidStateError::with_message(format!(
                "LMDB database files exist for {}::{}, but LMDB storage is not enabled",
                circuit_id, service_id
            ))
            .into());
        }
        Ok(())
    }

    /// Check that the SQL state doesn't exist for the given service.
    #[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
    fn sql_state_check(&self, circuit_id: &str, service_id: &str) -> Result<(), InvalidStateError> {
        let exists = MerkleState::check_existence(
            &self.create_sql_merkle_state_config(circuit_id, service_id),
        );

        if exists {
            return Err(InvalidStateError::with_message(format!(
                "A SQL-based merkle tree exists for {}::{}, but database storage is not enabled",
                circuit_id, service_id
            )));
        }

        Ok(())
    }

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn create_sql_merkle_state_config(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> MerkleStateConfig {
        match &self.store_factory_config {
            #[cfg(feature = "postgres")]
            ScabbardFactoryStorageConfig::Postgres { pool } => MerkleStateConfig::Postgres {
                pool: pool.clone(),
                tree_name: format!("{}::{}", circuit_id, service_id),
            },
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::Sqlite { pool } => MerkleStateConfig::Sqlite {
                pool: pool.clone(),
                tree_name: format!("{}::{}", circuit_id, service_id),
            },
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::SqliteExclusiveWrites { pool } => {
                MerkleStateConfig::SqliteExclusiveWrites {
                    pool: pool.clone(),
                    tree_name: format!("{}::{}", circuit_id, service_id),
                }
            }
        }
    }

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn create_sql_merkle_state_purge_handle(
        &self,
        circuit_id: &str,
        service_id: &str,
    ) -> Box<dyn ScabbardStatePurgeHandler> {
        match &self.store_factory_config {
            #[cfg(feature = "postgres")]
            ScabbardFactoryStorageConfig::Postgres { pool } => {
                Box::new(PostgresMerkleStatePurgeHandler {
                    pool: pool.clone(),
                    tree_name: format!("{}::{}", circuit_id, service_id),
                })
            }
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::Sqlite { pool } => {
                Box::new(SqliteMerkleStatePurgeHandler {
                    pool: Arc::new(RwLock::new(pool.clone())),
                    tree_name: format!("{}::{}", circuit_id, service_id),
                })
            }
            #[cfg(feature = "sqlite")]
            ScabbardFactoryStorageConfig::SqliteExclusiveWrites { pool } => {
                Box::new(SqliteMerkleStatePurgeHandler {
                    pool: pool.clone(),
                    tree_name: format!("{}::{}", circuit_id, service_id),
                })
            }
        }
    }
}

/// Parse a service argument into a list. Check if the argument is in json or csv format
/// and return the list of strings. An error is returned if json fmt cannot be parsed.
fn parse_list(values_list: &str) -> Result<Vec<String>, String> {
    if values_list.starts_with('[') {
        serde_json::from_str(values_list).map_err(|err| err.to_string())
    } else {
        Ok(values_list
            .split(',')
            .map(String::from)
            .collect::<Vec<String>>())
    }
}

#[cfg(feature = "postgres")]
fn get_postgres_pool(
    url: &str,
) -> Result<Pool<ConnectionManager<diesel::pg::PgConnection>>, InvalidStateError> {
    let connection_manager = ConnectionManager::<diesel::pg::PgConnection>::new(url);
    Pool::builder().build(connection_manager).map_err(|err| {
        InvalidStateError::with_message(format!("Failed to build connection pool: {}", err))
    })
}

#[cfg(feature = "sqlite")]
fn get_sqlite_pool(
    conn_str: &str,
) -> Result<Pool<ConnectionManager<diesel::SqliteConnection>>, InvalidStateError> {
    if (&*conn_str != ":memory:") && !Path::new(&*conn_str).exists() {
        return Err(InvalidStateError::with_message(format!(
            "Database file '{}' does not exist",
            conn_str
        )));
    }
    let connection_manager = ConnectionManager::<diesel::sqlite::SqliteConnection>::new(&*conn_str);
    let mut pool_builder = Pool::builder();
    // A new database is created for each connection to the in-memory SQLite
    // implementation; to ensure that the resulting stores will operate on the same
    // database, only one connection is allowed.
    if &*conn_str == ":memory:" {
        pool_builder = pool_builder.max_size(1);
    }
    pool_builder.build(connection_manager).map_err(|err| {
        InvalidStateError::with_message(format!("Failed to build connection pool: {}", err))
    })
}

#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
struct LmdbScabbardPurgeHandler {
    db_purge_handle: LmdbDatabasePurgeHandle,
}

#[cfg(all(feature = "lmdb", any(feature = "postgres", feature = "sqlite")))]
impl ScabbardStatePurgeHandler for LmdbScabbardPurgeHandler {
    fn purge_state(&self) -> Result<(), InternalError> {
        self.db_purge_handle.purge()
    }
}

#[cfg(feature = "postgres")]
struct PostgresMerkleStatePurgeHandler {
    pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    tree_name: String,
}

#[cfg(feature = "postgres")]
impl ScabbardStatePurgeHandler for PostgresMerkleStatePurgeHandler {
    fn purge_state(&self) -> Result<(), InternalError> {
        let postgres_backend = sql::backend::PostgresBackend::from(self.pool.clone());

        let state = sql::SqlMerkleStateBuilder::new()
            .with_backend(postgres_backend)
            .with_tree(self.tree_name.clone())
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        state
            .delete_tree()
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

#[cfg(feature = "sqlite")]
struct SqliteMerkleStatePurgeHandler {
    pool: Arc<RwLock<Pool<ConnectionManager<diesel::SqliteConnection>>>>,
    tree_name: String,
}

#[cfg(feature = "sqlite")]
impl ScabbardStatePurgeHandler for SqliteMerkleStatePurgeHandler {
    fn purge_state(&self) -> Result<(), InternalError> {
        let sqlite_backend = sql::backend::SqliteBackend::from(self.pool.clone());

        let state = sql::SqlMerkleStateBuilder::new()
            .with_backend(sqlite_backend)
            .with_tree(self.tree_name.clone())
            .build()
            .map_err(|e| InternalError::from_source(Box::new(e)))?;

        state
            .delete_tree()
            .map_err(|e| InternalError::from_source(Box::new(e)))
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use transact::state::merkle::sql::{backend::SqliteBackend, migration::MigrationManager};

    /// Verify that the scabbard factory produces a valid `Scabbard` instance.
    #[test]
    fn create_successful() {
        let factory = get_factory();

        let service = factory
            .create("0".into(), "", "1", get_mock_args())
            .expect("failed to create service");
        assert_eq!(service.service_id(), "0");

        let scabbard = (&*service)
            .as_any()
            .downcast_ref::<Scabbard>()
            .expect("failed to downcast Service to Scabbard");
        assert_eq!(&scabbard.service_id, "0");
        assert_eq!(&scabbard.circuit_id, "1");
    }

    /// Verify that the scabbard factory produces a valid `Scabbard` instance if the service
    /// arguments are commo seperated instead of json fmt.
    #[test]
    fn create_successful_no_json() {
        let factory = get_factory();

        let service = factory
            .create("2".into(), "", "1", get_mock_args_no_json())
            .expect("failed to create service");
        assert_eq!(service.service_id(), "2");

        let scabbard = (&*service)
            .as_any()
            .downcast_ref::<Scabbard>()
            .expect("failed to downcast Service to Scabbard");
        assert_eq!(&scabbard.service_id, "2");
        assert_eq!(&scabbard.circuit_id, "1");
    }

    /// Verify that the `coordinator_timeout` service argument is properly set for a new `Scabbard`
    /// instance.
    #[test]
    fn create_with_coordinator_timeout() {
        let factory = get_factory();
        let mut args = get_mock_args();
        args.insert("coordinator_timeout".into(), "123".into());

        let service = factory
            .create("".into(), "", "", args)
            .expect("failed to create service");
        let scabbard = (&*service)
            .as_any()
            .downcast_ref::<Scabbard>()
            .expect("failed to downcast Service to Scabbard");

        assert_eq!(scabbard.coordinator_timeout, Duration::from_millis(123));
    }

    /// Verify that `Scabbard` creation fails when the `peer_services` argument isn't specified.
    #[test]
    fn create_without_peer_services() {
        let factory = get_factory();
        let mut args = get_mock_args();
        args.remove("peer_services");

        assert!(
            factory.create("".into(), "", "", args).is_err(),
            "Creating factory without peer_services did not fail"
        );
    }

    /// Verify that `Scabbard` creation fails when the `admin_keys` argument isn't specified.
    #[test]
    fn create_without_admin_keys() {
        let factory = get_factory();
        let mut args = get_mock_args();
        args.remove("admin_keys");

        assert!(
            factory.create("".into(), "", "", args).is_err(),
            "Creating factory without admin_keys did not fail"
        );
    }

    /// Verify arg validation returns ok with valid common seperated Args
    #[test]
    fn test_valid_argument_validation_no_json() {
        let validator = ScabbardArgValidator;
        let args = get_mock_args_no_json();
        assert!(validator.validate(&args).is_ok());
    }

    /// Verify arg validation returns ok with valid Args using json
    #[test]
    fn test_valid_argument_validation() {
        let validator = ScabbardArgValidator;
        let args = get_mock_args();
        assert!(validator.validate(&args).is_ok());
    }

    /// Verify arg validation returns an error if the args are empty
    #[test]
    fn test_no_argument_validation() {
        let validator = ScabbardArgValidator;
        let args = HashMap::new();
        assert!(validator.validate(&args).is_err());
    }

    /// Verify arg validation returns an error if the args are present but the values are empty
    #[test]
    fn test_empty_argument_validation() {
        let validator = ScabbardArgValidator;
        let mut args = HashMap::new();
        args.insert("peer_services".into(), "".into());
        args.insert("admin_keys".into(), "".into());
        assert!(validator.validate(&args).is_err());
    }

    fn get_factory() -> ScabbardFactory {
        let connection_manager = ConnectionManager::<diesel::SqliteConnection>::new(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        sawtooth::migrations::run_sqlite_migrations(
            &*pool.get().expect("Failed to get connection for migrations"),
        )
        .expect("Failed to run migrations");
        crate::migrations::run_sqlite_migrations(
            &*pool.get().expect("Failed to get connection for migrations"),
        )
        .expect("Failed to run migrations");
        let backend = SqliteBackend::from(pool.clone());
        backend
            .run_migrations()
            .expect("Failed to run transact migrations");

        let store_factory_config = ScabbardFactoryStorageConfig::Sqlite { pool };
        ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            state_store_factory: LmdbDatabaseFactory::new_state_db_factory(
                &Path::new("/tmp"),
                None,
            ),
            enable_lmdb_state: false,
            state_autocleanup_enabled: false,
            store_factory_config,
            signature_verifier_factory: Arc::new(Mutex::new(Box::new(Secp256k1Context::new()))),
        }
    }

    fn get_mock_args() -> HashMap<String, String> {
        let peer_services = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let admin_keys = vec![get_public_key(), get_public_key()];
        let mut args = HashMap::new();
        args.insert(
            "peer_services".into(),
            serde_json::to_string(&peer_services).expect("failed to serialize peer_services"),
        );
        args.insert(
            "admin_keys".into(),
            serde_json::to_string(&admin_keys).expect("failed to serialize admin_keys"),
        );
        args
    }

    fn get_mock_args_no_json() -> HashMap<String, String> {
        let mut args = HashMap::new();
        args.insert("peer_services".into(), "0,1,3".into());
        args.insert(
            "admin_keys".into(),
            format!("{},{}", get_public_key(), get_public_key()),
        );
        args
    }

    fn get_public_key() -> String {
        let context = Secp256k1Context::new();
        let private_key = context.new_random_private_key();
        let public_key = context.get_public_key(&private_key).unwrap();
        public_key.as_hex()
    }
}
