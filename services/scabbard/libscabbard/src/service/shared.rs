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
use transact::protocol::batch::BatchPair;
use transact::protocol::transaction::{HashMethod, TransactionHeader};
use transact::protos::FromBytes;

use splinter::{
    consensus::{PeerId, ProposalId},
    service::ServiceNetworkSender,
};

use crate::hex::parse_hex;

use super::error::ScabbardError;

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
    /// Tracks which batches are currently being evaluated, indexed by corresponding proposal IDs.
    proposed_batches: HashMap<ProposalId, BatchPair>,
    signature_verifier: Box<dyn SignatureVerifier>,
}

impl ScabbardShared {
    pub fn new(
        batch_queue: VecDeque<BatchPair>,
        network_sender: Option<Box<dyn ServiceNetworkSender>>,
        peer_services: HashSet<String>,
        service_id: String,
        signature_verifier: Box<dyn SignatureVerifier>,
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

        ScabbardShared {
            batch_queue,
            network_sender,
            peer_services,
            coordinator_service_id,
            service_id,
            proposed_batches: HashMap::new(),
            signature_verifier,
        }
    }

    /// Determines if this service is the coordinator.
    pub fn is_coordinator(&self) -> bool {
        self.service_id == self.coordinator_service_id
    }

    /// Gets the service ID of the two-phase commit coordinator.
    pub fn coordinator_service_id(&self) -> &str {
        &self.coordinator_service_id
    }

    pub fn add_batch_to_queue(&mut self, batch: BatchPair) {
        self.batch_queue.push_back(batch)
    }

    pub fn pop_batch_from_queue(&mut self) -> Option<BatchPair> {
        self.batch_queue.pop_front()
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

    pub fn add_proposed_batch(
        &mut self,
        proposal_id: ProposalId,
        batch: BatchPair,
    ) -> Option<BatchPair> {
        self.proposed_batches.insert(proposal_id, batch)
    }

    pub fn get_proposed_batch(&self, proposal_id: &ProposalId) -> Option<&BatchPair> {
        self.proposed_batches.get(proposal_id)
    }

    pub fn remove_proposed_batch(&mut self, proposal_id: &ProposalId) -> Option<BatchPair> {
        self.proposed_batches.remove(&proposal_id)
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
            context.new_verifier(),
        );
        assert!(coordinator_shared.is_coordinator());
        assert_eq!(coordinator_shared.coordinator_service_id(), "svc0");

        let non_coordinator_shared = ScabbardShared::new(
            VecDeque::new(),
            Some(Box::new(MockServiceNetworkSender)),
            peer_services,
            "svc3".to_string(),
            context.new_verifier(),
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
    }
}
