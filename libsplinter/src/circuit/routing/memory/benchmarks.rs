// Copyright 2018-2020 Cargill Incorporated
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

//! This module contains a set of benchmark tests for storing a large number of circuits, services,
//! and nodes in the in-memory RoutingTable using a Rwlock.

use super::{
    Circuit, CircuitNode, RoutingTable, RoutingTableReader, RoutingTableWriter, Service, ServiceId,
};

extern crate test;

use test::Bencher;

use crate::base62::generate_random_base62_string;

use rand::distributions::{Distribution, Uniform};

use std::cmp::min;

// Helper function for generating a large number of nodes with the associated services, that are
// then added to circuits. The circuits contain a random number of nodes up to 10.
fn generate_circuits(num_circuits: i64, total_num_node: i64) -> (Vec<Circuit>, Vec<CircuitNode>) {
    // generate nodes and their associated services
    let mut nodes = vec![];
    for i in 0..total_num_node {
        let node = CircuitNode {
            node_id: format!("node_{}", i),
            endpoints: vec![format!("inproc://node_{}", i)],
        };
        let service = Service {
            service_id: generate_random_base62_string(4),
            service_type: "benchmark".to_string(),
            node_id: format!("inproc://node_{}", i),
            arguments: vec![("peer_services".to_string(), "node-000".to_string())],
            peer_id: Some("benchmark_peer_id".to_string()),
        };
        nodes.push((node, service));
    }

    let mut circuits = vec![];
    let mut rng = rand::thread_rng();
    let num_nodes = Uniform::from(2..(min(total_num_node, 10)));
    let node_indexes = Uniform::from(0..total_num_node);
    let mut used_nodes = vec![];
    for _ in 0..num_circuits {
        let num_of_nodes = num_nodes.sample(&mut rng);
        let mut members = vec![];
        let mut roster = vec![];
        for _ in 0..num_of_nodes {
            let node_index = node_indexes.sample(&mut rng);
            let (node, service) = nodes
                .get(node_index as usize)
                .expect(&format!("Unable to get node at index {}", node_index));
            members.push(node.node_id.clone());
            used_nodes.push(node.clone());
            roster.push(service.clone());
        }

        let circuit = Circuit {
            circuit_id: format!(
                "{}-{}",
                generate_random_base62_string(5),
                generate_random_base62_string(5)
            ),
            roster,
            members,
        };
        circuits.push(circuit);
    }
    used_nodes.sort();
    used_nodes.dedup();
    // generate a circuit with generated nodes
    (circuits, used_nodes)
}

// Test that a routing table that has been loaded with 2^14 circuits still functions.
//
// After the the circuits are loaded, verify that a circuit, service, and node can be fetched
// from the routing table.
#[test]
fn test_high_load_2_to_14_circuits() {
    let base: i64 = 2;
    let (circuits, used_nodes) = generate_circuits(base.pow(14), base.pow(7));

    let first_circuit = circuits.get(0).expect("Unable to get 1st circuit").clone();
    let first_node = used_nodes.get(0).expect("Unable to get 1st node").clone();

    let table = RoutingTable::default();
    let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
    let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

    let (mut circuit_to_add, mut circuits) = circuits.split_at(min(1000, circuits.len()));
    writer
        .add_nodes(used_nodes.clone())
        .expect("Unable to add nodes");
    while circuit_to_add.len() > 0 {
        writer
            .add_circuits(circuit_to_add.to_vec())
            .expect("Unable to write circuits");
        let (new, old) = circuits.split_at(min(1000, circuits.len()));
        circuits = old;
        circuit_to_add = new;
    }

    let fetched_circuit = reader
        .get_circuit(&first_circuit.circuit_id)
        .expect("Unable to fetch 1st circuit");
    assert_eq!(fetched_circuit, Some(first_circuit.clone()));

    let service = fetched_circuit
        .expect("Unable to get 1st circuit")
        .roster
        .get(0)
        .expect("Unable to get service")
        .clone();
    let service_id = ServiceId::new(
        first_circuit.circuit_id.to_string(),
        service.service_id.to_string(),
    );
    let fetched_service = reader
        .get_service(&service_id)
        .expect("Unable to fetch service");
    assert_eq!(fetched_service, Some(service));

    let fetched_node = reader
        .get_node(&first_node.node_id)
        .expect("Unable to fetch node");
    assert_eq!(fetched_node, Some(first_node));
}

// Benchmark the time it takes to load 2^14 cirucits with 2^7 nodes, while a seperate thread is
// also adding a new circuit continuously.
//
// The circuits are added 1000 at a time.
#[bench]
fn test_high_load_start_up_cost_threads(b: &mut Bencher) {
    let base: i64 = 2;
    let (circuits, used_nodes) = generate_circuits(base.pow(14), base.pow(7));

    let first_circuit = circuits.get(0).expect("Unable to get 1st circuit").clone();

    let table = RoutingTable::default();
    let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
    let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());
    let mut thread_writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());
    std::thread::spawn(move || loop {
        let (thread_circuits, thread_nodes) = generate_circuits(1, base.pow(7));
        let new_circuit = thread_circuits
            .get(0)
            .expect("Unable to get circuit")
            .clone();
        thread_writer
            .add_circuit(
                new_circuit.circuit_id.to_string(),
                new_circuit,
                thread_nodes,
            )
            .expect("Unable to add circuit");
    });

    b.iter(|| {
        let (mut circuit_to_add, mut circuits) = circuits.split_at(min(1000, circuits.len()));
        writer
            .add_nodes(used_nodes.clone())
            .expect("Unable to add nodes");
        while circuit_to_add.len() > 0 {
            writer
                .add_circuits(circuit_to_add.to_vec())
                .expect("Unable to write circuits");
            let (new, old) = circuits.split_at(min(1000, circuits.len()));
            circuits = old;
            circuit_to_add = new;
        }
    });

    let fetched_circuit = reader
        .get_circuit(&first_circuit.circuit_id)
        .expect("Unable to fetch circuit");
    assert_eq!(fetched_circuit, Some(first_circuit.clone()));
}

// Benchmark the time it takes to load 2^14 circuits with 2^7 nodes.
//
// The circuits are added 1000 at a time.
#[bench]
fn test_high_load_start_up_cost(b: &mut Bencher) {
    let base: i64 = 2;
    let (circuits, used_nodes) = generate_circuits(base.pow(14), base.pow(7));

    let first_circuit = circuits.get(1).expect("Unable to get 1st circuit").clone();

    let table = RoutingTable::default();
    let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
    let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

    b.iter(|| {
        let (mut circuit_to_add, mut circuits) = circuits.split_at(min(1000, circuits.len()));
        writer
            .add_nodes(used_nodes.clone())
            .expect("Unable to add nodes");
        while circuit_to_add.len() > 0 {
            writer
                .add_circuits(circuit_to_add.to_vec())
                .expect("Unable to write circuits");
            let (new, old) = circuits.split_at(min(1000, circuits.len()));
            circuits = old;
            circuit_to_add = new;
        }
    });

    let fetched_circuit = reader
        .get_circuit(&first_circuit.circuit_id)
        .expect("Unable to get 1st circuit");
    assert_eq!(fetched_circuit, Some(first_circuit));
}

// --------- Write benchmark tests -----------------
//
// The following benchmark tests benchmark the time it takes to add a new circuit to a loaded
// routing table. The routing table is loaded with 2^x circuits from 3-14 and 2^x nodes from 3-7.

// 2^3 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_3_node_3(b: &mut Bencher) {
    run_write_test(3, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_3_node_4(b: &mut Bencher) {
    run_write_test(3, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_3_node_5(b: &mut Bencher) {
    run_write_test(3, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_3_node_6(b: &mut Bencher) {
    run_write_test(3, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_3_node_7(b: &mut Bencher) {
    run_write_test(3, 7, b);
}

// 2^4 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_4_node_3(b: &mut Bencher) {
    run_write_test(4, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_4_node_4(b: &mut Bencher) {
    run_write_test(4, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_4_node_5(b: &mut Bencher) {
    run_write_test(4, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_4_node_6(b: &mut Bencher) {
    run_write_test(4, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_4_node_7(b: &mut Bencher) {
    run_write_test(4, 7, b);
}

// 2^5 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_5_node_3(b: &mut Bencher) {
    run_write_test(5, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_5_node_4(b: &mut Bencher) {
    run_write_test(5, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_5_node_5(b: &mut Bencher) {
    run_write_test(5, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_5_node_6(b: &mut Bencher) {
    run_write_test(5, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_5_node_7(b: &mut Bencher) {
    run_write_test(5, 7, b);
}

// 2^6 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_6_node_3(b: &mut Bencher) {
    run_write_test(6, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_6_node_4(b: &mut Bencher) {
    run_write_test(6, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_6_node_5(b: &mut Bencher) {
    run_write_test(6, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_6_node_6(b: &mut Bencher) {
    run_write_test(6, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_6_node_7(b: &mut Bencher) {
    run_write_test(6, 7, b);
}

// 2^7 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_7_node_3(b: &mut Bencher) {
    run_write_test(7, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_7_node_4(b: &mut Bencher) {
    run_write_test(7, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_7_node_5(b: &mut Bencher) {
    run_write_test(7, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_7_node_6(b: &mut Bencher) {
    run_write_test(7, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_7_node_7(b: &mut Bencher) {
    run_write_test(7, 7, b);
}

// 2^8 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_8_node_3(b: &mut Bencher) {
    run_write_test(8, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_8_node_4(b: &mut Bencher) {
    run_write_test(8, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_8_node_5(b: &mut Bencher) {
    run_write_test(8, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_8_node_6(b: &mut Bencher) {
    run_write_test(8, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_8_node_7(b: &mut Bencher) {
    run_write_test(8, 7, b);
}

// 2^9 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_9_node_3(b: &mut Bencher) {
    run_write_test(9, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_9_node_4(b: &mut Bencher) {
    run_write_test(9, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_9_node_5(b: &mut Bencher) {
    run_write_test(9, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_9_node_6(b: &mut Bencher) {
    run_write_test(9, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_9_node_7(b: &mut Bencher) {
    run_write_test(9, 7, b);
}

// 2^10 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_10_node_3(b: &mut Bencher) {
    run_write_test(10, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_10_node_4(b: &mut Bencher) {
    run_write_test(10, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_10_node_5(b: &mut Bencher) {
    run_write_test(10, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_10_node_6(b: &mut Bencher) {
    run_write_test(10, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_10_node_7(b: &mut Bencher) {
    run_write_test(10, 7, b);
}

// 2^11 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_11_node_3(b: &mut Bencher) {
    run_write_test(11, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_11_node_4(b: &mut Bencher) {
    run_write_test(11, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_11_node_5(b: &mut Bencher) {
    run_write_test(11, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_11_node_6(b: &mut Bencher) {
    run_write_test(11, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_11_node_7(b: &mut Bencher) {
    run_write_test(11, 7, b);
}

// 2^12 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_12_node_3(b: &mut Bencher) {
    run_write_test(12, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_12_node_4(b: &mut Bencher) {
    run_write_test(12, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_12_node_5(b: &mut Bencher) {
    run_write_test(12, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_12_node_6(b: &mut Bencher) {
    run_write_test(12, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_12_node_7(b: &mut Bencher) {
    run_write_test(12, 7, b);
}

// 2^13 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_13_node_3(b: &mut Bencher) {
    run_write_test(13, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_13_node_4(b: &mut Bencher) {
    run_write_test(13, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_13_node_5(b: &mut Bencher) {
    run_write_test(13, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_13_node_6(b: &mut Bencher) {
    run_write_test(13, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_13_node_7(b: &mut Bencher) {
    run_write_test(13, 7, b);
}

// 2^14 circuits, node varying
#[bench]
fn test_high_load_performance_write_circuit_14_node_3(b: &mut Bencher) {
    run_write_test(14, 3, b);
}

#[bench]
fn test_high_load_performance_write_circuit_14_node_4(b: &mut Bencher) {
    run_write_test(14, 4, b);
}

#[bench]
fn test_high_load_performance_write_circuit_14_node_5(b: &mut Bencher) {
    run_write_test(14, 5, b);
}

#[bench]
fn test_high_load_performance_write_circuit_14_node_6(b: &mut Bencher) {
    run_write_test(14, 6, b);
}

#[bench]
fn test_high_load_performance_write_circuit_14_node_7(b: &mut Bencher) {
    run_write_test(14, 7, b);
}

// Helper function for running the write benchmark tests. Takes the power of 2 that should be taken
//  for the number of circuits and nodes.
//
// Starts the test by generating the required circuits and nodes and adds them to the routing table.
// The time it takes to add a new circuit to the routing table is benchmarked.
fn run_write_test(circuit_pow: u32, node_pow: u32, b: &mut Bencher) {
    let base: i64 = 2;
    let (circuits, used_nodes) = generate_circuits(base.pow(circuit_pow), base.pow(node_pow));

    let (new_circuit_vec, new_used_nodes) = generate_circuits(1, base.pow(node_pow));
    let new_circuit = new_circuit_vec.get(0).expect("Unable to get new circuit");

    let table = RoutingTable::default();
    let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

    let (mut circuit_to_add, mut circuits) = circuits.split_at(min(1000, circuits.len()));
    writer.add_nodes(used_nodes).expect("Unable to add nodes");
    while circuit_to_add.len() > 0 {
        writer
            .add_circuits(circuit_to_add.to_vec())
            .expect("Unable to write circuits");
        let (new, old) = circuits.split_at(min(1000, circuits.len()));
        circuits = old;
        circuit_to_add = new;
    }

    b.iter(|| {
        writer
            .add_circuit(
                new_circuit.circuit_id.to_string(),
                new_circuit.clone(),
                new_used_nodes.clone(),
            )
            .expect("Unable to add circuit");
    });
}

// --------- Read benchmark tests -----------------
//
// The following benchmark tests benchmark the time it takes to fetch a circuit from a loaded
// routing table. The routing table is loaded with 2^x circuits from 3-14 and 2^x nodes from 3-7.

// 2^3 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_3_node_3(b: &mut Bencher) {
    run_read_test(3, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_3_node_4(b: &mut Bencher) {
    run_read_test(3, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_3_node_5(b: &mut Bencher) {
    run_read_test(3, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_3_node_6(b: &mut Bencher) {
    run_read_test(3, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_3_node_7(b: &mut Bencher) {
    run_read_test(3, 7, b);
}

// 2^4 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_4_node_3(b: &mut Bencher) {
    run_read_test(4, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_4_node_4(b: &mut Bencher) {
    run_read_test(4, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_4_node_5(b: &mut Bencher) {
    run_read_test(4, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_4_node_6(b: &mut Bencher) {
    run_read_test(4, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_4_node_7(b: &mut Bencher) {
    run_read_test(4, 7, b);
}

// 2^5 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_5_node_3(b: &mut Bencher) {
    run_read_test(5, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_5_node_4(b: &mut Bencher) {
    run_read_test(5, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_5_node_5(b: &mut Bencher) {
    run_read_test(5, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_5_node_6(b: &mut Bencher) {
    run_read_test(5, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_5_node_7(b: &mut Bencher) {
    run_read_test(5, 7, b);
}

// 2^6 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_6_node_3(b: &mut Bencher) {
    run_read_test(6, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_6_node_4(b: &mut Bencher) {
    run_read_test(6, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_6_node_5(b: &mut Bencher) {
    run_read_test(6, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_6_node_6(b: &mut Bencher) {
    run_read_test(6, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_6_node_7(b: &mut Bencher) {
    run_read_test(6, 7, b);
}

// 2^7 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_7_node_3(b: &mut Bencher) {
    run_read_test(7, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_7_node_4(b: &mut Bencher) {
    run_read_test(7, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_7_node_5(b: &mut Bencher) {
    run_read_test(7, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_7_node_6(b: &mut Bencher) {
    run_read_test(7, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_7_node_7(b: &mut Bencher) {
    run_read_test(7, 7, b);
}

// 2^8 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_8_node_3(b: &mut Bencher) {
    run_read_test(8, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_8_node_4(b: &mut Bencher) {
    run_read_test(8, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_8_node_5(b: &mut Bencher) {
    run_read_test(8, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_8_node_6(b: &mut Bencher) {
    run_read_test(8, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_8_node_7(b: &mut Bencher) {
    run_read_test(8, 7, b);
}

// 2^9 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_9_node_3(b: &mut Bencher) {
    run_read_test(9, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_9_node_4(b: &mut Bencher) {
    run_read_test(9, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_9_node_5(b: &mut Bencher) {
    run_read_test(9, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_9_node_6(b: &mut Bencher) {
    run_read_test(9, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_9_node_7(b: &mut Bencher) {
    run_read_test(9, 7, b);
}

// 2^10 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_10_node_3(b: &mut Bencher) {
    run_read_test(10, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_10_node_4(b: &mut Bencher) {
    run_read_test(10, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_10_node_5(b: &mut Bencher) {
    run_read_test(10, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_10_node_6(b: &mut Bencher) {
    run_read_test(10, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_10_node_7(b: &mut Bencher) {
    run_read_test(10, 7, b);
}

// 2^11 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_11_node_3(b: &mut Bencher) {
    run_read_test(11, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_11_node_4(b: &mut Bencher) {
    run_read_test(11, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_11_node_5(b: &mut Bencher) {
    run_read_test(11, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_11_node_6(b: &mut Bencher) {
    run_read_test(11, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_11_node_7(b: &mut Bencher) {
    run_read_test(11, 7, b);
}

// 2^12 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_12_node_3(b: &mut Bencher) {
    run_read_test(12, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_12_node_4(b: &mut Bencher) {
    run_read_test(12, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_12_node_5(b: &mut Bencher) {
    run_read_test(12, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_12_node_6(b: &mut Bencher) {
    run_read_test(12, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_12_node_7(b: &mut Bencher) {
    run_read_test(12, 7, b);
}

// 2^13 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_13_node_3(b: &mut Bencher) {
    run_read_test(13, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_13_node_4(b: &mut Bencher) {
    run_read_test(13, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_13_node_5(b: &mut Bencher) {
    run_read_test(13, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_13_node_6(b: &mut Bencher) {
    run_read_test(13, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_13_node_7(b: &mut Bencher) {
    run_read_test(13, 7, b);
}

// 2^14 circuits, node varying
#[bench]
fn test_high_load_performance_read_circuit_14_node_3(b: &mut Bencher) {
    run_read_test(14, 3, b);
}

#[bench]
fn test_high_load_performance_read_circuit_14_node_4(b: &mut Bencher) {
    run_read_test(14, 4, b);
}

#[bench]
fn test_high_load_performance_read_circuit_14_node_5(b: &mut Bencher) {
    run_read_test(14, 5, b);
}

#[bench]
fn test_high_load_performance_read_circuit_14_node_6(b: &mut Bencher) {
    run_read_test(14, 6, b);
}

#[bench]
fn test_high_load_performance_read_circuit_14_node_7(b: &mut Bencher) {
    run_read_test(14, 7, b);
}

// Helper function for running the read benchmark tests. Takes the power of 2 that should be taken
// for the number of circuits and nodes.
//
// Starts the test by generating the required circuits and nodes and adds them to the routing table.
// The time it takes to fetch a circuit from the routing table is benchmarked.
fn run_read_test(circuit_pow: u32, node_pow: u32, b: &mut Bencher) {
    let base: i64 = 2;
    let (circuits, used_nodes) = generate_circuits(base.pow(circuit_pow), base.pow(node_pow));

    let first_circuit = circuits.get(1).expect("Unable to get 1st circuit").clone();

    let table = RoutingTable::default();
    let reader: Box<dyn RoutingTableReader> = Box::new(table.clone());
    let mut writer: Box<dyn RoutingTableWriter> = Box::new(table.clone());

    let (mut circuit_to_add, mut circuits) = circuits.split_at(min(1000, circuits.len()));
    writer.add_nodes(used_nodes).expect("Unable to add nodes");
    while circuit_to_add.len() > 0 {
        writer
            .add_circuits(circuit_to_add.to_vec())
            .expect("Unable to write circuits");
        let (new, old) = circuits.split_at(min(1000, circuits.len()));
        circuits = old;
        circuit_to_add = new;
    }

    let mut fetched_circuit = None;

    b.iter(|| {
        fetched_circuit = reader
            .get_circuit(&first_circuit.circuit_id)
            .expect("Unable to fetch circuits");
    });

    assert_eq!(fetched_circuit, Some(first_circuit));
}
