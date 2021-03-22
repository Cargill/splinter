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

//! Integration tests for the lifecycle of a circuit between multiple nodes. These tests verify
//! the process of creating and then disbanding a circuit between two and three nodes.

use std::time::{Duration, Instant};

use splinterd::node::RestApiVariant;

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::{
    get_node_service_id,
    payload::{
        make_circuit_disband_payload, make_circuit_proposal_vote_payload,
        make_create_contract_registry_batch,
    },
};
use crate::framework::network::Network;

/// Test that a 2-party circuit may be created on a 2-node network. This test then validates the
/// circuit is able to be disbanded. This test also validates the Splinter services running on the
/// on the circuit behave as expected throughout the disband process. The service transactions
/// are expected to submit successfully while the services are still running, up until all circuit
/// members have agreed to disband the circuit. Once the circuit has been disbanded, the services
/// for the circuit are expected to have been stopped and service transaction submissions will
/// return an error. Furthermore, this test validates the disbanded circuit is still accessible to
/// each node and the circuit definition is as expected, after disbanding.
///
/// 1. Create and commit a circuit between 2 nodes
/// 2. Submit a Scabbard transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 3. Create and submit a `CircuitDisbandRequest` from the first node
/// 4. Wait until the disband proposal is available to the second node, using `list_proposals`
/// 5. Verify the same disband proposal is available on each node
/// 6. Submit the same `CircuitDisbandRequest` from the second step to the second node
/// 7. Validate this duplicate disband proposal is rejected
/// 8. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 9. Wait until the circuit is no longer available as an active circuit on the first node,
///    using `list_circuits`
/// 10. Validate the circuit is no longer active on every node
/// 11. Validate the disbanded circuit is still available to each node, though disbanded, and that
///    the disbanded circuit is the same for each node
/// 12. Submit a `Scabbard` transaction from one of the nodes to validate the batch is unable to be
///    committed on the disbanded circuit as the services set-up for this circuit have been stopped
#[test]
pub fn test_2_party_circuit_lifecycle() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");

    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(res.is_ok());
    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first proposal in this list as we've already validated the list is
            // not empty
            proposal_b = proposals.get(0).unwrap().clone();
        } else {
            continue;
        }

        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal) => proposal,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Submit a duplicate of the disband `CircuitManagementPayload` to the second node
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(duplicate_res.is_err());

    // Create the `ServiceId` struct based on the second node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_b = get_node_service_id(&circuit_id, node_b);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer());
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be removed from the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        // If the circuit no longer appears in the list of active circuits, the circuit
        // has been successfully disbanded.
        let circuits_a = node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from first node")
            .data;
        let circuits_b = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from second node")
            .data;
        if circuits_a.is_empty() && circuits_b.is_empty() {
            break;
        } else {
            continue;
        }
    }

    // Validate the disbanded circuit is the same for both nodes
    let disbanded_circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from first node")
        .unwrap();
    let disbanded_circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from second node")
        .unwrap();
    assert_eq!(disbanded_circuit_a, disbanded_circuit_b);

    // Submit a `CreateContractRegistryAction` to validate the service transaction returns an
    // error as the circuit has been disbanded, meaning both nodes' services running on that
    // circuit have been stopped
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_b.admin_signer());
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 2-party circuit may be created on a 2-node network. This test then validates a
/// circuit member is able to propose to disband the circuit. This test then validates the disband
/// request is able to be rejected by another circuit member, removing the disband proposal.
///
/// 1. Create and commit a circuit between 2 nodes
/// 2. Submit a `Scabbard` transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 3. Create and submit a `CircuitDisbandRequest` from the first node
/// 4. Wait until the disband proposal is available to the second node, using `list_proposals`
/// 5. Verify the same disband proposal is available on each node
/// 6. Create and submit a `CircuitProposalVote` from the second node to reject the disband proposal
/// 7. Wait until the disband proposal is no longer available to the second node,
///    using `list_proposals`
/// 8. Validate the proposal is no longer available on the nodes
/// 9. Validate the active circuit is still available to each node, using `list_circuits` which
///    only returns active circuits
/// 10. Submit a `Scabbard` transaction from one of the nodes to validate the service is still able
///    to commit the batch on the circuit that has remained active
#[test]
#[ignore]
pub fn test_2_party_circuit_disband_proposal_rejected() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(res.is_ok());

    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        if !proposals.is_empty() {
            // Unwrap the first proposal in this list as we've already validated the list is
            // not empty
            proposal_b = proposals.get(0).unwrap().clone();
        } else {
            continue;
        }

        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal_a) => proposal_a,
            None => continue,
        };

        assert_eq!(proposal_b, proposal_a);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        false,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the proposal to be removed from the first node
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        // If the proposal no longer appears in the list, the proposal has been removed as it was
        // rejected
        let proposals_a = node_a
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from first node")
            .data;
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        if proposals_a.is_empty() && proposals_b.is_empty() {
            break;
        }
    }

    // Validate the active circuit is still available to each node
    let active_circuits_a = node_a
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from first node")
        .data;
    assert!(active_circuits_a.len() == 1);
    let active_circuits_b = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from second node")
        .data;
    assert!(active_circuits_b.len() == 1);

    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be created on a 3-node network. This test then validates the
/// circuit is able to be disbanded. This test also validates that the Splinter services running
/// on the circuit behave as expected throughout the disband process. The service transactions
/// are expected to submit successfully while the services are still running, up until all circuit
/// members have agreed to disband the circuit. Once the circuit has been disbanded, the services
/// for the circuit are expected to have been stopped and service transaction submissions will
/// return an error.
///
/// 1. Create and commit a circuit between 3 nodes
/// 2. Create and submit a `CircuitDisbandRequest` from the first node
/// 3. Wait until the disband proposal is available to each node, using `list_proposals`
/// 4. Verify the same disband proposal is present on each node
/// 5. Submit the same `CircuitDisbandRequest` to the nodes that did not originally submit the
///    proposal
/// 6. Validate these duplicate proposals are rejected
/// 7. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 8. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal for each
///    remote node
/// 9. Validate the proposal has been updated and includes the `Vote` submitted in the previous
///    steps for every node
/// 10. Create and submit a `Scabbard` transaction from a node, validate this submission returns
///    successfully
/// 11. Create and submit a `CircuitProposalVote` from the third node to accept the disband proposal
/// 12. Wait until the active circuit is no longer available to the remote nodes, using
///    `list_circuits`
/// 13. Validate the circuit is no longer active on the nodes
/// 14. Validate the disbanded circuit is still available to each node, though disbanded, and that
///    the disbanded circuit is the same for each node
/// 15. Submit a `Scabbard` transaction from one of the nodes to validate the batch is unable to be
///    committed on the disbanded circuit as the services set-up for this circuit have been stopped
#[test]
#[ignore]
pub fn test_3_party_circuit_lifecycle() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node from the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node from the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Get the third node from the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(res.is_ok());

    // Wait for the disband proposal to be committed for the second and third node
    let mut proposal_b;
    let mut proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        let proposals_c = node_c
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from third node")
            .data;
        if !proposals_b.is_empty() && !proposals_c.is_empty() {
            // Unwrap the first elements in each list as we've already validated that both of
            // the lists are not empty
            proposal_b = proposals_b.get(0).unwrap().clone();
            proposal_c = proposals_c.get(0).unwrap().clone();
        } else {
            continue;
        }

        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal_a) => proposal_a,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_b, proposal_c);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Submit a duplicate of the disband `CircuitManagementPayload` to the second node
    let duplicate_res = node_b
        .admin_service_client()
        .submit_admin_payload(disband_payload.clone());
    assert!(duplicate_res.is_err());
    // Submit a duplicate of the disband `CircuitManagementPayload` to the third node
    let duplicate_res = node_c
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(duplicate_res.is_err());

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the vote from this node to appear on the proposal for the remote nodes
    let mut proposal_a;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        // The proposal should already be available to each of these nodes, so we are able to
        // unwrap the result of the `fetch_proposal` call
        proposal_a = node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
            .unwrap();
        let proposal_b = node_b
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from second node")
            .unwrap();
        let proposal_c = node_c
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from third node")
            .unwrap();
        if proposal_a.votes.is_empty() && proposal_b.votes.is_empty() && proposal_c.votes.is_empty()
        {
            continue;
        } else {
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
        }
    }

    // Create the `ServiceId` struct based on the second node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_c = get_node_service_id(&circuit_id, node_c);
    // Submit another `Scabbard` transaction from the other node to validate the transaction
    // is successfully committed on the circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_c.admin_signer());
    assert!(node_c
        .scabbard_client()
        .expect("Unable to get third node's ScabbardClient")
        .submit(
            &service_id_c,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        true,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the circuit to be removed for the other nodes
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let circuits_a = node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from first node")
            .data;
        let circuits_b = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from second node")
            .data;
        let circuits_c = node_c
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits from third node")
            .data;

        if circuits_a.is_empty() && circuits_b.is_empty() && circuits_c.is_empty() {
            break;
        }
    }
    // Validate the disbanded circuit is available and the same for each node
    let disbanded_circuit_a = node_a
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from first node")
        .unwrap();
    let disbanded_circuit_b = node_b
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from second node")
        .unwrap();
    let disbanded_circuit_c = node_c
        .admin_service_client()
        .fetch_circuit(&circuit_id)
        .expect("Unable to fetch circuit from third node")
        .unwrap();
    assert_eq!(disbanded_circuit_a, disbanded_circuit_b);
    assert_eq!(disbanded_circuit_b, disbanded_circuit_c);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction returns an
    // error as the circuit has been disbanded, meaning both nodes' services running on that
    // circuit have been stopped
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_3", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    shutdown!(network).expect("Unable to shutdown network");
}

/// Test that a 3-party circuit may be created on a 3-node network. This test then validates the
/// circuit is able to be disbanded.
///
/// 1. Create and commit a circuit between 3 nodes
/// 2. Submit a `Scabbard` transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 3. Create and submit a `CircuitDisbandRequest` from the first node
/// 4. Wait until the disband proposal is available to each node, using `list_proposals`
/// 5. Verify the same disband proposal is present on each node
/// 6. Create and submit a `CircuitProposalVote` from the second node to accept the disband proposal
/// 7. Wait until this vote is recorded on the proposal, using `fetch_proposal` and validating
///    the `Vote` from the node that voted in the previous step appears on the proposal for each
///    remote node
/// 8. Validate the proposal has been updated and includes the `Vote` submitted in the previous
///    steps for every node
/// 9. Create and submit a `CircuitProposalVote` from the third node to reject the disband proposal
/// 10. Wait until the disband proposal is no longer available to the remote nodes, using
///    `list_proposals`
/// 11. Validate the disband proposal is no longer available for every node
/// 12. Validate the circuit is still active for each node, using `list_circuits` which only
///     returns active circuits
/// 13. Submit a `Scabbard` transaction from one of the nodes to validate the service is still able
///    to commit the batch on the circuit that has remained active
#[test]
#[ignore]
pub fn test_3_party_circuit_lifecycle_proposal_rejected() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Get the third node in the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    commit_3_party_circuit(circuit_id, node_a, node_b, node_c);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create disband request to be sent from the first node
    let disband_payload = make_circuit_disband_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    let res = node_a
        .admin_service_client()
        .submit_admin_payload(disband_payload);
    assert!(res.is_ok());

    // Wait for the disband proposal to be committed for the second node
    let mut proposal_b;
    let mut proposal_c;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        let proposals_c = node_c
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from third node")
            .data;
        if !proposals_b.is_empty() && !proposals_c.is_empty() {
            // Unwrap the first elements in each list as we've already validated that both of
            // the lists are not empty
            proposal_b = proposals_b.get(0).unwrap().clone();
            proposal_c = proposals_c.get(0).unwrap().clone();
        } else {
            continue;
        }

        // Validate the same proposal is available to the first node
        let proposal_a = match node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
        {
            Some(proposal) => proposal,
            None => continue,
        };

        assert_eq!(proposal_a, proposal_b);
        assert_eq!(proposal_b, proposal_c);
        assert_eq!(proposal_a.proposal_type, "Disband");
        break;
    }

    // Create `CircuitProposalVote` to accept the disband proposal
    // Uses `true` for the `accept` argument to create a vote to accept the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_b,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
        true,
    );
    let res = node_b
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the vote from this node to appear on the proposal for the remote nodes
    let mut proposal_a;
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect proposal in time");
        }
        // The proposal should already be available to each of these nodes, so we are able to
        // unwrap the result of the `fetch_proposal` call
        proposal_a = node_a
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from first node")
            .unwrap();
        let proposal_b = node_b
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from second node")
            .unwrap();
        let proposal_c = node_c
            .admin_service_client()
            .fetch_proposal(&circuit_id)
            .expect("Unable to fetch proposal from third node")
            .unwrap();
        if proposal_a.votes.is_empty() && proposal_b.votes.is_empty() && proposal_c.votes.is_empty()
        {
            continue;
        } else {
            assert_eq!(proposal_a, proposal_b);
            assert_eq!(proposal_b, proposal_c);
            break;
        }
    }

    // Create the `CircuitProposalVote` to be sent to a node
    // Uses `false` for the `accept` argument to create a vote to reject the proposal
    let vote_payload_bytes = make_circuit_proposal_vote_payload(
        proposal_a,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
        false,
    );
    let res = node_c
        .admin_service_client()
        .submit_admin_payload(vote_payload_bytes);
    assert!(res.is_ok());

    // Wait for the disband proposal to be removed for the other nodes
    let start = Instant::now();
    loop {
        if Instant::now().duration_since(start) > Duration::from_secs(60) {
            panic!("Failed to detect circuit in time");
        }
        let proposals_a = node_a
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from first node")
            .data;
        let proposals_b = node_b
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from second node")
            .data;
        let proposals_c = node_c
            .admin_service_client()
            .list_proposals(None, None)
            .expect("Unable to list proposals from third node")
            .data;
        if proposals_a.is_empty() && proposals_b.is_empty() && proposals_c.is_empty() {
            break;
        }
    }

    // Validate the active circuit is still available to each node
    let active_circuits_a = node_a
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from first node")
        .data;
    assert!(active_circuits_a.len() == 1);

    let active_circuits_b = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from second node")
        .data;
    assert!(active_circuits_b.len() == 1);

    let active_circuits_c = node_c
        .admin_service_client()
        .list_circuits(None)
        .expect("Unable to list circuits from third node")
        .data;
    assert!(active_circuits_c.len() == 1);

    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_a.admin_signer());
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}
