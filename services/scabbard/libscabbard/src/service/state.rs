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

use std::collections::{HashMap, HashSet, VecDeque};
use std::convert::TryFrom;
use std::fmt;
use std::path::Path;
use std::sync::{
    mpsc::{channel, Receiver, RecvTimeoutError, Sender},
    Arc, RwLock,
};
use std::time::{Duration, Instant, SystemTime};

use protobuf::Message;
use sawtooth::store::{lmdb::LmdbOrderedStore, receipt_store::TransactionReceiptStore};
use sawtooth_sabre::{
    handler::SabreTransactionHandler, ADMINISTRATORS_SETTING_ADDRESS, ADMINISTRATORS_SETTING_KEY,
};
#[cfg(feature = "events")]
use splinter::events::{ParseBytes, ParseError};
#[cfg(test)]
use transact::families::command::CommandTransactionHandler;
use transact::{
    context::manager::sync::ContextManager,
    database::{
        lmdb::{LmdbContext, LmdbDatabase},
        Database,
    },
    execution::{adapter::static_adapter::StaticExecutionAdapter, executor::Executor},
    protocol::{
        batch::BatchPair,
        receipt::{TransactionReceipt, TransactionResult},
    },
    sawtooth::SawtoothToTransactHandlerAdapter,
    scheduler::{serial::SerialScheduler, BatchExecutionResult, Scheduler},
    state::{
        merkle::{MerkleRadixTree, MerkleState, StateDatabaseError, INDEXES},
        StateChange as TransactStateChange, Write,
    },
};

use crate::hex;
use crate::protos::scabbard::{Setting, Setting_Entry};

use super::error::{ScabbardStateError, StateSubscriberError};

const EXECUTION_TIMEOUT: u64 = 300; // five minutes
const CURRENT_STATE_ROOT_INDEX: &str = "current_state_root";
const ITER_CACHE_SIZE: usize = 64;
const COMPLETED_BATCH_INFO_ITER_RETRY_MILLIS: u64 = 100;
const DEFAULT_BATCH_HISTORY_SIZE: usize = 100;

/// Iterator over entries in a Scabbard service's state
pub type StateIter = Box<dyn Iterator<Item = Result<(String, Vec<u8>), ScabbardStateError>>>;

pub struct ScabbardState {
    db: Box<dyn Database>,
    context_manager: ContextManager,
    executor: Executor,
    current_state_root: String,
    transaction_receipt_store: Arc<RwLock<TransactionReceiptStore>>,
    pending_changes: Option<(String, Vec<TransactionReceipt>)>,
    event_subscribers: Vec<Box<dyn StateSubscriber>>,
    batch_history: BatchHistory,
}

impl ScabbardState {
    pub fn new(
        state_db_path: &Path,
        state_db_size: usize,
        receipt_db_path: &Path,
        receipt_db_size: usize,
        admin_keys: Vec<String>,
    ) -> Result<Self, ScabbardStateError> {
        // Initialize the database
        let mut indexes = INDEXES.to_vec();
        indexes.push(CURRENT_STATE_ROOT_INDEX);
        let db = Box::new(LmdbDatabase::new(
            LmdbContext::new(state_db_path, indexes.len(), Some(state_db_size))?,
            &indexes,
        )?);

        let current_state_root = if let Some(current_state_root) =
            Self::read_current_state_root(&*db)?
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

            let initial_state_root = MerkleRadixTree::new(db.clone_box(), None)?.get_merkle_root();
            MerkleState::new(db.clone()).commit(
                &initial_state_root,
                vec![admin_keys_state_change].as_slice(),
            )?
        };

        // Initialize transact
        let context_manager = ContextManager::new(Box::new(MerkleState::new(db.clone())));
        let mut executor = Executor::new(vec![Box::new(StaticExecutionAdapter::new_adapter(
            vec![
                Box::new(SawtoothToTransactHandlerAdapter::new(
                    SabreTransactionHandler::new(),
                )),
                #[cfg(test)]
                Box::new(CommandTransactionHandler::new()),
            ],
            context_manager.clone(),
        )?)]);
        executor
            .start()
            .map_err(|err| ScabbardStateError(format!("failed to start executor: {}", err)))?;

        Ok(ScabbardState {
            db,
            context_manager,
            executor,
            current_state_root,
            transaction_receipt_store: Arc::new(RwLock::new(TransactionReceiptStore::new(
                Box::new(
                    LmdbOrderedStore::new(receipt_db_path, Some(receipt_db_size))
                        .map_err(|err| ScabbardStateError(err.to_string()))?,
                ),
            ))),
            pending_changes: None,
            event_subscribers: vec![],
            batch_history: BatchHistory::new(),
        })
    }

    fn read_current_state_root(db: &dyn Database) -> Result<Option<String>, ScabbardStateError> {
        db.get_reader()
            .and_then(|reader| reader.index_get(CURRENT_STATE_ROOT_INDEX, b"HEAD"))
            .map(|head| head.map(|bytes| hex::to_hex(&bytes)))
            .map_err(|e| ScabbardStateError(format!("Unable to read HEAD entry: {}", e)))
    }

    fn write_current_state_root(&self) -> Result<(), ScabbardStateError> {
        let current_root_bytes = hex::parse_hex(&self.current_state_root).map_err(|e| {
            ScabbardStateError(format!(
                "The in-memory current state root is invalid: {}",
                e
            ))
        })?;

        let mut writer = self.db.get_writer().map_err(|e| {
            ScabbardStateError(format!(
                "Unable to start write transaction for HEAD entry: {}",
                e
            ))
        })?;

        writer
            .index_put(CURRENT_STATE_ROOT_INDEX, b"HEAD", &current_root_bytes)
            .map_err(|e| ScabbardStateError(format!("Unable to write HEAD entry: {}", e)))?;

        writer
            .commit()
            .map_err(|e| ScabbardStateError(format!("Unable to commit HEAD entry: {}", e)))?;

        Ok(())
    }

    /// Fetch the value at the given `address` in state. Returns `None` if the `address` is not set.
    pub fn get_state_at_address(
        &self,
        address: &str,
    ) -> Result<Option<Vec<u8>>, ScabbardStateError> {
        Ok(
            MerkleRadixTree::new(self.db.clone(), Some(&self.current_state_root))?
                .get_value(address)?,
        )
    }

    /// Fetch a list of entries in state. If a `prefix` is provided, only return entries whose
    /// addresses are under the given address prefix. If no `prefix` is provided, return all state
    /// entries.
    pub fn get_state_with_prefix(
        &self,
        prefix: Option<&str>,
    ) -> Result<StateIter, ScabbardStateError> {
        Ok(Box::new(
            MerkleRadixTree::new(self.db.clone(), Some(&self.current_state_root))?
                .leaves(prefix)
                .or_else(|err| match err {
                    StateDatabaseError::NotFound(_) => Ok(Box::new(std::iter::empty())),
                    err => Err(err),
                })?
                .map(|res| res.map_err(ScabbardStateError::from)),
        ))
    }

    /// Get the current state root hash.
    pub fn current_state_root(&self) -> &str {
        &self.current_state_root
    }

    pub fn prepare_change(&mut self, batch: BatchPair) -> Result<String, ScabbardStateError> {
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
        self.executor
            .execute(scheduler.take_task_iterator()?, scheduler.new_notifier()?)?;

        // Get the results and shutdown the scheduler
        let recv_result = result_rx.recv_timeout(Duration::from_secs(EXECUTION_TIMEOUT));

        let batch_result = recv_result
            .map_err(|_| ScabbardStateError("failed to receive result in reasonable time".into()))?
            .ok_or_else(|| ScabbardStateError("no result returned from executor".into()))?;

        let batch_status = batch_result.clone().into();
        let signature = batch.batch().header_signature();
        self.batch_history
            .update_batch_status(&signature, batch_status);

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
        let state_root = MerkleState::new(self.db.clone()).compute_state_id(
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
                self.current_state_root = MerkleState::new(self.db.clone())
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

                self.transaction_receipt_store
                    .write()
                    .map_err(|err| {
                        ScabbardStateError(format!(
                            "transaction receipt store lock poisoned: {}",
                            err
                        ))
                    })?
                    .append(txn_receipts)
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
        Events::new(self.transaction_receipt_store.clone(), event_id)
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
                format!("cannot convert transaction receipt ({}) to state cahnge event because transction result is `Invalid`", transaction_id)
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

/// An iterator that wraps the `TransactionReceiptStore` and returns `StateChangeEvent`s using an
/// in-memory cache.
pub struct Events {
    transaction_receipt_store: Arc<RwLock<TransactionReceiptStore>>,
    query: EventQuery,
    cache: VecDeque<StateChangeEvent>,
}

impl Events {
    fn new(
        transaction_receipt_store: Arc<RwLock<TransactionReceiptStore>>,
        start_id: Option<String>,
    ) -> Result<Self, ScabbardStateError> {
        let mut iter = Events {
            transaction_receipt_store,
            query: EventQuery::Fetch(start_id),
            cache: VecDeque::default(),
        };
        iter.reload_cache()?;
        Ok(iter)
    }

    fn reload_cache(&mut self) -> Result<(), ScabbardStateError> {
        match self.query {
            EventQuery::Fetch(ref start_id) => {
                let transaction_receipt_store =
                    self.transaction_receipt_store.read().map_err(|err| {
                        ScabbardStateError(format!(
                            "transaction receipt store lock poisoned: {}",
                            err
                        ))
                    })?;

                self.cache = if let Some(id) = start_id.as_ref() {
                    transaction_receipt_store.iter_since_id(id.clone())
                } else {
                    transaction_receipt_store.iter()
                }
                .map_err(|err| {
                    ScabbardStateError(format!(
                        "failed to get transaction receipts from store: {}",
                        err
                    ))
                })?
                .take(ITER_CACHE_SIZE)
                .map(StateChangeEvent::try_from)
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "statusType", content = "message")]
pub enum BatchStatus {
    Unknown,
    Pending,
    Invalid(Vec<InvalidTransaction>),
    Valid(Vec<ValidTransaction>),
    Committed(Vec<ValidTransaction>),
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidTransaction {
    pub transaction_id: String,
}

impl ValidTransaction {
    fn new(transaction_id: String) -> Self {
        Self { transaction_id }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
            BatchStatus::Invalid(_) | BatchStatus::Valid(_) => {
                self.send_completed_batch_info_to_subscribers(batch_info)
            }
            _ => {}
        }
    }

    fn commit(&mut self, signature: &str) {
        match self.history.get_mut(signature) {
            Some(info) => match info.status.clone() {
                BatchStatus::Valid(txns) => {
                    info.set_status(BatchStatus::Committed(txns));
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
        // Get batches that are already completed
        let iter = self
            .no_wait_batch_info_iter(&ids)
            .filter_map(|res| {
                let info = res.ok()?;
                match info.status {
                    BatchStatus::Invalid(_) | BatchStatus::Committed(_) => {
                        ids.remove(&info.id);
                        Some(Ok(info))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
            .into_iter();

        let (sender, receiver) = channel();

        self.batch_subscribers.push((ids.clone(), sender));

        Ok(Box::new(
            iter.chain(ChannelBatchInfoIter::new(receiver, timeout, ids)?),
        ))
    }

    fn send_completed_batch_info_to_subscribers(&mut self, info: BatchInfo) {
        self.batch_subscribers = self
            .batch_subscribers
            .drain(..)
            .filter_map(|(mut pending_signatures, sender)| {
                if pending_signatures.remove(&info.id) && sender.send(info.clone()).is_err() {
                    // Receiver has been dropped
                    return None;
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
}

impl ChannelBatchInfoIter {
    fn new(
        receiver: Receiver<BatchInfo>,
        timeout: Duration,
        pending_ids: HashSet<String>,
    ) -> Result<Self, ScabbardStateError> {
        let timeout = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| ScabbardStateError("failed to schedule timeout".into()))?;

        Ok(Self {
            receiver,
            retry_interval: Duration::from_millis(COMPLETED_BATCH_INFO_ITER_RETRY_MILLIS),
            timeout,
            pending_ids,
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
            // Check if the timeout has expired
            if Instant::now() >= self.timeout {
                return Some(Err(format!(
                    "timeout expired while waiting for incompleted batches: {:?}",
                    self.pending_ids
                )));
            }
            // Check for the next BatchInfo
            match self.receiver.recv_timeout(self.retry_interval) {
                Ok(batch_info) => {
                    self.pending_ids.remove(&batch_info.id);
                    return Some(Ok(batch_info));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use cylinder::{secp256k1::Secp256k1Context, Context};
    use tempdir::TempDir;
    use transact::{
        families::command::make_command_transaction,
        protocol::{
            batch::BatchBuilder,
            command::{BytesEntry, Command, SetState},
        },
    };

    const TEMP_DB_SIZE: usize = 1 << 30; // 1024 ** 3

    /// Verify that an empty receipt store returns an empty iterator
    #[test]
    fn empty_event_iterator() {
        let paths = StatePaths::new("empty_event_iterator");

        let transaction_receipt_store =
            Arc::new(RwLock::new(TransactionReceiptStore::new(Box::new(
                LmdbOrderedStore::new(&paths.receipt_db_path, Some(TEMP_DB_SIZE))
                    .expect("Failed to create LMDB store"),
            ))));

        // Test without a specified start
        let all_events = Events::new(transaction_receipt_store.clone(), None)
            .expect("failed to get iterator for all events");
        let all_event_ids = all_events.map(|event| event.id.clone()).collect::<Vec<_>>();
        assert!(
            all_event_ids.is_empty(),
            "All events should have been empty"
        );
    }

    /// Verify that the event iterator works as expected.
    #[test]
    fn event_iterator() {
        let paths = StatePaths::new("event_iterator");

        let receipts = vec![
            mock_transaction_receipt("ab"),
            mock_transaction_receipt("cd"),
            mock_transaction_receipt("ef"),
        ];
        let receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.transaction_id.clone())
            .collect::<Vec<_>>();

        let transaction_receipt_store =
            Arc::new(RwLock::new(TransactionReceiptStore::new(Box::new(
                LmdbOrderedStore::new(&paths.receipt_db_path, Some(TEMP_DB_SIZE))
                    .expect("Failed to create LMDB store"),
            ))));

        transaction_receipt_store
            .write()
            .expect("failed to get write lock")
            .append(receipts.clone())
            .expect("failed to add receipts to store");

        // Test without a specified start
        let all_events = Events::new(transaction_receipt_store.clone(), None)
            .expect("failed to get iterator for all events");
        let all_event_ids = all_events.map(|event| event.id.clone()).collect::<Vec<_>>();
        assert_eq!(all_event_ids, receipt_ids);

        // Test with a specified start
        let some_events = Events::new(
            transaction_receipt_store.clone(),
            Some(receipt_ids[0].clone()),
        )
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
        let paths = StatePaths::new("get_state_at_address");
        let mut state = ScabbardState::new(
            &paths.state_db_path,
            TEMP_DB_SIZE,
            &paths.receipt_db_path,
            TEMP_DB_SIZE,
            vec![],
        )
        .expect("Failed to initialize state");

        // Set a value in state
        let address = "abcdef".to_string();
        let value = b"value".to_vec();

        let signing_context = Secp256k1Context::new();
        let signer = signing_context.new_signer(signing_context.new_random_private_key());
        let batch = BatchBuilder::new()
            .with_transactions(vec![
                make_command_transaction(
                    &[Command::SetState(SetState::new(vec![BytesEntry::new(
                        address.clone(),
                        value.clone(),
                    )]))],
                    &*signer,
                )
                .take()
                .0,
            ])
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
        // Initialize state
        let paths = StatePaths::new("get_state_at_address");
        let mut state = ScabbardState::new(
            &paths.state_db_path,
            TEMP_DB_SIZE,
            &paths.receipt_db_path,
            TEMP_DB_SIZE,
            vec![],
        )
        .expect("Failed to initialize state");

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
        let batch = BatchBuilder::new()
            .with_transactions(vec![
                make_command_transaction(
                    &[Command::SetState(SetState::new(vec![
                        BytesEntry::new(address1.clone(), value1.clone()),
                        BytesEntry::new(address2.clone(), value2.clone()),
                        BytesEntry::new(address3.clone(), value3.clone()),
                    ]))],
                    &*signer,
                )
                .take()
                .0,
            ])
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
    }

    struct StatePaths {
        _temp_dir_handle: TempDir,
        pub state_db_path: PathBuf,
        pub receipt_db_path: PathBuf,
    }

    impl StatePaths {
        fn new(prefix: &str) -> Self {
            let temp_dir = TempDir::new(prefix).expect("Failed to create temp dir");
            let state_db_path = temp_dir.path().join("state.lmdb");
            let receipt_db_path = temp_dir.path().join("receipts.lmdb");
            Self {
                _temp_dir_handle: temp_dir,
                state_db_path,
                receipt_db_path,
            }
        }
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
}
