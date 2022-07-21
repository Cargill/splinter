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
//

use std::time::Duration;

use splinter::admin::messages::AuthorizationType;
use splinterd::node::{Node, RestApiVariant};

use crate::admin::circuit_commit::{commit_2_party_circuit, commit_3_party_circuit};
use crate::admin::{
    get_node_service_id,
    payload::{
        make_circuit_abandon_payload, make_circuit_purge_payload,
        make_create_contract_registry_batch,
    },
};
use crate::framework::network::Network;

/// This test checks that two node circuits can be purged, and that purging a circuit will not
/// interfere with other circuits.
///
/// 1. Create and commit a circuit for 2 nodes
/// 2. Submit a scabbard transaction to confirm the service is working
/// 3. Create another circuit to act as a control, there should be no functional change on this circuit
/// 4. Submit batch on each circuit to confirm the circuit works from either end
/// 5. Abandon circuit
/// 6. Purge circuit
/// 7. Verify the circuit is no longer accessible from purging node
/// 8. Verify control circuit is still operational/not abandoned or purged
/// 9. Verify test circuit no longer accepts scabbard batches
#[test]
pub fn test_2_party_circuit_purge() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    let node_a = network.node(0).expect("Unable to get first node");
    let node_b = network.node(1).expect("Unable to get first node");
    let test_circuit_id = "QAZED-12345";
    commit_2_party_circuit(&test_circuit_id, node_a, node_b, AuthorizationType::Trust);

    let test_service_id_a = get_node_service_id(&test_circuit_id, node_a);
    let test_service_id_b = get_node_service_id(&test_circuit_id, node_b);
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    node_a
        .scabbard_client()
        .expect("Unable to get first node's scabbard client")
        .submit(
            &test_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .expect("Unable to submit batch to scabbard");

    // Commit a circuit between the 2 nodes that will remain active while the other circuit is
    // purged
    let control_circuit_id = "FGHIJ-56789";
    // Commit the circuit to state
    commit_2_party_circuit(
        &control_circuit_id,
        node_a,
        node_b,
        AuthorizationType::Trust,
    );

    // Create the `ServiceId` struct based on the first node's associated `service_id` and the
    // committed `circuit_id`
    let control_service_id_a = get_node_service_id(&control_circuit_id, node_a);
    // Submit a `CreateContractRegistryAction` to validate the service transaction is
    // valid on the active circuit
    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient")
        .submit(
            &control_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .expect("Unable to submit batch to scabbard");

    // Create the abandon request to be sent from the first node
    // Circuits must be abandoned before they can be purged
    let abandon_payload = make_circuit_abandon_payload(
        &test_circuit_id,
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
        // check both circuits are still connected to the second node
        let purged_circuits = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("failed to list circuits")
            .data;
        assert_eq!(purged_circuits.len(), 2);
    } else {
        panic!("Failed to submit `CircuitAbandon` payload to node");
    }

    // Now that the circuit has been abandoned it can be purged
    let purge_payload = make_circuit_purge_payload(
        test_circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    if let Ok(()) = node_a
        .admin_service_client()
        .submit_admin_payload(purge_payload)
    {
        // check there is only one circuit on node_a
        let purged_circuits = node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("failed to list circuits")
            .data;
        assert_eq!(purged_circuits.len(), 1);

        // check there are two circuits on node_b
        let purged_circuits = node_b
            .admin_service_client()
            .list_circuits(None)
            .expect("failed to list circuits")
            .data;
        assert_eq!(purged_circuits.len(), 2);
    } else {
        panic!("failed to submit `CircuitPurge` payload to node");
    }

    let control_service_id_b = get_node_service_id(&control_circuit_id, node_b);

    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    // Check that the non purged circuit is still working
    node_b
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &control_service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .expect("Unable to submit batch to scabbard");

    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_1", &*node_b.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    // Check that the purged circuit is "dead" from the other end
    assert!(node_b
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient'")
        .submit(
            &test_service_id_b,
            vec![scabbard_batch],
            Some(Duration::from_secs(5))
        )
        .is_err());

    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_2", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    // Check that the non purged circuit is still working
    node_a
        .scabbard_client()
        .expect("Unable to get second node's ScabbardClient")
        .submit(
            &control_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5)),
        )
        .expect("Unable to submit batch to scabbard");

    let scabbard_batch =
        make_create_contract_registry_batch("contract_registry_0", &*node_a.admin_signer())
            .expect("Unable to build `CreateContractRegistryAction`");
    // Check that the purged circuit is "dead" on the purging node
    assert!(node_a
        .scabbard_client()
        .expect("Unable to get first node's ScabbardClient'")
        .submit(
            &test_service_id_a,
            vec![scabbard_batch],
            Some(Duration::from_secs(5))
        )
        .is_err());
    shutdown!(network).expect("Unable to shutdown network");
}

/// This test checks that three node circuits can be purged, and that purging a circuit will not
/// interfere with other circuits.
///
/// 1. Create and commit a circuit for 3 nodes
/// 2. Submit a scabbard transaction to confirm the service is working
/// 3. Create another circuit to act as a control, there should be no functional change on this circuit
/// 4. Submit batch on each circuit to confirm the circuit works from all nodes
/// 5. Abandon circuit
/// 6. Purge circuit
/// 7. Verify the circuit is no longer accessible from purging node
/// 8. Verify control circuit is still operational/not abandoned or purged
/// 9. Verify test circuit no longer accepts scabbard batches
#[test]
pub fn test_3_party_circuit_purge() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(3)
        .expect("Unable to start 3-node ActixWeb1 network");
    let node_a = network.node(0).expect("Could not get first node");
    let node_b = network.node(1).expect("Could not get second node");
    let node_c = network.node(2).expect("Could not get third node");

    let test_circuit_id = "QWERT-01234";
    commit_3_party_circuit(
        test_circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );

    let service_id_a = get_node_service_id(test_circuit_id, node_a);
    let service_id_b = get_node_service_id(test_circuit_id, node_b);
    let service_id_c = get_node_service_id(test_circuit_id, node_c);

    let control_circuit_id = "01234-QWERT";
    commit_3_party_circuit(
        control_circuit_id,
        node_a,
        node_b,
        node_c,
        AuthorizationType::Trust,
    );

    let control_service_id_a = get_node_service_id(control_circuit_id, node_a);
    let control_service_id_b = get_node_service_id(control_circuit_id, node_b);
    let control_service_id_c = get_node_service_id(control_circuit_id, node_c);

    let confirm_basic_function = |node: &Node, service_id, name_mask| {
        let scabbard_batch = make_create_contract_registry_batch(
            &format!("{}{}", node.node_id(), name_mask),
            &*node.admin_signer(),
        )
        .expect("Unable to build `CreateContractRegistryAction`");
        node.scabbard_client()
            .expect("Unable to get node's ScabbardClient")
            .submit(
                service_id,
                vec![scabbard_batch],
                Some(Duration::from_secs(5)),
            )
            .expect("Unable to submit batch to scabbard");
    };
    // Check test circuit function
    let mask = "before_test";
    confirm_basic_function(node_a, &service_id_a, &mask);
    confirm_basic_function(node_b, &service_id_b, &mask);
    confirm_basic_function(node_c, &service_id_c, &mask);

    // Check control circuit function
    let mask = "before_control";
    confirm_basic_function(node_a, &control_service_id_a, &mask);
    let mask = "before_test";
    confirm_basic_function(node_b, &control_service_id_b, &mask);
    confirm_basic_function(node_c, &control_service_id_c, &mask);

    // Create the abandon request to be sent from the first node
    // Circuits must be abandoned before they can be purged
    let abandon_payload = make_circuit_abandon_payload(
        &test_circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );

    // Send abandon_payload and check the circuit has been marked as abandoned
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

    // Now that the circuit has been abandoned it can be purged
    let purge_payload = make_circuit_purge_payload(
        test_circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    if let Ok(()) = node_a
        .admin_service_client()
        .submit_admin_payload(purge_payload)
    {
        let purged_circuits = node_a
            .admin_service_client()
            .list_circuits(None)
            .expect("failed to list circuits")
            .data;
        assert_eq!(purged_circuits.len(), 1);
    } else {
        panic!("failed to submit `CircuitPurge` payload to node");
    }

    let fail_submit_to_purged_circuit = |node: &Node, service_id| {
        let scabbard_batch = make_create_contract_registry_batch(
            &format!("{}1", node.node_id()),
            &*node.admin_signer(),
        )
        .expect("Unable to build `CreateContractRegistryAction`");
        // Check that the purged circuit is "dead" on the purging node
        assert!(node
            .scabbard_client()
            .expect("Unable to get first node's ScabbardClient'")
            .submit(
                service_id,
                vec![scabbard_batch],
                Some(Duration::from_secs(5))
            )
            .is_err());
    };

    // Check submissions fail for purger
    fail_submit_to_purged_circuit(node_a, &service_id_a);
    // Check submissions fail for other nodes
    fail_submit_to_purged_circuit(node_b, &service_id_b);
    fail_submit_to_purged_circuit(node_c, &service_id_c);

    // Check its been marked as purged on the other nodes.
    let purged_circuits = node_b
        .admin_service_client()
        .list_circuits(None)
        .expect("failed to list circuits")
        .data;
    assert_eq!(purged_circuits.len(), 2);
    let purged_circuits = node_c
        .admin_service_client()
        .list_circuits(None)
        .expect("failed to list circuits")
        .data;
    assert_eq!(purged_circuits.len(), 2);

    let mask = "after";
    confirm_basic_function(node_a, &control_service_id_a, &mask);
    confirm_basic_function(node_b, &control_service_id_b, &mask);
    confirm_basic_function(node_c, &control_service_id_c, &mask);

    shutdown!(network).expect("Unable to shutdown network")
}

/// This test checks that non abandoned circuits can't be purged.
///
/// 1. Create and commit a 2 node circuit
/// 2. Submit a transaction to confirm the circuit works
/// 3. Create a circuit purge transaction
/// 4. Submit the purge transaction and confirm it is rejected.
/// 5. Confirm the circuit still works for both member nodes
#[test]
fn test_purge_non_abandoned_circuit() {
    let mut network = Network::new()
        .with_default_rest_api_variant(RestApiVariant::ActixWeb1)
        .add_nodes_with_defaults(2)
        .expect("Unable to start 2-node ActixWeb1 network");
    let node_a = network.node(0).expect("Could not get first node");
    let node_b = network.node(1).expect("Could not get second node");
    let circuit_id = "QAZED-12345";
    commit_2_party_circuit(&circuit_id, node_a, node_b, AuthorizationType::Trust);

    let service_id_a = get_node_service_id(&circuit_id, node_a);
    let service_id_b = get_node_service_id(&circuit_id, node_b);
    let basic_function = |node: &Node, service_id, &mask| {
        let scabbard_batch = make_create_contract_registry_batch(
            &format!("{}{}", node.node_id(), mask),
            &*node.admin_signer(),
        )
        .expect("Unable to build `CreateContractRegistryAction`");
        node.scabbard_client()
            .expect("Unable to get first node's scabbard client")
            .submit(
                service_id,
                vec![scabbard_batch],
                Some(Duration::from_secs(5)),
            )
            .expect("Unable to submit batch to scabbard");
    };
    let mask = "before";
    basic_function(node_a, &service_id_a, &mask);
    basic_function(node_b, &service_id_b, &mask);

    let purge_batch = make_circuit_purge_payload(
        circuit_id,
        node_a.node_id(),
        &*node_a.admin_signer().clone_box(),
    );
    assert!(node_a
        .admin_service_client()
        .submit_admin_payload(purge_batch)
        .is_err());
    let mask = "after";
    basic_function(node_a, &service_id_a, &mask);
    basic_function(node_b, &service_id_b, &mask);
    shutdown!(network).expect("Unable to shutdown network");
}
