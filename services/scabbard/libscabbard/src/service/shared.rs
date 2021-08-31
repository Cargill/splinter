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

use cylinder::{PublicKey, Signature, Verifier as SignatureVerifier};
use openssl::hash::{hash, MessageDigest};
#[cfg(feature = "back-pressure")]
use protobuf::Message;
use transact::protocol::batch::BatchPair;
use transact::protocol::transaction::{HashMethod, TransactionHeader};
use transact::protos::FromBytes;

use splinter::{
    consensus::{PeerId, Proposal, ProposalId},
    service::ServiceNetworkSender,
};

use crate::hex::parse_hex;
#[cfg(feature = "back-pressure")]
use crate::protos::scabbard::{ScabbardMessage, ScabbardMessage_Type};

use super::error::ScabbardError;
#[cfg(feature = "back-pressure")]
use super::ScabbardVersion;

#[cfg(feature = "back-pressure")]
const DEFAULT_PENDING_BATCH_LIMIT: usize = 30;

/// Data structure used to store information that's shared between components in this service
pub struct ScabbardShared {
    /// Queue of batches that have been submitted locally via the REST API, but have not yet been
    /// proposed.
    batch_queue: VecDeque<BatchPair>,
    /// Used to send messages to other services; set when the service is started and unset when the
    /// service is stopped.
    network_sender: Option<Box<dyn ServiceNetworkSender>>,
    /// List of service IDs that this service is configured to communicate and share state with.
    peer_services: HashSet<String>,
    /// The two-phase commit coordinator. This is the service that will create all proposals, so all
    /// submitted batches should be sent to this service.
    coordinator_service_id: String,
    /// This service's ID
    service_id: String,
    /// This circuit's ID
    #[cfg(feature = "metrics")]
    circuit_id: String,
    /// Tracks which proposals are currently being evaluated along with the batch the proposal is
    /// for
    open_proposals: HashMap<ProposalId, (Proposal, BatchPair)>,
    signature_verifier: Box<dyn SignatureVerifier>,
    /// Whether scabbard is currently accepting new batches, a part of back pressure
    #[cfg(feature = "back-pressure")]
    accepting_batches: bool,
    #[cfg(feature = "back-pressure")]
    scabbard_version: ScabbardVersion,
}

impl ScabbardShared {
    pub fn new(
        batch_queue: VecDeque<BatchPair>,
        network_sender: Option<Box<dyn ServiceNetworkSender>>,
        peer_services: HashSet<String>,
        service_id: String,
        #[cfg(feature = "metrics")] circuit_id: String,
        signature_verifier: Box<dyn SignatureVerifier>,
        #[cfg(feature = "back-pressure")] scabbard_version: ScabbardVersion,
    ) -> Self {
        // The two-phase commit coordinator is the node with the lowest peer ID. Peer IDs are
        // computed from service IDs.
        let coordinator_service_id = String::from_utf8(
            peer_services
                .iter()
                .chain(std::iter::once(&service_id))
                .map(|service_id| PeerId::from(service_id.as_bytes()))
                .min()
                .expect("There will always be at least one service (self)")
                .into(),
        )
        .expect("String -> PeerId -> String conversion should not fail");

        let scabbard_shared = ScabbardShared {
            batch_queue,
            network_sender,
            peer_services,
            coordinator_service_id,
            service_id,
            #[cfg(feature = "metrics")]
            circuit_id,
            open_proposals: HashMap::new(),
            signature_verifier,
            #[cfg(feature = "back-pressure")]
            accepting_batches: true,
            #[cfg(feature = "back-pressure")]
            scabbard_version,
        };

        // initialize pending_batches metric
        scabbard_shared.update_pending_batches(0);

        scabbard_shared
    }

    /// Determines if this service is the coordinator.
    pub fn is_coordinator(&self) -> bool {
        self.service_id == self.coordinator_service_id
    }

    /// Gets the service ID of the two-phase commit coordinator.
    pub fn coordinator_service_id(&self) -> &str {
        &self.coordinator_service_id
    }

    /// set whether we are accepting new batches
    #[cfg(feature = "back-pressure")]
    pub fn set_accepting_batches(&mut self, accepting: bool) {
        self.accepting_batches = accepting;
    }

    #[cfg(feature = "back-pressure")]
    pub fn accepting_batches(&self) -> bool {
        self.accepting_batches
    }

    /// Updates pending batches metrics gauge
    ///
    /// # Arguments
    ///
    /// * `_batches` - The number of pending batches for this service. It is prefixed with an
    /// underscore due to rust recognizing the metrics macro noop when the metrics feature is
    /// disabled
    fn update_pending_batches(&self, _batches: i64) {
        gauge!(
            "splinter.scabbard.pending_batches",
            _batches,
            "service" => format!("{}::{}", self.circuit_id, self.service_id)
        );
    }

    pub fn add_batch_to_queue(&mut self, batch: BatchPair) -> Result<(), ScabbardError> {
        self.batch_queue.push_back(batch);
        self.update_pending_batches(self.batch_queue.len() as i64);

        #[cfg(feature = "back-pressure")]
        {
            // only the coordinator should change accepting batches and
            // back pressure is not supported by V1
            if !self.is_coordinator() || self.scabbard_version == ScabbardVersion::V1 {
                return Ok(());
            };

            // Check whether the pending batch queue has gotten too big and back pressure
            // should be enabled.
            if self.accepting_batches && self.batch_queue.len() >= DEFAULT_PENDING_BATCH_LIMIT {
                self.set_accepting_batches(false);
                // notify non_coordinators not to send new batches
                let mut msg = ScabbardMessage::new();
                msg.set_message_type(ScabbardMessage_Type::TOO_MANY_REQUESTS);
                let msg_bytes = msg
                    .write_to_bytes()
                    .map_err(|err| ScabbardError::Internal(Box::new(err)))?;

                for service in self.peer_services() {
                    self.network_sender()
                        .ok_or(ScabbardError::NotConnected)?
                        .send(service, msg_bytes.as_slice())
                        .map_err(|err| ScabbardError::Internal(Box::new(err)))?;
                }
            }
        }
        Ok(())
    }

    pub fn pop_batch_from_queue(&mut self) -> Result<Option<BatchPair>, ScabbardError> {
        let batch = self.batch_queue.pop_front();

        // if the batch is some, the length of pending batches has changed
        if batch.is_some() {
            self.update_pending_batches(self.batch_queue.len() as i64);
        }

        #[cfg(feature = "back-pressure")]
        {
            // only the coordinator should change accepting batches and
            // back pressure is not supported by V1
            if !self.is_coordinator() || self.scabbard_version == ScabbardVersion::V1 {
                return Ok(batch);
            };

            // If back pressure was enabled, only start accepting transactions again if the queue has
            // dropped to half the pending batch limit
            if !self.accepting_batches && self.batch_queue.len() < DEFAULT_PENDING_BATCH_LIMIT / 2 {
                self.set_accepting_batches(true);

                // notify non_coordinators that we are accepting batches now
                let mut msg = ScabbardMessage::new();
                msg.set_message_type(ScabbardMessage_Type::ACCEPTING_REQUESTS);
                let msg_bytes = msg
                    .write_to_bytes()
                    .map_err(|err| ScabbardError::Internal(Box::new(err)))?;

                for service in self.peer_services() {
                    self.network_sender()
                        .ok_or(ScabbardError::NotConnected)?
                        .send(service, msg_bytes.as_slice())
                        .map_err(|err| ScabbardError::Internal(Box::new(err)))?;
                }
            }
        }

        Ok(batch)
    }

    pub fn network_sender(&self) -> Option<&dyn ServiceNetworkSender> {
        self.network_sender.as_deref()
    }

    pub fn set_network_sender(&mut self, sender: Box<dyn ServiceNetworkSender>) {
        self.network_sender = Some(sender)
    }

    pub fn take_network_sender(&mut self) -> Option<Box<dyn ServiceNetworkSender>> {
        self.network_sender.take()
    }

    pub fn peer_services(&self) -> &HashSet<String> {
        &self.peer_services
    }

    pub fn add_open_proposal(&mut self, proposal: Proposal, batch: BatchPair) {
        self.open_proposals
            .insert(proposal.id.clone(), (proposal, batch));
    }

    pub fn get_open_proposal(&self, proposal_id: &ProposalId) -> Option<&(Proposal, BatchPair)> {
        self.open_proposals.get(proposal_id)
    }

    pub fn remove_open_proposal(&mut self, proposal_id: &ProposalId) {
        self.open_proposals.remove(proposal_id);
    }

    pub fn verify_batches(&self, batches: &[BatchPair]) -> Result<bool, ScabbardError> {
        for batch in batches {
            let batch_pub_key = batch.header().signer_public_key();

            // Verify batch signature
            if !self
                .signature_verifier
                .verify(
                    batch.batch().header(),
                    &Signature::from_hex(batch.batch().header_signature())
                        .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?,
                    &PublicKey::new(batch_pub_key.to_vec()),
                )
                .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?
            {
                warn!(
                    "Batch failed signature verification: {}",
                    batch.batch().header_signature()
                );
                return Ok(false);
            }

            // Verify list of txn IDs in the batch header matches the txns in the batch (verify
            // length here, then verify IDs as each txn is verified)
            if batch.header().transaction_ids().len() != batch.batch().transactions().len() {
                warn!(
                    "Number of transactions in batch header does not match number of transactions
                     in batch: {}",
                    batch.batch().header_signature(),
                );
                return Ok(false);
            }

            // Verify all transactions in batch
            for (i, txn) in batch.batch().transactions().iter().enumerate() {
                let header = TransactionHeader::from_bytes(txn.header())
                    .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?;

                // Verify this transaction matches the corresponding ID in the batch header
                let header_signature_bytes = parse_hex(txn.header_signature())
                    .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?;
                if header_signature_bytes != batch.header().transaction_ids()[i] {
                    warn!(
                        "Transaction at index {} does not match corresponding transaction ID in
                         batch header: {}",
                        i,
                        batch.batch().header_signature(),
                    );
                    return Ok(false);
                }

                if header.batcher_public_key() != batch_pub_key {
                    warn!(
                        "Transaction batcher public key does not match batch signer public key -
                         txn: {}, batch: {}",
                        txn.header_signature(),
                        batch.batch().header_signature(),
                    );
                    return Ok(false);
                }

                if !self
                    .signature_verifier
                    .verify(
                        txn.header(),
                        &Signature::from_hex(txn.header_signature())
                            .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?,
                        &PublicKey::new(header.signer_public_key().to_vec()),
                    )
                    .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?
                {
                    warn!(
                        "Transaction failed signature verification - txn: {}, batch: {}",
                        txn.header_signature(),
                        batch.batch().header_signature()
                    );
                    return Ok(false);
                }

                if !match header.payload_hash_method() {
                    HashMethod::SHA512 => {
                        let expected_hash = hash(MessageDigest::sha512(), txn.payload())
                            .map_err(|err| ScabbardError::BatchVerificationFailed(Box::new(err)))?;
                        header.payload_hash() == expected_hash.as_ref()
                    }
                } {
                    warn!(
                        "Transaction payload hash doesn't match payload - txn: {}, batch: {}",
                        txn.header_signature(),
                        batch.batch().header_signature()
                    );
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cylinder::{secp256k1::Secp256k1Context, VerifierFactory};
    use splinter::service::{ServiceMessageContext, ServiceSendError};

    /// Verifies that the `is_coordinator` and `coordinator_service_id` methods work properly.
    ///
    /// 1. Create a `ScabbardShared` instance for a coordinator (has a service ID lower than all of
    ///    its peers) and verify that it properly determines that the coordinator is itself.
    /// 2. Create a `ScabbardShared` instance for a non-coordinator (does not have a service ID
    ///    lower than all of its peers) and verify that it properly determines who the coordinator
    ///    is (not itself).
    #[test]
    fn coordinator() {
        let context = Secp256k1Context::new();

        let mut peer_services = HashSet::new();
        peer_services.insert("svc1".to_string());
        peer_services.insert("svc2".to_string());

        let coordinator_shared = ScabbardShared::new(
            VecDeque::new(),
            Some(Box::new(MockServiceNetworkSender)),
            peer_services.clone(),
            "svc0".to_string(),
            #[cfg(feature = "metrics")]
            "vzrQS-rvwf4".to_string(),
            context.new_verifier(),
            #[cfg(feature = "back-pressure")]
            ScabbardVersion::V2,
        );
        assert!(coordinator_shared.is_coordinator());
        assert_eq!(coordinator_shared.coordinator_service_id(), "svc0");

        let non_coordinator_shared = ScabbardShared::new(
            VecDeque::new(),
            Some(Box::new(MockServiceNetworkSender)),
            peer_services,
            "svc3".to_string(),
            #[cfg(feature = "metrics")]
            "vzrQS-rvwf4".to_string(),
            context.new_verifier(),
            #[cfg(feature = "back-pressure")]
            ScabbardVersion::V2,
        );
        assert!(!non_coordinator_shared.is_coordinator());
        assert_eq!(non_coordinator_shared.coordinator_service_id(), "svc1");
    }

    #[derive(Clone, Debug)]
    pub struct MockServiceNetworkSender;

    impl ServiceNetworkSender for MockServiceNetworkSender {
        fn send(&self, _recipient: &str, _message: &[u8]) -> Result<(), ServiceSendError> {
            unimplemented!()
        }

        fn send_and_await(
            &self,
            _recipient: &str,
            _message: &[u8],
        ) -> Result<Vec<u8>, ServiceSendError> {
            unimplemented!()
        }

        fn reply(
            &self,
            _message_origin: &ServiceMessageContext,
            _message: &[u8],
        ) -> Result<(), ServiceSendError> {
            unimplemented!()
        }

        fn clone_box(&self) -> Box<dyn ServiceNetworkSender> {
            Box::new(self.clone())
        }

        #[cfg(feature = "challenge-authorization")]
        fn send_with_sender(
            &mut self,
            _recipient: &str,
            _message: &[u8],
            _sender: &str,
        ) -> Result<(), ServiceSendError> {
            Ok(())
        }
    }
}
