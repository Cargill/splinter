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

//! Integration tests for the process of creating and committing a circuit, then abandoning the
//! circuit between multiple nodes.

use std::time::Duration;

use splinter::admin::messages::AuthorizationType;
use splinterd::node::RestApiVariant;

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::{
    get_node_service_id,
    payload::{make_circuit_abandon_payload, make_create_contract_registry_batch},
};
use crate::framework::network::Network;

/// This test validates the process of committing a circuit between 2 nodes and the process of both
/// nodes abandoning the committed circuit. The test also validates that Splinter service
/// transactions succeed on an active, committed circuit and do not succeed once any member has
/// abandoned the circuit.
///
/// 1. Create and commit a circuit between 2 nodes
/// 2. Submit a Scabbard transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 3. Create and commit another circuit (that will remain active throughout the test) between the
///    2 nodes
/// 4. Submit a Scabbard transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 5. Create and submit a `CircuitAbandon` payload to abandon the circuit from one node
/// 6. Verify the circuit is returned as `Abandoned` by the abandoning node from the previous step,
///    using `list_circuits` filtered on the circuit's status (`status=abandoned`, in this case)
/// 7. Create and submit a `Scabbard` transaction from the non-abandoning node to the abandoned
///    circuit, validate this transaction does not return successfully
/// 8. Create and submit a `Scabbard` transaction from a node to the circuit that remained active,
///    validate this transaction returns successfully
/// 9. Create and submit a `CircuitAbandon` payload to completely abandon the circuit for all
///    members
/// 10. Verify the circuit is returned as `Abandoned` by the abandoning node from the previous step,
///    using `list_circuits` filtered on the circuit's status (`status=abandoned`, in this case)
/// 11. Create and submit a `Scabbard` transaction to the `Abandoned` circuit, validate this
///    transaction does not complete successfully
/// 12. Create and submit a `Scabbard` transaction to the circuit that has remained active, validate
///    this transaction completes successfully.
#[test]
pub fn test_2_party_circuit_abandon() {
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
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Commit a circuit between the 2 nodes that will remain active while the other circuit is
    // abandoned
    let active_circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_2_party_circuit(&active_circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let active_service_id_a = get_node_service_id(&active_circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the abandon request to be sent from the first node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    if let Ok(()) = node_a
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
    {
        let abandoned_circuits = node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Failed to list circuits")
            .data;
        assert_eq!(abandoned_circuits.len(), 1);
    } else {
        panic!("Failed to submit `CircuitAbandon` payload to node");
    }

    // Create the `ServiceId` struct based on the second node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_b = get_node_service_id(&circuit_id, node_b);
    // Submit a `CreateContractRegistryAction` to validate the service transaction, though valid,
    // is not able to be committed as a node has abandoned the specified circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let active_service_id_b = get_node_service_id(&active_circuit_id, node_b);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the abandon request to be sent from the first node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    if let Ok(()) = node_b
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
    {
        let abandoned_circuits = node_b
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Failed to list circuits")
            .data;
        assert_eq!(abandoned_circuits.len(), 1);
    } else {
        panic!("Failed to submit `CircuitAbandon` payload to node");
    }

    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // not able to be committed as the first node has abandoned the specified circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// This test validates the process of committing a circuit between 3 nodes and the process of all
/// nodes abandoning the committed circuit. The test also validates that Splinter service
/// transactions succeed on an active, committed circuit and do not succeed once any member has
/// abandoned the circuit.
///
/// 1. Create and commit a circuit between 3 nodes
/// 2. Submit a Scabbard transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 3. Create and commit another circuit (that will remain active throughout the test) between the
///    3 nodes
/// 4. Submit a Scabbard transaction from one of the nodes to validate the service is able to
///    commit a batch on the circuit committed in the previous step
/// 5. Create and submit a `CircuitAbandon` payload to abandon the circuit from one node
/// 6. Verify the circuit is returned as `Abandoned` by the abandoning node from the previous step,
///    using `list_circuits` filtered on the circuit's status (`status=abandoned`, in this case)
/// 7. Create and submit a `Scabbard` transaction from a non-abandoning node to the abandoned
///    circuit, validate this transaction does not return successfully
/// 8. Create and submit a `Scabbard` transaction from a node to the circuit that remained active,
///    validate this transaction returns successfully
/// 9. Create and submit a `CircuitAbandon` payload to abandon the circuit from another node
/// 10. Verify the circuit is returned as `Abandoned` by the abandoning node from the previous
///    step, using `list_circuits` filtered on the circuit's status (`status=abandoned`)
/// 11. Create and submit a `Scabbard` transaction from the last non-abandoning node to the
///    abandoned circuit, validate this transaction does not return successfully
/// 12. Create and submit a `Scabbard` transaction from a node to the circuit that remained active,
///    validate this transaction returns successfully
/// 13. Create and submit a `CircuitAbandon` payload to completely abandon the circuit for all
///    members
/// 14. Verify the circuit is returned as `Abandoned` by the abandoning node from the previous
///    step, using `list_circuits` filtered on the circuit's status (`status=abandoned`)
/// 15. Create and submit a `Scabbard` transaction to the `Abandoned` circuit, validate this
///    transaction does not complete successfully
/// 16. Create and submit a `Scabbard` transaction to the circuit that has remained active,
///    validate this transaction completes successfully.
#[test]
pub fn test_3_party_circuit_abandon() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node in the network
    let node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let node_b = network.node(1).expect("Unable to get second node");
    // Get the third node from the network
    let node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    // Commit a circuit to state
    commit_3_party_circuit(
        &circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_a = get_node_service_id(&circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Commit a circuit between the nodes that will remain active while the other circuit is
    // abandoned
    let active_circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_3_party_circuit(
        &active_circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let active_service_id_a = get_node_service_id(&active_circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the abandon request to be sent from the first node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the first node
    if let Ok(()) = node_a
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
    {
        let abandoned_circuits = node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Failed to list circuits")
            .data;
        assert_eq!(abandoned_circuits.len(), 1);
    } else {
        panic!("Failed to submit `CircuitAbandon` payload to node");
    }

    // Create the `ServiceId` struct based on the second node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_b = get_node_service_id(&circuit_id, node_b);
    // Submit a `CreateContractRegistryAction` to validate the service transaction, though valid,
    // is not able to be committed as a node has abandoned the specified circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let active_service_id_b = get_node_service_id(&active_circuit_id, node_b);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the abandon request to be sent from the second node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the second node
    if let Ok(()) = node_b
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
    {
        let circuits = node_b
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list abandoned circuits")
            .data;
        assert_eq!(circuits.len(), 1);
    } else {
        panic!("Failed to abandon circuit from node");
    }

    // Create the `ServiceId` struct based on the third node's associated `service_id` and the
    // committed `circuit_id`
    let service_id_c = get_node_service_id(&circuit_id, node_c);
    // Submit a `CreateContractRegistryAction` to validate the service transaction, though valid,
    // is not able to be committed as a node has abandoned the specified circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_c.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_c
        .scabbard_client()
        .expect("Unable to get third node's ScabbardClient")
        .submit(
            &service_id_c,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    // Create the `ServiceId` struct based on the third node's associated `service_id` and the
    // committed `circuit_id` for the circuit that is still active
    let active_service_id_c = get_node_service_id(&active_circuit_id, node_c);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_c.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_c
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_c,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    // Create the abandon request to be sent from the third node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
    );
    // Submit the `CircuitManagementPayload` to the third node
    if let Ok(()) = node_c
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
    {
        let circuits = node_c
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list abandoned circuits")
            .data;
        assert_eq!(circuits.len(), 1);
    } else {
        panic!("Failed to abandon circuit from node");
    }

    // Submit a `CreateContractRegistryAction` to validate the service transaction, though valid,
    // is not able to be committed as nodes have abandoned the specified circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_3", &*node_c.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_c
        .scabbard_client()
        .expect("Unable to get third node's ScabbardClient")
        .submit(
            &service_id_c,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_err());

    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit.
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_3", &*node_c.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    assert!(node_c
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &active_service_id_c,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .is_ok());

    shutdown!(network).expect("Unable to shutdown network");
}

/// This test validates the process of committing a circuit between 2 nodes and the process of both
/// nodes abandoning the committed circuit, while the other node is stopped.
///
/// 1. Create and commit a circuit between 2 nodes
/// 2. Create and commit another circuit (that will remain active throughout the test) between the
///    2 nodes
/// 3. Stop the node that is not abandoning the circuit
/// 4. Create and submit a `CircuitAbandon` payload to abandon the circuit from one node
/// 5. Restart the stopped node, verify both circuits are active for this node
/// 6. Verify the circuit is returned as `Abandoned` by the abandoning node, using `list_circuits`
///    filtered on the circuit's status (`status=abandoned`, in this case)
/// 7. Stop the node that has already abandoned the circuit
/// 8. Create and submit a `CircuitAbandon` payload to completely abandon the circuit for all
///    members
/// 9. Restart the stopped node
/// 10. Verify the restarted node returns 1 active circuit and 1 abandoned circuit
/// 11. Verify the final abandoning node returns 1 active circuit and 1 abandoned circuit
#[test]
pub fn test_2_party_circuit_abandon_stop() {
    // Start a 2-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    // Get the first node in the network
    let mut node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let mut node_b = network.node(1).expect("Unable to get second node");
    let circuit_id = "ABCDE-01234";
    // Commit the circuit to state
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);
    // Commit a circuit between the 2 nodes that will remain active while the other circuit is
    // abandoned
    let active_circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_2_party_circuit(&active_circuit_id, node_a, node_b, AuthorizationType::Trust);

    // Stop the second node in the network
    network = network.stop(1).expect("Unable to stop second node");
    node_a = network.node(0).expect("Unable to get first node");

    // Create the abandon request to be sent from the first node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );

    assert!(node_a
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
        .is_ok());

    // Restart the second node in the network
    network = network.start(1).expect("Unable to start second node");
    node_b = network.node(1).expect("Unable to get second node");
    node_a = network.node(0).expect("Unable to get first node");

    let active_circuits = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("Failed to list circuits")
        .data;
    assert_eq!(active_circuits.len(), 2);

    let abandoned_circuits = node_a
        .admin_service_client()
        .list_circuits(Some("status=abandoned"))
        .expect("Failed to list circuits")
        .data;
    assert_eq!(abandoned_circuits.len(), 1);

    // Stop the first node in the network
    network = network.stop(0).expect("Unable to stop first node");
    node_b = network.node(1).expect("Unable to get second node");

    // Create the abandon request to be sent from the second node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
    );
    assert!(node_b
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
        .is_ok());

    // Restart the first node in the network
    network = network.start(0).expect("Unable to start first node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    // Assert the circuits for the first node are returned as expected
    assert_eq!(
        node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("Failed to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Failed to list circuits")
            .data
            .len(),
        1
    );
    // Assert the circuits for the second node are returned as expected
    assert_eq!(
        node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Failed to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_b
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Failed to list circuits")
            .data
            .len(),
        1
    );

    shutdown!(network).expect("Unable to shutdown network");
}

/// This test validates the process of committing a circuit between 3 nodes and the process of all
/// nodes abandoning the committed circuit. The test also validates that Splinter service
/// transactions succeed on an active, committed circuit and do not succeed once any member has
/// abandoned the circuit.
///
/// 1. Create and commit a circuit between 3 nodes
/// 2. Create and commit another circuit (that will remain active throughout the test) between the
///    3 nodes
/// 3. Stop the second node in the network
/// 4. Create and submit a `CircuitAbandon` payload to abandon the circuit from one node
/// 5. Restart the stopped node, the second one in the network
/// 6. Verify the circuit is still returned as active for the restarted node
/// 7. Verify the circuit is returned as `Abandoned` by the abandoning node, using `list_circuits`
///    filtered on the circuit's status (`status=abandoned`, in this case)
/// 8. Stop the third node in the network
/// 9. Create and submit a `CircuitAbandon` payload to abandon the circuit from the second node
/// 10. Restart the stopped node, the third one in the network
/// 11. Verify the circuit is returned as active for the restarted node
/// 12. Verify the circuit is returned as `Abandoned` by the abandoning node, using `list_circuits`
///    filtered on the circuit's status (`status=abandoned`)
/// 13. Stop the first node in the network
/// 14. Create and submit a `CircuitAbandon` payload to completely abandon the circuit for all
///    members, from the third node in the network
/// 15. Restart the stopped node
/// 16. Verify the circuit is returned as `Abandoned` for each node, using `list_circuits` filtered
///     on the circuit's status (`status=abandoned`)
#[test]
pub fn test_3_party_circuit_abandon_stop() {
    // Start a 3-node network
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    // Get the first node in the network
    let mut node_a = network.node(0).expect("Unable to get first node");
    // Get the second node in the network
    let mut node_b = network.node(1).expect("Unable to get second node");
    // Get the third node from the network
    let mut node_c = network.node(2).expect("Unable to get third node");

    let circuit_id = "ABCDE-01234";
    // Commit a circuit to state
    commit_3_party_circuit(
        &circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );
    // Commit a circuit between the nodes that will remain active while the other circuit is
    // abandoned
    let active_circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_3_party_circuit(
        &active_circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );

    // Stop the second node in the network
    network = network.stop(1).expect("Unable to stop second node");
    node_a = network.node(0).expect("Unable to get first node");

    // Create the abandon request to be sent from the first node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    assert!(node_a
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
        .is_ok());

    // Restart the second node in the network
    network = network.start(1).expect("Unable to start second node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    node_c = network.node(2).expect("Unable to get third node");

    assert_eq!(
        node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits")
            .data
            .len(),
        2
    );
    assert_eq!(
        node_c
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits")
            .data
            .len(),
        2
    );

    // Stop the third node in the network
    network = network.stop(2).expect("Unable to stop third node");
    node_b = network.node(1).expect("Unable to get second node");

    // Create the abandon request to be sent from the second node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_b.node_id(),
        &*node_b.admin_signer().clone_box(),
    );
    assert!(node_b
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
        .is_ok());

    // Restart the third node in the network
    network = network.start(2).expect("Unable to start third node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    node_c = network.node(2).expect("Unable to get third node");

    assert_eq!(
        node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_b
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_c
            .admin_service_client()
            .list_circuits(None)
            .expect("Unable to list circuits")
            .data
            .len(),
        2
    );

    // Stop the first node in the network
    network = network.stop(0).expect("Unable to stop first node");
    node_c = network.node(2).expect("Unable to get third node");

    // Create the abandon request to be sent from the third node
    let abandon_payload = make_circuit_abandon_payload(
        &circuit_id,
        node_c.node_id(),
        &*node_c.admin_signer().clone_box(),
    );
    assert!(node_c
        .admin_service_client()
        .submit_admin_payload(abandon_payload)
        .is_ok());

    // Restart the first node in the network
    network = network.start(0).expect("Unable to start first node");
    node_a = network.node(0).expect("Unable to get first node");
    node_b = network.node(1).expect("Unable to get second node");
    node_c = network.node(2).expect("Unable to get third node");

    assert_eq!(
        node_a
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_b
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );
    assert_eq!(
        node_c
            .admin_service_client()
            .list_circuits(Some("status=abandoned"))
            .expect("Unable to list circuits")
            .data
            .len(),
        1
    );

    shutdown!(network).expect("Unable to shutdown network");
}
