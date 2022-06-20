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

//! Scabbard is a Splinter `Service` that runs the Sawtooth Sabre smart contract engine using the
//! `transact` library for state. Scabbard uses two-phase consensus to reach agreement on
//! transactions.

mod consensus;
mod error;
pub(crate) mod factory;
#[cfg(feature = "rest-api")]
mod rest_api;
mod shared;
mod state;

use std::any::Any;
use std::collections::{HashSet, VecDeque};
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cylinder::Verifier as SignatureVerifier;
use protobuf::Message;
use sawtooth::receipt::store::ReceiptStore;
use splinter::{
    consensus::{Proposal, ProposalUpdate},
    orchestrator::OrchestratableService,
    service::{
        Service, ServiceDestroyError, ServiceError, ServiceMessageContext, ServiceNetworkRegistry,
        ServiceStartError, ServiceStopError,
    },
};
use transact::{
    protocol::batch::BatchPair,
    protos::{FromBytes, IntoBytes},
};

use crate::store::CommitHashStore;

use super::protos::scabbard::{ScabbardMessage, ScabbardMessage_Type};

use consensus::ScabbardConsensusManager;
use error::ScabbardError;
pub use factory::ConnectionUri;
pub use factory::ScabbardArgValidator;
pub use factory::{ScabbardFactory, ScabbardFactoryBuilder, ScabbardStorageConfiguration};
use shared::ScabbardShared;
use state::merkle_state::MerkleState;
pub use state::{
    BatchInfo, BatchInfoIter, BatchStatus, Events, StateChange, StateChangeEvent, StateIter,
};
use state::{ScabbardState, StateSubscriber};

const SERVICE_TYPE: &str = "scabbard";

const DEFAULT_COORDINATOR_TIMEOUT: u64 = 30; // 30 seconds

/// Specifies the version of scabbard to use.
#[derive(Clone, Copy, PartialEq)]
pub enum ScabbardVersion {
    V1,
    V2,
}

impl TryFrom<Option<&str>> for ScabbardVersion {
    type Error = String;

    fn try_from(str_opt: Option<&str>) -> Result<Self, Self::Error> {
        match str_opt {
            Some("1") => Ok(Self::V1),
            Some("2") => Ok(Self::V2),
            Some(v) => Err(format!("Unsupported scabbard version: {}", v)),
            None => Ok(Self::V1),
        }
    }
}

/// A handler for purging a scabbard instances state
pub trait ScabbardStatePurgeHandler: Send + Sync {
    /// Purge the scabbard instances state.
    fn purge_state(&self) -> Result<(), splinter::error::InternalError>;
}

/// A service for running Sawtooth Sabre smart contracts with two-phase commit consensus.
#[derive(Clone)]
pub struct Scabbard {
    circuit_id: String,
    service_id: String,
    version: ScabbardVersion,
    shared: Arc<Mutex<ScabbardShared>>,
    state: Arc<Mutex<ScabbardState>>,
    purge_handler: Arc<dyn ScabbardStatePurgeHandler>,
    /// The coordinator timeout for the two-phase commit consensus engine
    coordinator_timeout: Duration,
    consensus: Arc<Mutex<Option<ScabbardConsensusManager>>>,
}

impl Scabbard {
    #[allow(clippy::too_many_arguments)]
    /// Generate a new Scabbard service.
    pub fn new(
        service_id: String,
        circuit_id: &str,
        // The protocol version for scabbard
        version: ScabbardVersion,
        // List of other scabbard services on the same circuit that this service shares state with
        peer_services: HashSet<String>,
        merkle_state: MerkleState,
        commit_hash_store: Arc<dyn CommitHashStore + Sync + Send>,
        receipt_store: Arc<dyn ReceiptStore>,
        purge_handler: Box<dyn ScabbardStatePurgeHandler>,
        signature_verifier: Box<dyn SignatureVerifier>,
        // The public keys that are authorized to create and manage sabre contracts
        admin_keys: Vec<String>,
        // The coordinator timeout for the two-phase commit consensus engine; if `None`, the
        // default value will be used (30 seconds).
        coordinator_timeout: Option<Duration>,
    ) -> Result<Self, ScabbardError> {
        let shared = ScabbardShared::new(
            VecDeque::new(),
            None,
            peer_services,
            service_id.clone(),
            #[cfg(feature = "metrics")]
            circuit_id.to_string(),
            signature_verifier,
            version,
        );

        let state = ScabbardState::new(
            merkle_state,
            false,
            commit_hash_store,
            receipt_store,
            #[cfg(feature = "metrics")]
            service_id.clone(),
            #[cfg(feature = "metrics")]
            circuit_id.to_string(),
            admin_keys,
        )
        .map_err(|err| ScabbardError::InitializationFailed(Box::new(err)))?;

        let coordinator_timeout =
            coordinator_timeout.unwrap_or_else(|| Duration::from_secs(DEFAULT_COORDINATOR_TIMEOUT));

        Ok(Scabbard {
            circuit_id: circuit_id.to_string(),
            service_id,
            version,
            shared: Arc::new(Mutex::new(shared)),
            state: Arc::new(Mutex::new(state)),
            purge_handler: purge_handler.into(),
            coordinator_timeout,
            consensus: Arc::new(Mutex::new(None)),
        })
    }

    /// Fetch the value at the given `address` in the scabbard service's state. Returns `None` if
    /// the `address` is not set.
    pub fn get_state_at_address(&self, address: &str) -> Result<Option<Vec<u8>>, ScabbardError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?
            .get_state_at_address(address)?)
    }

    /// Fetch a list of entries in the scabbard service's state. If a `prefix` is provided, only
    /// return entries whose addresses are under the given address prefix. If no `prefix` is
    /// provided, return all state entries.
    pub fn get_state_with_prefix(&self, prefix: Option<&str>) -> Result<StateIter, ScabbardError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?
            .get_state_with_prefix(prefix)?)
    }

    /// Get the current state root hash of the scabbard service's state.
    pub fn get_current_state_root(&self) -> Result<String, ScabbardError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?
            .current_state_root()
            .to_string())
    }

    /// Get whether the service is currently accepting batches
    pub fn accepting_batches(&self) -> Result<bool, ScabbardError> {
        let shared = self
            .shared
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?;

        match self.version {
            ScabbardVersion::V1 => Ok(true),
            ScabbardVersion::V2 => Ok(shared.accepting_batches()),
        }
    }

    pub fn add_batches(&self, batches: Vec<BatchPair>) -> Result<Option<String>, ScabbardError> {
        let mut shared = self
            .shared
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?;

        if shared.verify_batches(&batches)? {
            let mut link = format!(
                "/scabbard/{}/{}/batch_statuses?ids=",
                self.circuit_id, self.service_id
            );

            for batch in batches {
                self.state
                    .lock()
                    .map_err(|_| ScabbardError::LockPoisoned)?
                    .batch_history()
                    .add_batch(batch.batch().header_signature());

                link.push_str(&format!("{},", batch.batch().header_signature()));

                match self.version {
                    ScabbardVersion::V1 => shared.add_batch_to_queue(batch)?,
                    ScabbardVersion::V2 => {
                        if shared.is_coordinator() {
                            shared.add_batch_to_queue(batch)?;
                        } else {
                            let batch_bytes = batch
                                .into_bytes()
                                .map_err(|err| ScabbardError::Internal(Box::new(err)))?;

                            let mut msg = ScabbardMessage::new();
                            msg.set_message_type(ScabbardMessage_Type::NEW_BATCH);
                            msg.set_new_batch(batch_bytes);
                            let msg_bytes = msg
                                .write_to_bytes()
                                .map_err(|err| ScabbardError::Internal(Box::new(err)))?;

                            shared
                                .network_sender()
                                .ok_or(ScabbardError::NotConnected)?
                                .send(shared.coordinator_service_id(), msg_bytes.as_slice())
                                .map_err(|err| ScabbardError::Internal(Box::new(err)))?;
                        }
                    }
                }
            }

            // Remove trailing comma
            link.pop();

            debug!("Batch Status Link Created: {}", link);
            Ok(Some(link))
        } else {
            Ok(None)
        }
    }

    /// Get the `BatchInfo` for each specified batch.
    ///
    /// # Arguments
    ///
    /// * `ids`: List of batch IDs to get info on
    /// * `wait`: If `Some`, wait up to the given time for all requested batches to complete
    ///   (statuses will be either `Committed` or `Invalid`); if the timeout expires, an `Err`
    ///   result will be given by the returned iterator. If `None`, return the `BatchInfo`s to
    ///   complete.
    ///
    pub fn get_batch_info(
        &self,
        ids: HashSet<String>,
        wait: Option<Duration>,
    ) -> Result<BatchInfoIter, ScabbardError> {
        let mut state = self.state.lock().map_err(|_| ScabbardError::LockPoisoned)?;
        Ok(state.batch_history().get_batch_info(ids, wait)?)
    }

    pub fn get_events_since(&self, event_id: Option<String>) -> Result<Events, ScabbardError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?
            .get_events_since(event_id)?)
    }

    pub fn add_state_subscriber(
        &self,
        subscriber: Box<dyn StateSubscriber>,
    ) -> Result<(), ScabbardError> {
        self.state
            .lock()
            .map_err(|_| ScabbardError::LockPoisoned)?
            .add_subscriber(subscriber);

        Ok(())
    }
}

impl Service for Scabbard {
    fn service_id(&self) -> &str {
        &self.service_id
    }

    fn service_type(&self) -> &str {
        SERVICE_TYPE
    }

    fn start(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStartError> {
        let mut consensus = self
            .consensus
            .lock()
            .map_err(|_| ServiceStartError::PoisonedLock("consensus lock poisoned".into()))?;

        if consensus.is_some() {
            return Err(ServiceStartError::AlreadyStarted);
        }

        self.shared
            .lock()
            .map_err(|_| ServiceStartError::PoisonedLock("shared lock poisoned".into()))?
            .set_network_sender(service_registry.connect(self.service_id())?);

        self.state
            .lock()
            .map_err(|_| ServiceStartError::PoisonedLock("shared lock poisoned".into()))?
            .start_executor()
            .map_err(|err| ServiceStartError::Internal(err.to_string()))?;

        // Setup consensus
        consensus.replace(
            ScabbardConsensusManager::new(
                self.service_id().into(),
                self.version,
                self.shared.clone(),
                self.state.clone(),
                self.coordinator_timeout,
            )
            .map_err(|err| {
                ServiceStartError::Internal(format!("Unable to start consensus: {}", err))
            })?,
        );

        Ok(())
    }

    fn stop(
        &mut self,
        service_registry: &dyn ServiceNetworkRegistry,
    ) -> Result<(), ServiceStopError> {
        debug!("Stopping scabbard service with id {}", self.service_id);

        // Shutdown consensus
        self.consensus
            .lock()
            .map_err(|_| ServiceStopError::PoisonedLock("consensus lock poisoned".into()))?
            .take()
            .ok_or(ServiceStopError::NotStarted)?
            .shutdown()
            .map_err(|err| ServiceStopError::Internal(Box::new(ScabbardError::from(err))))?;

        self.shared
            .lock()
            .map_err(|_| ServiceStopError::PoisonedLock("shared lock poisoned".into()))?
            .take_network_sender()
            .ok_or_else(|| ServiceStopError::Internal(Box::new(ScabbardError::NotConnected)))?;

        let mut state = self
            .state
            .lock()
            .map_err(|_| ServiceStopError::PoisonedLock("state lock poisoned".into()))?;

        state.clear_subscribers();

        state.stop_executor();

        service_registry.disconnect(self.service_id())?;

        Ok(())
    }

    fn destroy(self: Box<Self>) -> Result<(), ServiceDestroyError> {
        if self
            .consensus
            .lock()
            .map_err(|_| ServiceDestroyError::PoisonedLock("consensus lock poisoned".into()))?
            .is_some()
        {
            Err(ServiceDestroyError::NotStopped)
        } else {
            Ok(())
        }
    }

    fn purge(&mut self) -> Result<(), splinter::error::InternalError> {
        self.purge_handler.purge_state()
    }

    fn handle_message(
        &self,
        message_bytes: &[u8],
        _message_context: &ServiceMessageContext,
    ) -> Result<(), ServiceError> {
        let message: ScabbardMessage = Message::parse_from_bytes(message_bytes)?;

        match message.get_message_type() {
            ScabbardMessage_Type::CONSENSUS_MESSAGE => self
                .consensus
                .lock()
                .map_err(|_| ServiceError::PoisonedLock("consensus lock poisoned".into()))?
                .as_ref()
                .ok_or(ServiceError::NotStarted)?
                .handle_message(message.get_consensus_message())
                .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err))),
            ScabbardMessage_Type::PROPOSED_BATCH => {
                let proposed_batch = message.get_proposed_batch();

                let proposal = Proposal::try_from(proposed_batch.get_proposal())?;
                let batch = BatchPair::from_bytes(proposed_batch.get_batch())
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))?;

                self.shared
                    .lock()
                    .map_err(|_| ServiceError::PoisonedLock("shared lock poisoned".into()))?
                    .add_open_proposal(proposal.clone(), batch);

                self.consensus
                    .lock()
                    .map_err(|_| ServiceError::PoisonedLock("consensus lock poisoned".into()))?
                    .as_ref()
                    .ok_or(ServiceError::NotStarted)?
                    .send_update(ProposalUpdate::ProposalReceived(
                        proposal,
                        proposed_batch.get_service_id().as_bytes().into(),
                    ))
                    .map_err(|err| ServiceError::UnableToHandleMessage(Box::new(err)))
            }
            ScabbardMessage_Type::NEW_BATCH => {
                match self.version {
                    ScabbardVersion::V1 => {
                        warn!("Scabbard V1 does not accept NEW_BATCH messages");
                    }
                    ScabbardVersion::V2 => {
                        let mut shared = self.shared.lock().map_err(|_| {
                            ServiceError::PoisonedLock("shared lock poisoned".into())
                        })?;

                        if shared.is_coordinator() {
                            let batch =
                                BatchPair::from_bytes(message.get_new_batch()).map_err(|err| {
                                    ServiceError::UnableToHandleMessage(Box::new(err))
                                })?;
                            shared.add_batch_to_queue(batch).map_err(|err| {
                                ServiceError::UnableToHandleMessage(Box::new(err))
                            })?;
                        } else {
                            warn!("Ignoring new batch; this service is not the coordinator");
                        }
                    }
                }

                Ok(())
            }
            ScabbardMessage_Type::TOO_MANY_REQUESTS => {
                match self.version {
                    ScabbardVersion::V1 => {
                        warn!("Scabbard V1 does not accept TOO_MANY_REQUESTS messages");
                    }
                    ScabbardVersion::V2 => {
                        let mut shared = self.shared.lock().map_err(|_| {
                            ServiceError::PoisonedLock("shared lock poisoned".into())
                        })?;
                        if shared.is_coordinator() {
                            warn!("Ignoring too many requests message, not from the coordinator");
                        } else {
                            shared.set_accepting_batches(false);
                        }
                    }
                }
                Ok(())
            }
            ScabbardMessage_Type::ACCEPTING_REQUESTS => {
                match self.version {
                    ScabbardVersion::V1 => {
                        warn!("Scabbard V1 does not accept ACCEPTING_REQUESTS messages");
                    }
                    ScabbardVersion::V2 => {
                        let mut shared = self.shared.lock().map_err(|_| {
                            ServiceError::PoisonedLock("shared lock poisoned".into())
                        })?;
                        if shared.is_coordinator() {
                            warn!("Ignoring accepting requests message, not from the coordinator");
                        } else {
                            shared.set_accepting_batches(true);
                        }
                    }
                }
                Ok(())
            }
            _ => Err(ServiceError::InvalidMessageFormat(Box::new(
                ScabbardError::MessageTypeUnset,
            ))),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl OrchestratableService for Scabbard {
    fn as_service(&self) -> &dyn Service {
        self
    }

    fn clone_box(&self) -> Box<dyn OrchestratableService> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::error::Error;

    use cylinder::{secp256k1::Secp256k1Context, VerifierFactory};
    use sawtooth::receipt::store::{ReceiptIter, ReceiptStoreError};
    use splinter::service::{
        ServiceConnectionError, ServiceDisconnectionError, ServiceMessageContext,
        ServiceNetworkSender, ServiceSendError,
    };
    use transact::protocol::receipt::TransactionReceipt;
    use transact::{
        database::{btree::BTreeDatabase, Database},
        state::merkle::INDEXES,
    };

    use crate::service::state::merkle_state::MerkleStateConfig;
    use crate::store::transact::{TransactCommitHashStore, CURRENT_STATE_ROOT_INDEX};

    /// Tests that a new scabbard service is properly instantiated.
    #[test]
    fn new_scabbard() {
        let (merkle_state, commit_hash_store) = create_merkle_state_and_commit_hash_store();

        let service = Scabbard::new(
            "new_scabbard".into(),
            "test_circuit",
            ScabbardVersion::V1,
            HashSet::new(),
            merkle_state,
            commit_hash_store,
            Arc::new(MockReceiptStore),
            Box::new(NoOpScabbardStatePurgeHandler),
            Secp256k1Context::new().new_verifier(),
            vec![],
            None,
        )
        .expect("failed to create service");
        assert_eq!(service.service_id(), "new_scabbard");
        assert_eq!(service.service_type(), SERVICE_TYPE);
    }

    /// Tests that the scabbard service properly shuts down its internal thread on stop. This test
    /// will hang if the thread does not get shutdown correctly.
    #[test]
    fn thread_cleanup() {
        let (merkle_state, commit_hash_store) = create_merkle_state_and_commit_hash_store();

        let mut service = Scabbard::new(
            "thread_cleanup".into(),
            "test_circuit",
            ScabbardVersion::V1,
            HashSet::new(),
            merkle_state,
            commit_hash_store,
            Arc::new(MockReceiptStore),
            Box::new(NoOpScabbardStatePurgeHandler),
            Secp256k1Context::new().new_verifier(),
            vec![],
            None,
        )
        .expect("failed to create service");
        let registry = MockServiceNetworkRegistry::new();
        service.start(&registry).expect("failed to start service");
        service.stop(&registry).expect("failed to stop service");
    }

    /// Tests that the service properly connects and disconnects using the network registry.
    #[test]
    fn connect_and_disconnect() {
        let (merkle_state, commit_hash_store) = create_merkle_state_and_commit_hash_store();
        let mut service = Scabbard::new(
            "connect_and_disconnect".into(),
            "test_circuit",
            ScabbardVersion::V1,
            HashSet::new(),
            merkle_state,
            commit_hash_store,
            Arc::new(MockReceiptStore),
            Box::new(NoOpScabbardStatePurgeHandler),
            Secp256k1Context::new().new_verifier(),
            vec![],
            None,
        )
        .expect("failed to create service");
        test_connect_and_disconnect(&mut service);
    }

    fn create_merkle_state_and_commit_hash_store(
    ) -> (MerkleState, Arc<dyn CommitHashStore + Send + Sync>) {
        let mut indexes = INDEXES.to_vec();
        indexes.push(CURRENT_STATE_ROOT_INDEX);
        let db = BTreeDatabase::new(&indexes);
        let merkle_state = MerkleState::new(MerkleStateConfig::key_value(db.clone_box()))
            .expect("Unable to create merkle state");
        let commit_hash_store = TransactCommitHashStore::new(db);
        (merkle_state, Arc::new(commit_hash_store))
    }

    #[derive(Debug)]
    pub struct MockServiceNetworkRegistryError(pub String);

    impl Error for MockServiceNetworkRegistryError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            None
        }
    }

    impl std::fmt::Display for MockServiceNetworkRegistryError {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    pub struct MockServiceNetworkRegistry {
        pub connected_ids: Arc<Mutex<HashSet<String>>>,
        network_sender: MockServiceNetworkSender,
    }

    impl MockServiceNetworkRegistry {
        pub fn new() -> Self {
            MockServiceNetworkRegistry {
                connected_ids: Arc::new(Mutex::new(HashSet::new())),
                network_sender: MockServiceNetworkSender::new(),
            }
        }

        pub fn network_sender(&self) -> &MockServiceNetworkSender {
            &self.network_sender
        }
    }

    impl ServiceNetworkRegistry for MockServiceNetworkRegistry {
        fn connect(
            &self,
            service_id: &str,
        ) -> Result<Box<dyn ServiceNetworkSender>, ServiceConnectionError> {
            if self
                .connected_ids
                .lock()
                .expect("connected_ids lock poisoned")
                .insert(service_id.into())
            {
                Ok(Box::new(self.network_sender.clone()))
            } else {
                Err(ServiceConnectionError::RejectedError(format!(
                    "service with id {} already connected",
                    service_id
                )))
            }
        }

        fn disconnect(&self, service_id: &str) -> Result<(), ServiceDisconnectionError> {
            if self
                .connected_ids
                .lock()
                .expect("connected_ids lock poisoned")
                .remove(service_id)
            {
                Ok(())
            } else {
                Err(ServiceDisconnectionError::RejectedError(format!(
                    "service with id {} not connected",
                    service_id
                )))
            }
        }
    }

    #[derive(Clone, Debug)]
    pub struct MockServiceNetworkSender {
        pub sent: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
        pub sent_and_awaited: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
        pub replied: Arc<Mutex<Vec<(ServiceMessageContext, Vec<u8>)>>>,
    }

    impl MockServiceNetworkSender {
        pub fn new() -> Self {
            MockServiceNetworkSender {
                sent: Arc::new(Mutex::new(vec![])),
                sent_and_awaited: Arc::new(Mutex::new(vec![])),
                replied: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    impl ServiceNetworkSender for MockServiceNetworkSender {
        fn send(&self, recipient: &str, message: &[u8]) -> Result<(), ServiceSendError> {
            self.sent
                .lock()
                .expect("sent lock poisoned")
                .push((recipient.to_string(), message.to_vec()));
            Ok(())
        }

        fn send_and_await(
            &self,
            recipient: &str,
            message: &[u8],
        ) -> Result<Vec<u8>, ServiceSendError> {
            self.sent_and_awaited
                .lock()
                .expect("sent_and_awaited lock poisoned")
                .push((recipient.to_string(), message.to_vec()));
            Ok(vec![])
        }

        fn reply(
            &self,
            message_origin: &ServiceMessageContext,
            message: &[u8],
        ) -> Result<(), ServiceSendError> {
            self.replied
                .lock()
                .expect("replied lock poisoned")
                .push((message_origin.clone(), message.to_vec()));
            Ok(())
        }

        fn clone_box(&self) -> Box<dyn ServiceNetworkSender> {
            Box::new(self.clone())
        }

        fn send_with_sender(
            &mut self,
            _recipient: &str,
            _message: &[u8],
            _sender: &str,
        ) -> Result<(), ServiceSendError> {
            Ok(())
        }
    }

    struct MockReceiptStore;

    impl ReceiptStore for MockReceiptStore {
        fn get_txn_receipt_by_id(
            &self,
            _id: String,
        ) -> Result<Option<TransactionReceipt>, ReceiptStoreError> {
            unimplemented!()
        }

        fn get_txn_receipt_by_index(
            &self,
            _index: u64,
        ) -> Result<Option<TransactionReceipt>, ReceiptStoreError> {
            unimplemented!()
        }

        fn add_txn_receipts(
            &self,
            _receipts: Vec<TransactionReceipt>,
        ) -> Result<(), ReceiptStoreError> {
            unimplemented!()
        }

        fn remove_txn_receipt_by_id(
            &self,
            _id: String,
        ) -> Result<Option<TransactionReceipt>, ReceiptStoreError> {
            unimplemented!()
        }

        fn remove_txn_receipt_by_index(
            &self,
            _index: u64,
        ) -> Result<Option<TransactionReceipt>, ReceiptStoreError> {
            unimplemented!()
        }

        fn count_txn_receipts(&self) -> Result<u64, ReceiptStoreError> {
            unimplemented!()
        }

        fn list_receipts_since(
            &self,
            _id: Option<String>,
        ) -> Result<ReceiptIter, ReceiptStoreError> {
            unimplemented!()
        }
    }

    struct NoOpScabbardStatePurgeHandler;

    impl ScabbardStatePurgeHandler for NoOpScabbardStatePurgeHandler {
        fn purge_state(&self) -> Result<(), splinter::error::InternalError> {
            Ok(())
        }
    }

    /// Verifies that the given service connects on start and disconnects on stop.
    pub fn test_connect_and_disconnect(service: &mut dyn Service) {
        let registry = MockServiceNetworkRegistry::new();
        service.start(&registry).expect("failed to start engine");
        assert!(registry
            .connected_ids
            .lock()
            .expect("connected_ids lock poisoned")
            .contains(service.service_id()));
        service.stop(&registry).expect("failed to stop engine");
        assert!(registry
            .connected_ids
            .lock()
            .expect("connected_ids lock poisoned")
            .is_empty());
    }
}
