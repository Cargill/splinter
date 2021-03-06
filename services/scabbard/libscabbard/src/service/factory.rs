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
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cylinder::VerifierFactory;
#[cfg(feature = "service-arg-validation")]
use splinter::service::validation::{ServiceArgValidationError, ServiceArgValidator};
use splinter::service::{FactoryCreateError, Service, ServiceFactory};

#[cfg(feature = "service-arg-validation")]
use crate::hex::parse_hex;

use super::error::ScabbardError;
use super::{Scabbard, ScabbardVersion, SERVICE_TYPE};

const DEFAULT_STATE_DB_DIR: &str = "/var/lib/splinter";
const DEFAULT_STATE_DB_SIZE: usize = 1 << 30; // 1024 ** 3
const DEFAULT_RECEIPT_DB_DIR: &str = "/var/lib/splinter";
const DEFAULT_RECEIPT_DB_SIZE: usize = 1 << 30; // 1024 ** 3

#[cfg(feature = "factory-builder")]
/// Builds new ScabbardFactory instances.
#[derive(Default)]
pub struct ScabbardFactoryBuilder {
    state_db_dir: Option<String>,
    state_db_size: Option<usize>,
    receipt_db_dir: Option<String>,
    receipt_db_size: Option<usize>,
    signature_verifier_factory: Option<Arc<Mutex<Box<dyn VerifierFactory>>>>,
}

#[cfg(feature = "factory-builder")]
impl ScabbardFactoryBuilder {
    /// Constructs a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the state db directory to be used by the resulting factory.
    pub fn with_state_db_dir(mut self, state_db_dir: String) -> Self {
        self.state_db_dir = Some(state_db_dir);
        self
    }

    /// Sets the state db size to be used by the resulting factory.
    pub fn with_state_db_size(mut self, state_db_size: usize) -> Self {
        self.state_db_size = Some(state_db_size);
        self
    }

    /// Sets the receipt db directory to be used by the resulting factory.
    pub fn with_receipt_db_dir(mut self, receipt_db_dir: String) -> Self {
        self.receipt_db_dir = Some(receipt_db_dir);
        self
    }

    /// Sets the receipt db size to be used by the resulting factory.
    pub fn with_receipt_db_size(mut self, receipt_db_size: usize) -> Self {
        self.receipt_db_size = Some(receipt_db_size);
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
    pub fn build(self) -> Result<ScabbardFactory, splinter::error::InvalidStateError> {
        let signature_verifier_factory = self.signature_verifier_factory.ok_or_else(|| {
            splinter::error::InvalidStateError::with_message(
                "A scabbard factory requires a signature verifier factory".into(),
            )
        })?;

        Ok(ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            state_db_dir: self
                .state_db_dir
                .unwrap_or_else(|| DEFAULT_STATE_DB_DIR.into()),
            state_db_size: self.state_db_size.unwrap_or(DEFAULT_STATE_DB_SIZE),
            receipt_db_dir: self
                .receipt_db_dir
                .unwrap_or_else(|| DEFAULT_RECEIPT_DB_DIR.into()),
            receipt_db_size: self.receipt_db_size.unwrap_or(DEFAULT_RECEIPT_DB_SIZE),
            signature_verifier_factory,
        })
    }
}

pub struct ScabbardFactory {
    service_types: Vec<String>,
    state_db_dir: String,
    state_db_size: usize,
    receipt_db_dir: String,
    receipt_db_size: usize,
    signature_verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
}

impl ScabbardFactory {
    pub fn new(
        state_db_dir: Option<String>,
        state_db_size: Option<usize>,
        receipt_db_dir: Option<String>,
        receipt_db_size: Option<usize>,
        signature_verifier_factory: Arc<Mutex<Box<dyn VerifierFactory>>>,
    ) -> Self {
        ScabbardFactory {
            service_types: vec![SERVICE_TYPE.into()],
            state_db_dir: state_db_dir.unwrap_or_else(|| DEFAULT_STATE_DB_DIR.into()),
            state_db_size: state_db_size.unwrap_or(DEFAULT_STATE_DB_SIZE),
            receipt_db_dir: receipt_db_dir.unwrap_or_else(|| DEFAULT_RECEIPT_DB_DIR.into()),
            receipt_db_size: receipt_db_size.unwrap_or(DEFAULT_RECEIPT_DB_SIZE),
            signature_verifier_factory,
        }
    }
}

#[cfg(feature = "service-arg-validation")]
pub struct ScabbardArgValidator;

#[cfg(feature = "service-arg-validation")]
impl ServiceArgValidator for ScabbardArgValidator {
    fn validate(&self, args: &HashMap<String, String>) -> Result<(), ServiceArgValidationError> {
        let peer_services_str = args.get("peer_services").ok_or_else(|| {
            ServiceArgValidationError("peer_services argument not provided".into())
        })?;

        serde_json::from_str::<Vec<String>>(peer_services_str).map_err(|err| {
            ServiceArgValidationError(format!("failed to parse peer_services list: {}", err,))
        })?;

        let admin_keys_str = args
            .get("admin_keys")
            .ok_or_else(|| ServiceArgValidationError("admin_keys argument not provided".into()))?;

        let admin_keys = parse_list(admin_keys_str).map_err(|err| {
            ServiceArgValidationError(format!("failed to parse admin_keys list: {}", err,))
        })?;

        for key in admin_keys {
            let key_bytes = parse_hex(&key).map_err(|_| {
                ServiceArgValidationError(format!(
                    "{:?} is not a valid hex-formatted public key",
                    key,
                ))
            })?;

            if key_bytes.len() != 33 {
                return Err(ServiceArgValidationError(format!(
                    "{} is not a valid public key: invalid length",
                    key
                )));
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
        let receipt_db_dir = Path::new(&self.receipt_db_dir);
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

        let service = Scabbard::new(
            service_id,
            circuit_id,
            version,
            peer_services,
            &state_db_dir,
            self.state_db_size,
            &receipt_db_dir,
            self.receipt_db_size,
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
            use super::rest_api::actix;
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

#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::secp256k1::Secp256k1Context;

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

    fn get_factory() -> ScabbardFactory {
        ScabbardFactory::new(
            Some("/tmp".into()),
            Some(1024 * 1024),
            Some("/tmp".into()),
            Some(1024 * 1024),
            Arc::new(Mutex::new(Box::new(Secp256k1Context::new()))),
        )
    }

    fn get_mock_args() -> HashMap<String, String> {
        let peer_services = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let admin_keys: Vec<String> = vec![];
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
        args.insert("admin_keys".into(), "".into());
        args
    }
}
