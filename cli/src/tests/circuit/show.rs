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

//! Tests for the `splinter circuit show` subcommand.

use std::collections::BTreeMap;

use serial_test::serial;

use crate::CliError;

use super::{
    get_circuit_id_from_propose_output, get_key, run_with_captured_output,
    wait_until_circuits_created, wait_until_proposals_committed, Circuit,
};

/// Test that a basic `splinter circuit show` command is successful.
///
/// 1. Create a new circuit by proposing and voting on it and waiting for it to be created.
/// 2. Run the `splinter circuit show` command with the new circuit's ID and no special arguments;
///    verify that the circuit is shown by inspecting the output for the member node IDs, service
///    IDs, service types, and admin keys.
#[test]
#[serial(stdout)]
#[ignore]
fn show_successful() {
    // Submit a new proposal, get the circuit ID, and wait for the proposal to be committed
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tls://splinterd-node-acme:8044 \
         --node bubba-node-000::tls://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id]);

    // Vote on the circuit and wait for it to be created
    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");
    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);

    // Verify `show` output
    let output = run_with_captured_output(&format!(
        "splinter circuit show --url http://localhost:8088 {}",
        circuit_id,
    ))
    .expect("Failed to show circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));
}

/// Test that a `splinter circuit show --format json` is successful.
///
/// 1. Create a new circuit by proposing and voting on it and waiting for it to be created.
/// 2. Run the `splinter circuit show` command with the new circuit's ID and the `--format json`
///    argument; verify that the circuit is in correct JSON format by deserializing the output and
///    checking the values of all fields on the parsed circuit.
#[test]
#[serial(stdout)]
#[ignore]
fn show_format_json() {
    // Submit a new proposal, get the circuit ID, and wait for the proposal to be committed
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tls://splinterd-node-acme:8044 \
         --node bubba-node-000::tls://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id]);

    // Vote on the circuit and wait for it to be created
    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");
    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);

    // Verify `show` output
    let output = run_with_captured_output(&format!(
        "splinter circuit show --url http://localhost:8088 {} --format json",
        circuit_id,
    ))
    .expect("Failed to show circuit");

    let circuit: Circuit = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert_eq!(circuit.id, circuit_id);
    assert_eq!(
        circuit.members,
        vec!["acme-node-000".to_string(), "bubba-node-000".to_string()]
    );
    assert_eq!(&circuit.management_type, "custom");
    assert_eq!(circuit.roster.len(), 2);

    let mut expected_service_args = BTreeMap::new();
    expected_service_args.insert(
        "admin_keys".into(),
        format!("[\"{}\"]", get_key("/tmp/alice.pub")),
    );

    let service1 = &circuit.roster[0];
    assert_eq!(&service1.service_id, "sc00");
    assert_eq!(&service1.service_type, "scabbard");
    assert_eq!(service1.allowed_nodes, vec!["acme-node-000".to_string()]);
    let mut expected_service_args_service1 = expected_service_args.clone();
    expected_service_args_service1.insert("peer_services".into(), "[\"sc01\"]".into());
    assert_eq!(service1.arguments, expected_service_args_service1);

    let service2 = &circuit.roster[1];
    assert_eq!(&service2.service_id, "sc01");
    assert_eq!(&service2.service_type, "scabbard");
    assert_eq!(service2.allowed_nodes, vec!["bubba-node-000".to_string()]);
    let mut expected_service_args_service2 = expected_service_args;
    expected_service_args_service2.insert("peer_services".into(), "[\"sc00\"]".into());
    assert_eq!(service2.arguments, expected_service_args_service2);
}

/// Test that a `splinter circuit show --format yaml` is successful.
///
/// 1. Create a new circuit by proposing and voting on it and waiting for it to be created.
/// 2. Run the `splinter circuit show` command with the new circuit's ID and the `--format yaml`
///    argument; verify that the circuit is in correct YAML format by deserializing the output and
///    checking the values of all fields on the parsed circuit.
#[test]
#[serial(stdout)]
#[ignore]
fn show_format_yaml() {
    // Submit a new proposal, get the circuit ID, and wait for the proposal to be committed
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tls://splinterd-node-acme:8044 \
         --node bubba-node-000::tls://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id]);

    // Vote on the circuit and wait for it to be created
    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");

    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);

    // Verify `show` output
    let output = run_with_captured_output(&format!(
        "splinter circuit show --url http://localhost:8088 {} --format yaml",
        circuit_id,
    ))
    .expect("Failed to show circuit");

    let circuit: Circuit = serde_yaml::from_str(&output).expect("Failed to parse YAML");
    assert_eq!(circuit.id, circuit_id);
    assert_eq!(
        circuit.members,
        vec!["acme-node-000".to_string(), "bubba-node-000".to_string()]
    );
    assert_eq!(&circuit.management_type, "custom");
    assert_eq!(circuit.roster.len(), 2);

    let mut expected_service_args = BTreeMap::new();
    expected_service_args.insert(
        "admin_keys".into(),
        format!("[\"{}\"]", get_key("/tmp/alice.pub")),
    );

    let service1 = &circuit.roster[0];
    assert_eq!(&service1.service_id, "sc00");
    assert_eq!(&service1.service_type, "scabbard");
    assert_eq!(service1.allowed_nodes, vec!["acme-node-000".to_string()]);
    let mut expected_service_args_service1 = expected_service_args.clone();
    expected_service_args_service1.insert("peer_services".into(), "[\"sc01\"]".into());
    assert_eq!(service1.arguments, expected_service_args_service1);

    let service2 = &circuit.roster[1];
    assert_eq!(&service2.service_id, "sc01");
    assert_eq!(&service2.service_type, "scabbard");
    assert_eq!(service2.allowed_nodes, vec!["bubba-node-000".to_string()]);
    let mut expected_service_args_service2 = expected_service_args;
    expected_service_args_service2.insert("peer_services".into(), "[\"sc00\"]".into());
    assert_eq!(service2.arguments, expected_service_args_service2);
}

/// Test that a `splinter circuit show` command with an invalid format argument (`--format invalid`)
/// fails with a `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn show_invalid_format() {
    match run_with_captured_output(
        "splinter circuit show circuit_id --url http://localhost:8088 --format invalid",
    ) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a `splinter circuit show` command without a circuit ID fails with a
/// `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn show_without_circuit_id() {
    match run_with_captured_output("splinter circuit show --url http://localhost:8088") {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}
