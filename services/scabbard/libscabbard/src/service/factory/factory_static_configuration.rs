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

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use cylinder::VerifierFactory;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use diesel::r2d2::{ConnectionManager, Pool};
use openssl::hash::{hash, MessageDigest};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
use sawtooth::receipt::store::diesel::DieselReceiptStore;
use sawtooth::receipt::store::{lmdb::LmdbReceiptStore, ReceiptStore};
use splinter::error::InvalidArgumentError;
use splinter::error::{InternalError, InvalidStateError};
use splinter::service::validation::ServiceArgValidator;
use splinter::service::{FactoryCreateError, Service, ServiceFactory};

use crate::hex::parse_hex;
use crate::hex::to_hex;
#[cfg(feature = "rest-api-actix")]
use crate::service::rest_api::actix;
use crate::service::{
    error::ScabbardError, Scabbard, ScabbardStatePurgeHandler, ScabbardVersion, SERVICE_TYPE,
};

const DEFAULT_STATE_DB_DIR: &str = "/var/lib/splinter";
// Linux, with a 64bit CPU supports sparse files of a large size
#[cfg(target_os = "linux")]
const DEFAULT_STATE_DB_SIZE: usize = 1 << 40; // 1024 ** 4
#[cfg(any(target_arch = "x86", target_arch = "arm", not(target_os = "linux")))]
const DEFAULT_STATE_DB_SIZE: usize = 1 << 30; // 1024 ** 3
const DEFAULT_RECEIPT_DB_DIR: &str = "/var/lib/splinter";
#[cfg(target_os = "linux")]
const DEFAULT_RECEIPT_DB_SIZE: usize = 1 << 40; // 1024 ** 4
#[cfg(any(target_arch = "x86", target_arch = "arm", not(target_os = "linux")))]
const DEFAULT_RECEIPT_DB_SIZE: usize = 1 << 30; // 1024 ** 3

type ScabbardReceiptStore = Arc<RwLock<dyn ReceiptStore>>;

pub enum ConnectionUri {
    #[cfg(feature = "postgres")]
    Postgres(Box<str>),
    #[cfg(feature = "sqlite")]
    Sqlite(Box<str>),
    #[cfg(not(feature = "sqlite"))]
    Unknown(Box<str>),
}

/// Configuration for underlying storage that will be enabled for each service produced by the
/// resulting ScabbardFactory.
pub enum ScabbardStorageConfiguration {
    Lmdb {
        db_dir: Option<String>,
        db_size: Option<usize>,
    },
    DatabaseConnectionUri {
        connection_uri: ConnectionUri,
    },
    #[cfg(feature = "postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    },
}

impl ScabbardStorageConfiguration {
    fn lmdb() -> Self {
        ScabbardStorageConfiguration::Lmdb {
            db_dir: None,
            db_size: None,
        }
    }

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    fn connection_uri(connection_uri: &str) -> Self {
        let connection_uri = match connection_uri {
            #[cfg(feature = "postgres")]
            s if s.starts_with("postgres://") => ConnectionUri::Postgres(s.into()),
            #[cfg(feature = "sqlite")]
            s => ConnectionUri::Sqlite(s.into()),
            #[cfg(not(feature = "sqlite"))]
            s => ConnectionUri::Unknown(s.into()),
        };
        ScabbardStorageConfiguration::DatabaseConnectionUri { connection_uri }
    }

    #[cfg(feature = "postgres")]
    fn with_postgres_connection_pool(
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    ) -> Self {
        Self::Postgres { pool }
    }

    #[cfg(feature = "sqlite")]
    fn with_sqlite_connection_pool(
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    ) -> Self {
        Self::Sqlite { pool }
    }

    fn with_db_dir(self, db_dir: String) -> Self {
        match self {
            Self::Lmdb { db_size, .. } => Self::Lmdb {
                db_dir: Some(db_dir),
                db_size,
            },
            _ => Self::Lmdb {
                db_dir: Some(db_dir),
                db_size: None,
            },
        }
    }

    fn with_db_size(self, db_size: usize) -> Self {
        match self {
            Self::Lmdb { db_dir, .. } => Self::Lmdb {
                db_dir,
                db_size: Some(db_size),
            },
            _ => Self::Lmdb {
                db_dir: None,
                db_size: Some(db_size),
            },
        }
    }
}

/// Builds new ScabbardFactory instances.
#[derive(Default)]
pub struct ScabbardFactoryBuilder {
    state_storage_configuration: Option<ScabbardStorageConfiguration>,
    receipt_storage_configuration: Option<ScabbardStorageConfiguration>,
    signature_verifier_factory: Option<Arc<Mutex<Box<dyn VerifierFactory>>>>,
}

impl ScabbardFactoryBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the state db directory to be used by the resulting factory.
    pub fn with_state_db_dir(mut self, state_db_dir: String) -> Self {
        self.state_storage_configuration = self
            .state_storage_configuration
            .or_else(|| Some(ScabbardStorageConfiguration::lmdb()))
            .map(|config| config.with_db_dir(state_db_dir));
        self
    }

    /// Sets the state db size to be used by the resulting factory.
    pub fn with_state_db_size(mut self, state_db_size: usize) -> Self {
        self.state_storage_configuration = self
            .state_storage_configuration
            .or_else(|| Some(ScabbardStorageConfiguration::lmdb()))
            .map(|config| config.with_db_size(state_db_size));
        self
    }

    /// Sets the receipt db directory to be used by the resulting factory.
    pub fn with_receipt_db_dir(mut self, receipt_db_dir: String) -> Self {
        self.receipt_storage_configuration = self
            .receipt_storage_configuration
            .or_else(|| Some(ScabbardStorageConfiguration::lmdb()))
            .map(|config| config.with_db_dir(receipt_db_dir));
        self
    }

    /// Sets the receipt db size to be used by the resulting factory.
    pub fn with_receipt_db_size(mut self, receipt_db_size: usize) -> Self {
        self.receipt_storage_configuration = self
            .receipt_storage_configuration
            .or_else(|| Some(ScabbardStorageConfiguration::lmdb()))
            .map(|config| config.with_db_size(receipt_db_size));
        self
    }

    #[cfg(any(feature = "postgres", feature = "sqlite"))]
    /// Sets the receipt db connection url to be used by the resulting factory.
    pub fn with_receipt_db_url(mut self, receipt_db_url: String) -> Self {
        self.receipt_storage_configuration = Some(ScabbardStorageConfiguration::connection_uri(
            &receipt_db_url,
        ));
        self
    }

    #[cfg(feature = "postgres")]
    /// Sets the receipt db connection pool to be used by the resulting factory with a Postgres
    /// connection pool.
    pub fn with_receipt_postgres_connection_pool(
        mut self,
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    ) -> Self {
        self.receipt_storage_configuration = Some(
            ScabbardStorageConfiguration::with_postgres_connection_pool(pool),
        );
        self
    }

    #[cfg(feature = "sqlite")]
    /// Sets the receipt db connection pool to be used by the resulting factory with a SQLite
    /// connection pool.
    pub fn with_receipt_sqlite_connection_pool(
        mut self,
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    ) -> Self {
        self.receipt_storage_configuration = Some(
            ScabbardStorageConfiguration::with_sqlite_connection_pool(pool),
        );
        self
    }

    pub fn with_receipt_storage_configuration(
        mut self,
        storage_config: ScabbardStorageConfiguration,
    ) -> Self {
        self.receipt_storage_configuration = Some(storage_config);
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
    /// Returns an InvalidStateError if a signature_verifier_factory has not been set.
    pub fn build(self) -> Result<ScabbardFactory, InvalidStateError> {
        let signature_verifier_factory = self.signature_verifier_factory.ok_or_else(|| {
            splinter::error::InvalidStateError::with_message(
                "A scabbard factory requires a signature verifier factory".into(),
            )
        })?;

        let receipt_storage_configuration = self
            .receipt_storage_configuration
            .unwrap_or_else(ScabbardStorageConfiguration::lmdb);

        let receipt_store_factory_config = match receipt_storage_configuration {
            ScabbardStorageConfiguration::Lmdb { db_dir, db_size } => {
                ScabbardFactoryStorageConfig::Lmdb {
                    db_dir: db_dir.unwrap_or_else(|| DEFAULT_RECEIPT_DB_DIR.into()),
                    db_size: db_size.unwrap_or(DEFAULT_RECEIPT_DB_SIZE),
                }
            }
            #[cfg(feature = "postgres")]
            ScabbardStorageConfiguration::Postgres { pool } => {
                ScabbardFactoryStorageConfig::Postgres { pool }
            }
            #[cfg(feature = "sqlite")]
            ScabbardStorageConfiguration::Sqlite { pool } => {
                ScabbardFactoryStorageConfig::Sqlite { pool }
            }
            ScabbardStorageConfiguration::DatabaseConnectionUri { connection_uri } => {
                match connection_uri {
                    #[cfg(feature = "postgres")]
                    ConnectionUri::Postgres(url) => {
                        let connection_manager =
                            ConnectionManager::<diesel::pg::PgConnection>::new(&*url);
                        let pool = Pool::builder().build(connection_manager).map_err(|err| {
                            InvalidStateError::with_message(format!(
                                "Failed to build connection pool: {}",
                                err
                            ))
                        })?;
                        ScabbardFactoryStorageConfig::Postgres { pool }
                    }
                    #[cfg(feature = "sqlite")]
                    ConnectionUri::Sqlite(conn_str) => {
                        if (&*conn_str != ":memory:") && !std::path::Path::new(&*conn_str).exists()
                        {
                            return Err(InvalidStateError::with_message(format!(
                                "Database file '{}' does not exist",
                                conn_str
                            )));
                        }
                        let connection_manager =
                            ConnectionManager::<diesel::sqlite::SqliteConnection>::new(&*conn_str);
                        let mut pool_builder = Pool::builder();
                        // A new database is created for each connection to the in-memory SQLite
                        // implementation; to ensure that the resulting stores will operate on the same
                        // database, only one connection is allowed.
                        if &*conn_str == ":memory:" {
                            pool_builder = pool_builder.max_size(1);
                        }
                        let pool = pool_builder.build(connection_manager).map_err(|err| {
                            InvalidStateError::with_message(format!(
                                "Failed to build connection pool: {}",
                                err
                            ))
                        })?;
                        ScabbardFactoryStorageConfig::Sqlite { pool }
                    }
                    #[cfg(not(feature = "sqlite"))]
                    ConnectionUri::Unknown(conn_str) => {
                        return Err(InvalidStateError::with_message(format!(
                            "Unrecognizable database connection URI {}",
                            conn_str
                        )));
                    }
                }
            }
        };

        let state_storage_configuration = self
            .state_storage_configuration
            .unwrap_or_else(ScabbardStorageConfiguration::lmdb);
        let (state_db_dir, state_db_size) = match state_storage_configuration {
            ScabbardStorageConfiguration::Lmdb { db_dir, db_size } => (
                db_dir.unwrap_or_else(|| DEFAULT_STATE_DB_DIR.into()),
                db_size.unwrap_or(DEFAULT_STATE_DB_SIZE),
            ),
            _ => unreachable!(),
        };

        Ok(ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            state_db_dir,
            state_db_size,
            receipt_store_factory_config,
            signature_verifier_factory,
        })
    }
}

/// Internal Factory storage configuration.
enum ScabbardFactoryStorageConfig {
    Lmdb {
        db_dir: String,
        db_size: usize,
    },
    #[cfg(feature = "postgres")]
    Postgres {
        pool: Pool<ConnectionManager<diesel::pg::PgConnection>>,
    },
    #[cfg(feature = "sqlite")]
    Sqlite {
        pool: Pool<ConnectionManager<diesel::SqliteConnection>>,
    },
}

pub struct ScabbardFactory {
    service_types: Vec<String>,
    state_db_dir: String,
    state_db_size: usize,
    receipt_store_factory_config: ScabbardFactoryStorageConfig,
    signature_verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
}

pub struct ScabbardArgValidator;

impl ServiceArgValidator for ScabbardArgValidator {
    fn validate(&self, args: &HashMap<String, String>) -> Result<(), InvalidArgumentError> {
        let peer_services_str = args.get("peer_services").ok_or_else(|| {
            InvalidArgumentError::new("peer_services".into(), "argument not provided".into())
        })?;

        let peer_services = parse_list(peer_services_str).map_err(|err| {
            InvalidArgumentError::new(
                "peer_services".into(),
                format!("failed to parse list: {}", err,),
            )
        })?;

        for service in peer_services {
            if service.is_empty() {
                return Err(InvalidArgumentError::new(
                    "peer_services".into(),
                    "must provide at least one service ID".into(),
                ));
            }
        }

        let admin_keys_str = args.get("admin_keys").ok_or_else(|| {
            InvalidArgumentError::new("admin_keys".into(), "argument not provided".into())
        })?;

        let admin_keys = parse_list(admin_keys_str).map_err(|err| {
            InvalidArgumentError::new(
                "admin_keys".into(),
                format!("failed to parse list: {}", err,),
            )
        })?;

        for key in admin_keys {
            if key.is_empty() {
                return Err(InvalidArgumentError::new(
                    "admin_keys".into(),
                    "must provide at least one admin key".into(),
                ));
            }

            let key_bytes = parse_hex(&key).map_err(|_| {
                InvalidArgumentError::new(
                    "admin_keys".into(),
                    format!("{:?} is not a valid hex-formatted public key", key,),
                )
            })?;

            if key_bytes.len() != 33 {
                return Err(InvalidArgumentError::new(
                    "admin_keys".into(),
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
    fn create(
        &self,
        service_id: String,
        _service_type: &str,
        circuit_id: &str,
        args: HashMap<String, String>,
    ) -> Result<Box<dyn Service>, FactoryCreateError> {
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

        let state_db_dir = Path::new(&self.state_db_dir);
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

        let state_db_path = compute_db_path(&service_id, circuit_id, state_db_dir, "state")?;

        let (receipt_store, receipt_path_opt): (ScabbardReceiptStore, _) =
            match &self.receipt_store_factory_config {
                ScabbardFactoryStorageConfig::Lmdb { db_dir, db_size } => {
                    let receipt_db_dir_path = Path::new(&db_dir);

                    let receipt_db_path =
                        compute_db_path(&service_id, circuit_id, receipt_db_dir_path, "receipts")?;

                    let file: String = receipt_db_path
                        .with_extension("lmdb")
                        .components()
                        .rev()
                        .next()
                        .map(|p| {
                            let p: &std::path::Path = p.as_ref();
                            p.to_str().map(|s| s.to_string())
                        })
                        .flatten()
                        .ok_or_else(|| {
                            FactoryCreateError::Internal(
                                "File name produced was not valid UTF-8".into(),
                            )
                        })?;

                    (
                        Arc::new(RwLock::new(
                            LmdbReceiptStore::new(
                                receipt_db_dir_path,
                                &[file.clone()],
                                file,
                                Some(*db_size),
                            )
                            .map_err(|e| FactoryCreateError::Internal(e.to_string()))?,
                        )),
                        Some(receipt_db_path),
                    )
                }
                #[cfg(feature = "postgres")]
                ScabbardFactoryStorageConfig::Postgres { pool } => (
                    Arc::new(RwLock::new(DieselReceiptStore::new(
                        pool.clone(),
                        Some(format!("{}::{}", circuit_id, service_id)),
                    ))),
                    None,
                ),
                #[cfg(feature = "sqlite")]
                ScabbardFactoryStorageConfig::Sqlite { pool } => (
                    Arc::new(RwLock::new(DieselReceiptStore::new(
                        pool.clone(),
                        Some(format!("{}::{}", circuit_id, service_id)),
                    ))),
                    None,
                ),
            };

        let state_purge_handle = Box::new(LmdbScabbardStatePurgeHandlerHandler {
            state_lmdb_path: state_db_path.clone(),
            receipt_lmdb_path: receipt_path_opt,
        });

        let service = Scabbard::new(
            service_id,
            circuit_id,
            version,
            peer_services,
            &state_db_path,
            self.state_db_size,
            receipt_store,
            state_purge_handle,
            self.signature_verifier_factory
                .lock()
                .map_err(|_| {
                    FactoryCreateError::CreationFailed(Box::new(ScabbardError::LockPoisoned))
                })?
                .new_verifier(),
            admin_keys,
            coordinator_timeout,
        )
        .map_err(|err| FactoryCreateError::CreationFailed(Box::new(err)))?;

        Ok(Box::new(service))
    }

    #[cfg(feature = "rest-api")]
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
    fn get_rest_endpoints(&self) -> Vec<splinter::service::rest_api::ServiceEndpoint> {
        // Allowing unused_mut because resources must be mutable if feature rest-api-actix is
        // enabled
        #[allow(unused_mut)]
        let mut endpoints = vec![];

        #[cfg(feature = "rest-api-actix")]
        {
            endpoints.append(&mut vec![
                actix::batches::make_add_batches_to_queue_endpoint(),
                actix::ws_subscribe::make_subscribe_endpoint(),
                actix::batch_statuses::make_get_batch_status_endpoint(),
                actix::state_address::make_get_state_at_address_endpoint(),
                actix::state::make_get_state_with_prefix_endpoint(),
                actix::state_root::make_get_state_root_endpoint(),
            ])
        }

        endpoints
    }
}

struct LmdbScabbardStatePurgeHandlerHandler {
    state_lmdb_path: PathBuf,
    receipt_lmdb_path: Option<PathBuf>,
}

impl ScabbardStatePurgeHandler for LmdbScabbardStatePurgeHandlerHandler {
    fn purge_state(&self) -> Result<(), InternalError> {
        purge_paths(&self.state_lmdb_path)?;
        if let Some(receipt_lmdb_path) = self.receipt_lmdb_path.as_deref() {
            purge_paths(receipt_lmdb_path)?;
        }
        Ok(())
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

/// Compute the LMDB file path for a circuit::service-id pair.
pub fn compute_db_path(
    service_id: &str,
    circuit_id: &str,
    db_dir: &Path,
    suffix: &str,
) -> Result<PathBuf, FactoryCreateError> {
    let hash = hash(
        MessageDigest::sha256(),
        format!("{}::{}", service_id, circuit_id).as_bytes(),
    )
    .map(|digest| to_hex(&*digest))
    .map_err(|err| FactoryCreateError::CreationFailed(Box::new(err)))?;
    let db_path = db_dir.join(format!("{}-{}", hash, suffix));
    Ok(db_path)
}

fn purge_paths(lmdb_path: &Path) -> Result<(), InternalError> {
    let db_path = lmdb_path.with_extension("lmdb");
    let db_lock_file_path = lmdb_path.with_extension("lmdb-lock");

    std::fs::remove_file(db_path.as_path())
        .map_err(|err| InternalError::from_source(Box::new(err)))?;
    std::fs::remove_file(db_lock_file_path.as_path())
        .map_err(|err| InternalError::from_source(Box::new(err)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use tempdir::TempDir;

    /// Verify that the scabbard factory produces a valid `Scabbard` instance.
    #[test]
    fn create_successful() {
        let (_tempdir, factory) = get_factory();

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
        let (_tempdir, factory) = get_factory();

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
        let (_tempdir, factory) = get_factory();
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
        let (_tempdir, factory) = get_factory();
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
        let (_tempdir, factory) = get_factory();
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

    fn get_factory() -> (TempDir, ScabbardFactory) {
        let tempdir = TempDir::new("scabbard_factory").expect("Unable to create new tempdir");
        #[cfg(not(feature = "sqlite"))]
        let receipt_store_factory_config = ScabbardFactoryStorageConfig::Lmdb {
            db_dir: tempdir.path().to_string_lossy().into(),
            db_size: 1024 * 1024,
        };
        #[cfg(feature = "sqlite")]
        let receipt_store_factory_config = ScabbardFactoryStorageConfig::Sqlite {
            pool: {
                let connection_manager =
                    ConnectionManager::<diesel::SqliteConnection>::new(":memory:");
                let pool = Pool::builder()
                    .max_size(1)
                    .build(connection_manager)
                    .expect("Failed to build connection pool");

                sawtooth::migrations::run_sqlite_migrations(
                    &*pool.get().expect("Failed to get connection for migrations"),
                )
                .expect("Failed to run migrations");
                pool
            },
        };
        let state_db_dir = tempdir.path().to_string_lossy().into();
        (
            tempdir,
            ScabbardFactory {
                service_types: vec![SERVICE_TYPE.into()],
                state_db_dir,
                state_db_size: 1024 * 1024,
                receipt_store_factory_config,
                signature_verifier_factory: Arc::new(Mutex::new(Box::new(Secp256k1Context::new()))),
            },
        )
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
