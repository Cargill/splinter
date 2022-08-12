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

pub mod merkle_state;

use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::fmt;
use std::sync::{
    mpsc::{channel, Receiver, Sender, TryRecvError},
    Arc,
};
use std::time::{Duration, Instant, SystemTime};

use protobuf::Message;
use sawtooth::receipt::store::ReceiptStore;
use serde::{
    de::{SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
#[cfg(feature = "events")]
use splinter::events::{ParseBytes, ParseError};
#[cfg(test)]
use transact::families::command::CommandTransactionHandler;
use transact::{
    context::manager::sync::ContextManager,
    execution::{adapter::static_adapter::StaticExecutionAdapter, executor::Executor},
    families::sabre::{
        admin::SettingsAdminPermission, handler::SabreTransactionHandler,
        ADMINISTRATORS_SETTING_ADDRESS, ADMINISTRATORS_SETTING_KEY,
    },
    protocol::{
        batch::BatchPair,
        receipt::{TransactionReceipt, TransactionResult},
    },
    scheduler::{serial::SerialScheduler, BatchExecutionResult, Scheduler},
    state::{
        merkle::{MerkleRadixLeafReadError, MerkleRadixLeafReader},
        Prune, Read, StateChange as TransactStateChange, Write,
    },
};

use crate::protos::scabbard::{Setting, Setting_Entry};
use crate::service::error::{ScabbardStateError, StateSubscriberError};
use crate::store::CommitHashStore;

const EXECUTION_TIMEOUT: u64 = 300; // five minutes
const ITER_CACHE_SIZE: usize = 64;
const COMPLETED_BATCH_INFO_ITER_RETRY: Duration = Duration::from_millis(100);
const DEFAULT_BATCH_HISTORY_SIZE: usize = 100;

/// Iterator over entries in a Scabbard service's state
pub type StateIter = Box<dyn Iterator<Item = Result<(String, Vec<u8>), ScabbardStateError>>>;

pub struct ScabbardState {
    merkle_state: merkle_state::MerkleState,
    state_autocleanup_enabled: bool,
    commit_hash_store: Arc<dyn CommitHashStore + Sync + Send>,
    context_manager: ContextManager,
    executor: Option<Executor>,
    current_state_root: String,
    receipt_store: Arc<dyn ReceiptStore>,
    pending_changes: Option<(String, Vec<TransactionReceipt>)>,
    event_subscribers: Vec<Box<dyn StateSubscriber>>,
    #[cfg(feature = "metrics")]
    service_id: String,
    #[cfg(feature = "metrics")]
    circuit_id: String,
    batch_history: BatchHistory,
}

impl ScabbardState {
    pub fn new(
        merkle_state: merkle_state::MerkleState,
        state_autocleanup_enabled: bool,
        commit_hash_store: Arc<dyn CommitHashStore + Sync + Send>,
        receipt_store: Arc<dyn ReceiptStore>,
        #[cfg(feature = "metrics")] service_id: String,
        #[cfg(feature = "metrics")] circuit_id: String,
        admin_keys: Vec<String>,
    ) -> Result<Self, ScabbardStateError> {
        let current_state_root = if let Some(current_state_root) = commit_hash_store
            .get_current_commit_hash()
            .map_err(|err| ScabbardStateError(err.to_string()))?
        {
            debug!("Restoring scabbard state on root {}", current_state_root);
            current_state_root
        } else {
            // Set initial state (admin keys)
            let mut admin_keys_entry = Setting_Entry::new();
            admin_keys_entry.set_key(ADMINISTRATORS_SETTING_KEY.into());
            admin_keys_entry.set_value(admin_keys.join(","));
            let mut admin_keys_setting = Setting::new();
            admin_keys_setting.set_entries(vec![admin_keys_entry].into());
            let admin_keys_setting_bytes = admin_keys_setting.write_to_bytes().map_err(|err| {
                ScabbardStateError(format!(
                    "failed to write admin keys setting to bytes: {}",
                    err
                ))
            })?;
            let admin_keys_state_change = TransactStateChange::Set {
                key: ADMINISTRATORS_SETTING_ADDRESS.into(),
                value: admin_keys_setting_bytes,
            };

            let initial_state_root = merkle_state
                .get_initial_state_root()
                .map_err(|err| ScabbardStateError(err.to_string()))?;

            let new_state_root = merkle_state.commit(
                &initial_state_root,
                vec![admin_keys_state_change].as_slice(),
            )?;

            // store the new state root to the commit store
            commit_hash_store
                .set_current_commit_hash(&new_state_root)
                .map_err(|err| ScabbardStateError(err.to_string()))?;

            new_state_root
        };

        // Initialize transact
        let context_manager = ContextManager::new(Box::new(merkle_state.clone()));
        // initialize committed_batches metric
        counter!("splinter.scabbard.committed_batches", 0,
            "circuit" => circuit_id.clone(),
            "service" => format!("{}::{}", &circuit_id, &service_id)
        );

        Ok(ScabbardState {
            merkle_state,
            state_autocleanup_enabled,
            commit_hash_store,
            context_manager,
            executor: None,
            current_state_root,
            receipt_store,
            pending_changes: None,
            event_subscribers: vec![],
            #[cfg(feature = "metrics")]
            service_id,
            #[cfg(feature = "metrics")]
            circuit_id,
            batch_history: BatchHistory::new(),
        })
    }

    pub fn start_executor(&mut self) -> Result<(), ScabbardStateError> {
        let mut executor = Executor::new(vec![Box::new(StaticExecutionAdapter::new_adapter(
            vec![
                Box::new(SabreTransactionHandler::new(Box::new(
                    SettingsAdminPermission,
                ))),
                #[cfg(test)]
                Box::new(CommandTransactionHandler::new()),
            ],
            self.context_manager.clone(),
        )?)]);
        executor
            .start()
            .map_err(|err| ScabbardStateError(format!("failed to start executor: {}", err)))?;

        self.executor = Some(executor);

        Ok(())
    }

    pub fn stop_executor(&mut self) {
        if let Some(executor) = self.executor.take() {
            executor.stop();
        }
    }

    fn write_current_state_root(&self) -> Result<(), ScabbardStateError> {
        self.commit_hash_store
            .set_current_commit_hash(&self.current_state_root)
            .map_err(|err| ScabbardStateError(err.to_string()))
    }

    /// Fetch the value at the given `address` in state. Returns `None` if the `address` is not set.
    pub fn get_state_at_address(
        &self,
        address: &str,
    ) -> Result<Option<Vec<u8>>, ScabbardStateError> {
        self.merkle_state
            .get(&self.current_state_root, &[address.to_string()])
            .map(|mut values| values.remove(address))
            .map_err(|err| ScabbardStateError(err.to_string()))
    }

    /// Fetch a list of entries in state. If a `prefix` is provided, only return entries whose
    /// addresses are under the given address prefix. If no `prefix` is provided, return all state
    /// entries.
    pub fn get_state_with_prefix(
        &self,
        prefix: Option<&str>,
    ) -> Result<StateIter, ScabbardStateError> {
        Ok(Box::new(
            self.merkle_state
                .leaves(&self.current_state_root, prefix)
                .or_else(|err| match err {
                    MerkleRadixLeafReadError::InvalidStateError(_) => {
                        Ok(Box::new(std::iter::empty()))
                    }
                    err => Err(ScabbardStateError(err.to_string())),
                })?
                .map(|res| res.map_err(|e| ScabbardStateError(e.to_string()))),
        ))
    }

    /// Get the current state root hash.
    pub fn current_state_root(&self) -> &str {
        &self.current_state_root
    }

    pub fn prepare_change(&mut self, batch: BatchPair) -> Result<String, ScabbardStateError> {
        let executor = self.executor.as_ref().ok_or_else(|| {
            ScabbardStateError("attempting to prepare a change on a stopped service".into())
        })?;
        // Setup the transact scheduler
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let mut scheduler = SerialScheduler::new(
            Box::new(self.context_manager.clone()),
            self.current_state_root.clone(),
        )?;
        scheduler.set_result_callback(Box::new(move |batch_result| {
            if result_tx.send(batch_result).is_err() {
                error!("Unable to send batch result; receiver must have dropped");
            }
        }))?;

        // Add the batch to, finalize, and execute the scheduler
        scheduler.add_batch(batch.clone())?;
        scheduler.finalize()?;
        executor.execute(scheduler.take_task_iterator()?, scheduler.new_notifier()?)?;

        let mut recv_result: Option<BatchExecutionResult> = None;

        // Get the results and shutdown the scheduler
        // after receiving the batch result wait until the receiver gets a `None` response
        // from the scheduler before shutting down
        loop {
            match result_rx.recv_timeout(Duration::from_secs(EXECUTION_TIMEOUT)) {
                Ok(Some(res)) => recv_result = Some(res),
                Ok(None) => break,
                Err(_) => {
                    return Err(ScabbardStateError(
                        "Failed to receive result in reasonable time".into(),
                    ))
                }
            }
        }

        let batch_result = recv_result
            .ok_or_else(|| ScabbardStateError("No batch result returned from executor".into()))?;

        let batch_status = batch_result.clone().into();
        let signature = batch.batch().header_signature();
        self.batch_history
            .update_batch_status(signature, batch_status);

        let txn_receipts = batch_result
            .receipts
            .into_iter()
            .map(|receipt| match receipt.transaction_result {
                TransactionResult::Valid { .. } => Ok(receipt),
                TransactionResult::Invalid { error_message, .. } => Err(ScabbardStateError(
                    format!("transaction failed: {:?}", error_message),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Save the results and compute the resulting state root
        let state_root = self.merkle_state.compute_state_id(
            &self.current_state_root,
            &receipts_into_transact_state_changes(&txn_receipts)?,
        )?;
        self.pending_changes = Some((signature.to_string(), txn_receipts));
        Ok(state_root)
    }

    pub fn commit(&mut self) -> Result<(), ScabbardStateError> {
        match self.pending_changes.take() {
            Some((signature, txn_receipts)) => {
                let state_changes = receipts_into_transact_state_changes(&txn_receipts)?;

                let previous_state_root = self.current_state_root.clone();
                self.current_state_root = self
                    .merkle_state
                    .commit(&self.current_state_root, &state_changes)?;

                self.write_current_state_root()?;

                info!(
                    "committed {} change(s) for new state root {}",
                    state_changes.len(),
                    self.current_state_root,
                );

                let events = txn_receipts
                    .iter()
                    .cloned()
                    .map(StateChangeEvent::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                self.receipt_store
                    .add_txn_receipts(txn_receipts)
                    .map_err(|err| {
                        ScabbardStateError(format!(
                            "failed to add transaction receipts to store: {}",
                            err
                        ))
                    })?;

                for event in events {
                    self.event_subscribers.retain(|subscriber| {
                        match subscriber.handle_event(event.clone()) {
                            Ok(()) => true,
                            Err(StateSubscriberError::Unsubscribe) => false,
                            Err(err @ StateSubscriberError::UnableToHandleEvent(_)) => {
                                error!("{}", err);
                                true
                            }
                        }
                    });
                }

                self.batch_history.commit(&signature);
                counter!("splinter.scabbard.committed_batches", 1,
                    "circuit" => self.circuit_id.clone(),
                    "service" => format!("{}::{}", &self.circuit_id, &self.service_id)
                );

                if previous_state_root != self.current_state_root {
                    self.merkle_state
                        .prune(vec![previous_state_root.clone()])
                        .map_err(|err| {
                            ScabbardStateError(format!(
                                "failed to prune previous state {}: {}",
                                previous_state_root, err
                            ))
                        })?;

                    if self.state_autocleanup_enabled {
                        if let Err(err) = self.merkle_state.remove_pruned_entries() {
                            error!(
                                "failed to cleanup pruned state for root {}: {}",
                                previous_state_root, err
                            )
                        }
                    }
                }

                Ok(())
            }
            None => Err(ScabbardStateError("no pending changes to commit".into())),
        }
    }

    pub fn rollback(&mut self) -> Result<(), ScabbardStateError> {
        match self.pending_changes.take() {
            Some((_, txn_receipts)) => info!(
                "discarded {} change(s)",
                receipts_into_transact_state_changes(&txn_receipts)?.len()
            ),
            None => debug!("no changes to rollback"),
        }

        Ok(())
    }

    pub fn batch_history(&mut self) -> &mut BatchHistory {
        &mut self.batch_history
    }

    pub fn get_events_since(&self, event_id: Option<String>) -> Result<Events, ScabbardStateError> {
        Events::new(self.receipt_store.clone(), event_id)
    }

    pub fn add_subscriber(&mut self, subscriber: Box<dyn StateSubscriber>) {
        self.event_subscribers.push(subscriber);
    }

    pub fn clear_subscribers(&mut self) {
        self.event_subscribers.clear();
    }
}

fn receipts_into_transact_state_changes(
    receipts: &[TransactionReceipt],
) -> Result<Vec<TransactStateChange>, ScabbardStateError> {
    Ok(receipts
        .iter()
        .cloned()
        .map(Vec::<TransactStateChange>::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| ScabbardStateError(err.to_string()))?
        .into_iter()
        .flatten()
        .collect())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub id: String,
    pub state_changes: Vec<StateChange>,
}

#[cfg(feature = "events")]
impl ParseBytes<StateChangeEvent> for StateChangeEvent {
    fn from_bytes(bytes: &[u8]) -> Result<StateChangeEvent, ParseError> {
        serde_json::from_slice(bytes)
            .map_err(Box::new)
            .map_err(|err| ParseError::MalformedMessage(err))
    }
}

impl TryFrom<TransactionReceipt> for StateChangeEvent {
    type Error = ScabbardStateError;

    fn try_from(receipt: TransactionReceipt) -> Result<Self, Self::Error> {
        let TransactionReceipt {
            transaction_id,
            transaction_result,
        } = receipt;

        match transaction_result {
            TransactionResult::Valid { state_changes, .. } => {
                Ok(StateChangeEvent {
                    id: transaction_id,
                    state_changes: state_changes.into_iter().map(StateChange::from).collect(),
                })
            }
            TransactionResult::Invalid { .. } => Err(ScabbardStateError(
                format!("cannot convert transaction receipt ({}) to state change event because transction result is `Invalid`", transaction_id)
            )),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StateChange {
    Set { key: String, value: Vec<u8> },
    Delete { key: String },
}

impl fmt::Display for StateChange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StateChange::Set { key, value } => {
                write!(f, "Set(key: {}, payload_size: {})", key, value.len())
            }
            StateChange::Delete { key } => write!(f, "Delete(key: {})", key),
        }
    }
}

impl fmt::Debug for StateChange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl From<transact::protocol::receipt::StateChange> for StateChange {
    fn from(change: transact::protocol::receipt::StateChange) -> Self {
        match change {
            transact::protocol::receipt::StateChange::Set { key, value } => {
                StateChange::Set { key, value }
            }
            transact::protocol::receipt::StateChange::Delete { key } => StateChange::Delete { key },
        }
    }
}

pub trait StateSubscriber: Send {
    fn handle_event(&self, event: StateChangeEvent) -> Result<(), StateSubscriberError>;
}

#[derive(PartialEq)]
enum EventQuery {
    Fetch(Option<String>),
    Exhausted,
}

/// An iterator that wraps the `ReceiptStore` and returns `StateChangeEvent`s using an
/// in-memory cache.
pub struct Events {
    receipt_store: Arc<dyn ReceiptStore>,
    query: EventQuery,
    cache: VecDeque<StateChangeEvent>,
}

impl Events {
    fn new(
        receipt_store: Arc<dyn ReceiptStore>,
        start_id: Option<String>,
    ) -> Result<Self, ScabbardStateError> {
        let mut iter = Events {
            receipt_store,
            query: EventQuery::Fetch(start_id),
            cache: VecDeque::default(),
        };
        iter.reload_cache()?;
        Ok(iter)
    }

    fn reload_cache(&mut self) -> Result<(), ScabbardStateError> {
        match self.query {
            EventQuery::Fetch(ref start_id) => {
                self.cache = if let Some(id) = start_id.as_ref() {
                    self.receipt_store.list_receipts_since(Some(id.clone()))
                } else {
                    self.receipt_store.list_receipts_since(None)
                }
                .map_err(|err| {
                    ScabbardStateError(format!(
                        "failed to get transaction receipts from store: {}",
                        err
                    ))
                })?
                .take(ITER_CACHE_SIZE)
                .map(|res| match res {
                    Ok(receipt) => StateChangeEvent::try_from(receipt),
                    Err(err) => Err(ScabbardStateError(format!(
                        "failed to get transaction receipt: {}",
                        err
                    ))),
                })
                .collect::<Result<VecDeque<_>, _>>()?;

                self.query = self
                    .cache
                    .back()
                    .map(|event| EventQuery::Fetch(Some(event.id.clone())))
                    .unwrap_or(EventQuery::Exhausted);

                Ok(())
            }
            EventQuery::Exhausted => Ok(()),
        }
    }
}

impl Iterator for Events {
    type Item = StateChangeEvent;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cache.is_empty() && self.query != EventQuery::Exhausted {
            if let Err(err) = self.reload_cache() {
                error!("Unable to reload iterator cache: {}", err);
            }
        }
        self.cache.pop_front()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "statusType", content = "message")]
pub enum BatchStatus {
    #[serde(deserialize_with = "empty_array")]
    Unknown,
    #[serde(deserialize_with = "empty_array")]
    Pending,
    Invalid(Vec<InvalidTransaction>),
    Valid(Vec<ValidTransaction>),
    Committed(Vec<ValidTransaction>),
}

fn empty_array<'de, D>(d: D) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    struct OuterVisitor;

    impl<'de> Visitor<'de> for OuterVisitor {
        type Value = Vec<()>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an array of messages")
        }

        #[inline]
        fn visit_seq<V>(self, _: V) -> Result<Self::Value, V::Error>
        where
            V: SeqAccess<'de>,
        {
            Ok(Vec::new())
        }
    }

    d.deserialize_seq(OuterVisitor)?;

    Ok(())
}

impl From<BatchExecutionResult> for BatchStatus {
    fn from(batch_result: BatchExecutionResult) -> Self {
        let mut valid = Vec::new();
        let mut invalid = Vec::new();

        for receipt in batch_result.receipts.into_iter() {
            match receipt.transaction_result {
                TransactionResult::Valid { .. } => {
                    valid.push(ValidTransaction::new(receipt.transaction_id));
                }
                TransactionResult::Invalid {
                    error_message,
                    error_data,
                } => {
                    invalid.push(InvalidTransaction::new(
                        receipt.transaction_id,
                        error_message,
                        error_data,
                    ));
                }
            }
        }

        if !invalid.is_empty() {
            BatchStatus::Invalid(invalid)
        } else {
            BatchStatus::Valid(valid)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidTransaction {
    pub transaction_id: String,
}

impl ValidTransaction {
    fn new(transaction_id: String) -> Self {
        Self { transaction_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvalidTransaction {
    pub transaction_id: String,
    pub error_message: String,
    pub error_data: Vec<u8>,
}

impl InvalidTransaction {
    fn new(transaction_id: String, error_message: String, error_data: Vec<u8>) -> Self {
        Self {
            transaction_id,
            error_message,
            error_data,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchInfo {
    pub id: String,
    pub status: BatchStatus,
    #[serde(skip, default = "SystemTime::now")]
    pub timestamp: SystemTime,
}

impl BatchInfo {
    fn set_status(&mut self, status: BatchStatus) {
        self.status = status;
    }
}

/// BatchHistory keeps track of batches submitted to scabbard
pub struct BatchHistory {
    history: HashMap<String, BatchInfo>,
    limit: usize,
    batch_subscribers: Vec<(HashSet<String>, Sender<BatchInfo>)>,
}

impl BatchHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_batch(&mut self, signature: &str) {
        self.upsert_batch(signature.into(), BatchStatus::Pending);
    }

    fn update_batch_status(&mut self, signature: &str, status: BatchStatus) {
        let batch_info = self.upsert_batch(signature.into(), status);

        match batch_info.status {
            BatchStatus::Invalid(_) | BatchStatus::Committed(_) => {
                self.send_completed_batch_info_to_subscribers(batch_info)
            }
            _ => {}
        }
    }

    fn commit(&mut self, signature: &str) {
        match self.history.get_mut(signature) {
            Some(info) => match info.status.clone() {
                BatchStatus::Valid(txns) => {
                    self.update_batch_status(signature, BatchStatus::Committed(txns));
                }
                _ => {
                    error!(
                        "Received commit for batch that was not valid: {:?}",
                        signature
                    );
                }
            },
            None => {
                debug!(
                    "Received commit for batch that is not in the history: {:?}",
                    signature
                );
            }
        }
    }

    fn upsert_batch(&mut self, signature: String, status: BatchStatus) -> BatchInfo {
        match self.history.get_mut(&signature) {
            Some(info) => {
                info.set_status(status);
                info.clone()
            }
            None => {
                let batch_info = BatchInfo {
                    id: signature.clone(),
                    status,
                    timestamp: SystemTime::now(),
                };

                self.history.insert(signature, batch_info.clone());

                if self.history.len() > self.limit {
                    self.history
                        .clone()
                        .into_iter()
                        .min_by_key(|(_, v)| v.timestamp)
                        .and_then(|(k, _)| self.history.remove(&k));
                }

                batch_info
            }
        }
    }

    pub fn get_batch_info(
        &mut self,
        ids: HashSet<String>,
        wait: Option<Duration>,
    ) -> Result<BatchInfoIter, ScabbardStateError> {
        match wait {
            Some(timeout) => self.completed_batch_info_iter(ids, timeout),
            None => Ok(self.no_wait_batch_info_iter(&ids)),
        }
    }

    fn no_wait_batch_info_iter(&self, ids: &HashSet<String>) -> BatchInfoIter {
        Box::new(
            ids.iter()
                .map(|id| {
                    Ok(if let Some(info) = self.history.get(id) {
                        info.clone()
                    } else {
                        BatchInfo {
                            id: id.to_string(),
                            status: BatchStatus::Unknown,
                            timestamp: SystemTime::now(),
                        }
                    })
                })
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn completed_batch_info_iter(
        &mut self,
        mut ids: HashSet<String>,
        timeout: Duration,
    ) -> Result<BatchInfoIter, ScabbardStateError> {
        let mut ready = vec![];
        let mut wait: HashMap<String, BatchInfo> = HashMap::new();

        // Get batches that are already completed
        for res in self.no_wait_batch_info_iter(&ids) {
            match res {
                Ok(info) => {
                    match info.status {
                        // Invalid and committed batches are "ready" and can be returned
                        // immediately
                        BatchStatus::Invalid(_) | BatchStatus::Committed(_) => {
                            ids.remove(&info.id);
                            ready.push(Ok(info));
                        }
                        // Other batches need to be waited on, but we'll still prepare a status to
                        // return if the wait times out
                        status => {
                            wait.insert(
                                info.id.clone(),
                                BatchInfo {
                                    id: info.id.clone(),
                                    status,
                                    timestamp: info.timestamp,
                                },
                            );
                        }
                    }
                }
                Err(err) => {
                    ready.push(Err(err));
                }
            }
        }

        let (sender, receiver) = channel();

        self.batch_subscribers.push((ids.clone(), sender));

        Ok(Box::new(ready.into_iter().chain(
            ChannelBatchInfoIter::new(receiver, timeout, ids, wait)?,
        )))
    }

    fn send_completed_batch_info_to_subscribers(&mut self, info: BatchInfo) {
        self.batch_subscribers = self
            .batch_subscribers
            .drain(..)
            .filter_map(|(mut pending_signatures, sender)| {
                match info.status {
                    BatchStatus::Invalid(_) | BatchStatus::Committed(_) => {
                        if pending_signatures.remove(&info.id) && sender.send(info.clone()).is_err()
                        {
                            // Receiver has been dropped
                            return None;
                        }
                    }
                    _ => (),
                }

                if pending_signatures.is_empty() {
                    None
                } else {
                    Some((pending_signatures, sender))
                }
            })
            .collect();
    }
}

impl Default for BatchHistory {
    fn default() -> Self {
        Self {
            history: HashMap::new(),
            limit: DEFAULT_BATCH_HISTORY_SIZE,
            batch_subscribers: vec![],
        }
    }
}

pub type BatchInfoIter = Box<dyn Iterator<Item = Result<BatchInfo, String>>>;

pub struct ChannelBatchInfoIter {
    receiver: Receiver<BatchInfo>,
    retry_interval: Duration,
    timeout: Instant,
    pending_ids: HashSet<String>,
    history: HashMap<String, BatchInfo>,
}

impl ChannelBatchInfoIter {
    fn new(
        receiver: Receiver<BatchInfo>,
        timeout: Duration,
        pending_ids: HashSet<String>,
        history: HashMap<String, BatchInfo>,
    ) -> Result<Self, ScabbardStateError> {
        Ok(Self {
            receiver,
            retry_interval: std::cmp::min(timeout, COMPLETED_BATCH_INFO_ITER_RETRY),
            timeout: Instant::now()
                .checked_add(timeout)
                .ok_or_else(|| ScabbardStateError("failed to schedule timeout".into()))?,
            pending_ids,
            history,
        })
    }
}

impl Iterator for ChannelBatchInfoIter {
    type Item = Result<BatchInfo, String>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Check if all pending IDs have been returned
            if self.pending_ids.is_empty() {
                return None;
            }

            match self.receiver.try_recv() {
                Ok(batch_info) => match batch_info.status {
                    BatchStatus::Invalid(_) | BatchStatus::Committed(_) => {
                        self.pending_ids.remove(&batch_info.id);
                        return Some(Ok(batch_info));
                    }
                    _ => {}
                },
                Err(TryRecvError::Empty) => {
                    // Check if the timeout has expired
                    if Instant::now() >= self.timeout {
                        return Some(match self.pending_ids.iter().next() {
                            Some(id) => {
                                let id = id.to_string();
                                self.pending_ids.remove(&id);
                                self.history
                                    .remove(&id)
                                    .ok_or_else(|| format!("error getting id '{id}'"))
                            }
                            None => Err("error getting pending id".to_string()),
                        });
                    }
                    std::thread::sleep(self.retry_interval);
                }
                Err(TryRecvError::Disconnected) => return None,
            }
        }
    }
}

#[cfg(feature = "sqlite")]
#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use diesel::{
        r2d2::{ConnectionManager, Pool},
        sqlite::SqliteConnection,
    };
    use sawtooth::migrations::run_sqlite_migrations;
    use sawtooth::receipt::store::diesel::DieselReceiptStore;
    use transact::{
        database::{btree::BTreeDatabase, Database},
        families::command::CommandTransactionBuilder,
        protocol::command::{BytesEntry, Command, SetState},
        state::merkle::INDEXES,
    };

    use crate::store::transact::{TransactCommitHashStore, CURRENT_STATE_ROOT_INDEX};

    use super::merkle_state::{MerkleState, MerkleStateConfig};

    /// Verify that the ChannelBatchInfoIter returns results as they are passed in after timeout
    #[test]
    fn channel_batch_iter_results_after_timeout() -> Result<(), Box<dyn std::error::Error>> {
        let (_tx, rx) = channel();

        let history: HashMap<String, BatchInfo> = vec![
            (
                "batch-id-1".to_string(),
                BatchInfo {
                    id: "batch-id-1".to_string(),
                    status: BatchStatus::Unknown,
                    timestamp: SystemTime::now(),
                },
            ),
            (
                "batch-id-2".to_string(),
                BatchInfo {
                    id: "batch-id-2".to_string(),
                    status: BatchStatus::Pending,
                    timestamp: SystemTime::now(),
                },
            ),
        ]
        .into_iter()
        .collect();

        let results: HashMap<String, BatchStatus> = ChannelBatchInfoIter::new(
            rx,
            Duration::from_secs(0),
            vec!["batch-id-1", "batch-id-2"]
                .into_iter()
                .map(String::from)
                .collect(),
            history,
        )?
        .map(|result| {
            let result = result.unwrap();
            (result.id, result.status)
        })
        .into_iter()
        .collect();

        // Validate the results match what was passed in
        assert_eq!(results.get("batch-id-1").unwrap(), &BatchStatus::Unknown);
        assert_eq!(results.get("batch-id-2").unwrap(), &BatchStatus::Pending);

        // Validate the result length is what we expect
        assert_eq!(results.values().count(), 2);

        Ok(())
    }

    /// Verify that the ChannelBatchInfoIter returns a value if it is sent before next is called.
    #[test]
    fn channel_batch_iter_batch_info_before_next() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = channel();

        let mut iter = ChannelBatchInfoIter::new(
            rx,
            Duration::from_secs(0),
            vec!["batch-id-1".to_string()].into_iter().collect(),
            HashMap::new(),
        )?;

        tx.send(BatchInfo {
            id: "batch-id-1".into(),
            status: BatchStatus::Committed(vec![ValidTransaction::new("ab".into())]),
            timestamp: SystemTime::now(),
        })?;

        let info = iter.next().transpose()?;

        assert!(info.is_some());

        let info = info.unwrap();

        assert_eq!(&info.id, "batch-id-1");

        match info.status {
            BatchStatus::Committed(_) => (), // Expected
            status => panic!(
                "Unexpected batch status {:?}. Expected BatchStatus::Committed",
                status
            ),
        }

        let info = iter.next();

        assert!(info.is_none());

        Ok(())
    }

    /// Verify that the ChannelBatchInfoIter returns a value if it is sent after next is called.
    #[test]
    fn channel_batch_iter_batch_info_after_next() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = channel();

        let barrier = Arc::new(std::sync::Barrier::new(2));

        let iter_barrier = Arc::clone(&barrier);
        let jh = std::thread::spawn(move || {
            let mut iter = ChannelBatchInfoIter::new(
                rx,
                Duration::from_secs(1),
                vec!["batch-id-1".to_string()].into_iter().collect(),
                HashMap::new(),
            )
            .unwrap();

            iter_barrier.wait();

            let info = iter.next().transpose().unwrap();

            assert!(info.is_some());

            let info = info.unwrap();

            assert_eq!(&info.id, "batch-id-1");

            match info.status {
                BatchStatus::Committed(_) => (), // Expected
                status => panic!(
                    "Unexpected batch status {:?}. Expected BatchStatus::Committed",
                    status
                ),
            }

            let info = iter.next();

            assert!(info.is_none());
        });

        barrier.wait();

        // Wait a tiny amount for the iter.next() call.
        std::thread::sleep(Duration::from_millis(10));

        tx.send(BatchInfo {
            id: "batch-id-1".into(),
            status: BatchStatus::Committed(vec![ValidTransaction::new("ab".into())]),
            timestamp: SystemTime::now(),
        })?;

        jh.join().unwrap();

        Ok(())
    }

    /// Verify that an empty receipt store returns an empty iterator
    #[test]
    fn empty_event_iterator() {
        let pool = create_connection_pool_and_migrate(":memory:".to_string());

        let receipt_store = Arc::new(DieselReceiptStore::new(
            pool,
            Some("empty_event_iterator".into()),
        ));

        // Test without a specified start
        let all_events =
            Events::new(receipt_store, None).expect("failed to get iterator for all events");
        let all_event_ids = all_events.map(|event| event.id.clone()).collect::<Vec<_>>();

        assert!(
            all_event_ids.is_empty(),
            "All events should have been empty"
        );
    }

    /// Verify that the event iterator works as expected.
    #[test]
    fn event_iterator() {
        let receipts = vec![
            mock_transaction_receipt("ab"),
            mock_transaction_receipt("cd"),
            mock_transaction_receipt("ef"),
        ];
        let receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.transaction_id.clone())
            .collect::<Vec<_>>();

        let pool = create_connection_pool_and_migrate(":memory:".to_string());

        let receipt_store = Arc::new(DieselReceiptStore::new(pool, Some("event_iterator".into())));

        receipt_store
            .add_txn_receipts(receipts.clone())
            .expect("failed to add receipts to store");

        // Test without a specified start
        let all_events = Events::new(receipt_store.clone(), None)
            .expect("failed to get iterator for all events");

        let all_event_ids = all_events.map(|event| event.id.clone()).collect::<Vec<_>>();
        assert_eq!(all_event_ids, receipt_ids);

        // Test with a specified start
        let some_events = Events::new(receipt_store, Some(receipt_ids[0].clone()))
            .expect("failed to get iterator for some events");

        let some_event_ids = some_events
            .map(|event| event.id.clone())
            .collect::<Vec<_>>();
        assert_eq!(some_event_ids, receipt_ids[1..].to_vec());
    }

    /// Verify that the `ScabbardState::get_state_at_address` method works properly.
    ///
    /// 1. Initialize a new, empty `ScabbardState`.
    /// 2. Set the value for a single address in state.
    /// 3. Get the value at the set address and verify it matches the value that was set.
    /// 4. Get the value at an unset address and verify that `None` is returned, which indicates
    ///    that the address is unset.
    #[test]
    fn get_state_at_address() {
        // Initialize state
        let receipt_store = Arc::new(DieselReceiptStore::new(
            create_connection_pool_and_migrate(":memory:".to_string()),
            None,
        ));

        let db = create_btree_db();
        let merkle_state = MerkleState::new(MerkleStateConfig::key_value(db.clone_box()))
            .expect("Unable to create merkle state");
        let commit_hash_store = TransactCommitHashStore::new(db);

        let mut state = ScabbardState::new(
            merkle_state,
            true,
            Arc::new(commit_hash_store),
            receipt_store,
            #[cfg(feature = "metrics")]
            "svc0".to_string(),
            #[cfg(feature = "metrics")]
            "vzrQS-rvwf4".to_string(),
            vec![],
        )
        .expect("Failed to initialize state");

        state.start_executor().expect("Failed to start executor");

        // Set a value in state
        let address = "abcdef".to_string();
        let value = b"value".to_vec();

        let signing_context = Secp256k1Context::new();
        let signer = signing_context.new_signer(signing_context.new_random_private_key());
        let batch = CommandTransactionBuilder::new()
            .with_commands(vec![Command::SetState(SetState::new(vec![
                BytesEntry::new(address.clone(), value.clone()),
            ]))])
            .into_transaction_builder()
            .expect("failed to convert to transaction builder")
            .into_batch_builder(&*signer)
            .expect("failed to build transaction")
            .build_pair(&*signer)
            .expect("Failed to build batch");
        state
            .prepare_change(batch)
            .expect("Failed to prepare change");
        state.commit().expect("Failed to commit change");

        // Get the value and verify it
        assert_eq!(
            state
                .get_state_at_address(&address)
                .expect("Failed to get state for set address"),
            Some(value),
        );

        // Get state at an unset address and verify it
        assert_eq!(
            state
                .get_state_at_address("0123456789")
                .expect("Failed to get state for unset address"),
            None,
        );

        state.stop_executor();
    }

    /// Verify that the `ScabbardState::get_state_with_prefix` method works properly.
    ///
    /// 1. Initialize a new, empty `ScabbardState`.
    /// 2. Set some values in state; 2 with a shared prefix, and 1 without.
    /// 3. Call `get_state_with_prefix(None)` to get all state entries and verify that all the
    ///    entries that were set are included in the result (there may be other entries because
    ///    the `ScabbardState` contstructor sets some state).
    /// 4. Call `get_state_with_prefix` with the shared prefix and verify that only the 2 entries
    ///    under that prefix are returned.
    /// 5. Call `get_state_with_prefix` with a prefix under which no addresses are set and verify
    ///    that no entries are returned (the iterator is empty).
    #[test]
    fn get_state_with_prefix() {
        let receipt_store = Arc::new(DieselReceiptStore::new(
            create_connection_pool_and_migrate(":memory:".to_string()),
            None,
        ));

        let db = create_btree_db();
        let merkle_state = MerkleState::new(MerkleStateConfig::key_value(db.clone_box()))
            .expect("Unable to create merkle state");
        let commit_hash_store = TransactCommitHashStore::new(db);

        let mut state = ScabbardState::new(
            merkle_state,
            true,
            Arc::new(commit_hash_store),
            receipt_store,
            #[cfg(feature = "metrics")]
            "svc0".to_string(),
            #[cfg(feature = "metrics")]
            "vzrQS-rvwf4".to_string(),
            vec![],
        )
        .expect("Failed to initialize state");

        state.start_executor().expect("Failed to start executor");

        // Set some values in state
        let prefix = "abcdef".to_string();

        let address1 = format!("{}01", prefix);
        let value1 = b"value1".to_vec();
        let address2 = format!("{}02", prefix);
        let value2 = b"value2".to_vec();
        let address3 = "0123456789".to_string();
        let value3 = b"value3".to_vec();

        let signing_context = Secp256k1Context::new();
        let signer = signing_context.new_signer(signing_context.new_random_private_key());
        let batch = CommandTransactionBuilder::new()
            .with_commands(vec![Command::SetState(SetState::new(vec![
                BytesEntry::new(address1.clone(), value1.clone()),
                BytesEntry::new(address2.clone(), value2.clone()),
                BytesEntry::new(address3.clone(), value3.clone()),
            ]))])
            .into_transaction_builder()
            .expect("failed to convert to transaction builder")
            .into_batch_builder(&*signer)
            .expect("failed to build transaction")
            .build_pair(&*signer)
            .expect("Failed to build batch");
        state
            .prepare_change(batch)
            .expect("Failed to prepare change");
        state.commit().expect("Failed to commit change");

        // Get all state entries and verify that they're correctly returned
        let all_entries = state
            .get_state_with_prefix(None)
            .expect("Failed to get all entries")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect all entries");
        assert!(all_entries.contains(&(address1.clone(), value1.clone())));
        assert!(all_entries.contains(&(address2.clone(), value2.clone())));
        assert!(all_entries.contains(&(address3, value3)));

        // Get state entries under the shared prefix and verify the correct entries are returned
        let prefix_entries = state
            .get_state_with_prefix(Some(&prefix))
            .expect("Failed to get entries under prefix")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect entries under prefix");
        assert_eq!(prefix_entries.len(), 2);
        assert!(prefix_entries.contains(&(address1, value1)));
        assert!(prefix_entries.contains(&(address2, value2)));

        // Get state entries under a prefix with no set addresses and verify that no entries are
        // returned
        let no_entries = state
            .get_state_with_prefix(Some("abcdef0123456789"))
            .expect("Failed to get entries under unset prefix")
            .collect::<Result<Vec<_>, _>>()
            .expect("Failed to collect entries under unset prefix");
        assert!(no_entries.is_empty());

        state.stop_executor();
    }

    fn mock_transaction_receipt(id: &str) -> TransactionReceipt {
        TransactionReceipt {
            transaction_id: id.into(),
            transaction_result: TransactionResult::Valid {
                state_changes: vec![],
                events: vec![],
                data: vec![],
            },
        }
    }

    fn create_connection_pool_and_migrate(
        connection_string: String,
    ) -> Pool<ConnectionManager<SqliteConnection>> {
        let connection_manager = ConnectionManager::<SqliteConnection>::new(connection_string);
        let pool = Pool::builder()
            .max_size(1)
            .build(connection_manager)
            .expect("Failed to build connection pool");

        run_sqlite_migrations(&*pool.get().expect("Failed to get connection for migrations"))
            .expect("Failed to run migrations");

        pool
    }

    fn create_btree_db() -> BTreeDatabase {
        let mut indexes = INDEXES.to_vec();
        indexes.push(CURRENT_STATE_ROOT_INDEX);
        BTreeDatabase::new(&indexes)
    }

    #[test]
    fn batch_history_correctly_fetches_batch_info() {
        let mut history = BatchHistory::new();
        history.add_batch("batch-id-1");
        history.add_batch("batch-id-2");

        // Add one batch id that we know is not part of the set (batch-id-3)
        let ids: HashSet<String> = vec!["batch-id-1", "batch-id-2", "batch-id-3"]
            .into_iter()
            .map(String::from)
            .collect::<HashSet<_>>();
        let duration = Duration::from_secs(0);
        let result = history
            .get_batch_info(ids, Some(duration))
            .expect("received unexpected error");
        let results: HashMap<String, BatchStatus> = result
            .map(|result| {
                let result = result.unwrap();
                (result.id, result.status)
            })
            .into_iter()
            .collect();

        // The items scabbard is aware of should be Pending
        assert_eq!(results.get("batch-id-1").unwrap(), &BatchStatus::Pending);
        assert_eq!(results.get("batch-id-2").unwrap(), &BatchStatus::Pending);

        // The item that has timed out should be Unknown
        assert_eq!(results.get("batch-id-3").unwrap(), &BatchStatus::Unknown);

        // Validate there are no extra items
        assert_eq!(results.values().count(), 3);
    }

    #[test]
    fn batch_status_deserializes_correctly() {
        assert_eq!(
            serde_json::from_str::<BatchStatus>(
                r#"{
              "statusType": "Unknown",
              "message": []
            }"#
            )
            .expect("could not deserialize"),
            BatchStatus::Unknown,
        );

        assert_eq!(
            serde_json::from_str::<BatchStatus>(
                r#"{
              "statusType": "Pending",
              "message": []
            }"#
            )
            .expect("could not deserialize"),
            BatchStatus::Pending,
        );

        assert_eq!(
            serde_json::from_str::<BatchStatus>(
                r#"{
              "statusType": "Invalid",
              "message": [{
                "transaction_id": "txid",
                "error_message": "message",
                "error_data": [
                    0,
                    1,
                    2
                ]
              }]
            }"#
            )
            .expect("could not deserialize"),
            BatchStatus::Invalid(vec![InvalidTransaction {
                transaction_id: String::from("txid"),
                error_message: String::from("message"),
                error_data: vec![0, 1, 2]
            }]),
        );

        assert_eq!(
            serde_json::from_str::<BatchStatus>(
                r#"{
              "statusType": "Valid",
              "message": [{
                "transaction_id": "txid"
              }]
            }"#
            )
            .expect("could not deserialize"),
            BatchStatus::Valid(vec![ValidTransaction {
                transaction_id: String::from("txid")
            }]),
        );

        assert_eq!(
            serde_json::from_str::<BatchStatus>(
                r#"{
              "statusType": "Committed",
              "message": [{
                "transaction_id": "txid"
              }]
            }"#
            )
            .expect("could not deserialize"),
            BatchStatus::Committed(vec![ValidTransaction {
                transaction_id: String::from("txid")
            }]),
        );
    }
}
