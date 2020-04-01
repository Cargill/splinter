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

//! Tests for the `splinter circuit list` subcommand.

use serial_test::serial;

use crate::CliError;

use super::{
    get_circuit_id_from_propose_output, get_key, run_with_captured_output,
    wait_until_circuits_created, wait_until_proposals_committed,
};

/// Test that a basic `splinter circuit list` is successful.
///
/// 1. Make sure at least two circuits exist by proposing and voting on two new circuits and
///    waiting for them to be created.
/// 2. Run the `splinter circuit list` command without any special arguments and verify that the
///    two new circuits appear in the output along with the correct circuit management types and
///    members.
#[test]
#[serial(stdout)]
#[ignore]
fn list_successful() {
    // Make sure at least two circuits exist
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
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit1");
    let circuit_id1 = get_circuit_id_from_propose_output(&output);

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
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit2");
    let circuit_id2 = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id1, &circuit_id2]);

    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id1,
    ))
    .expect("Failed to vote on circuit1");

    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id2,
    ))
    .expect("Failed to vote on circuit2");

    wait_until_circuits_created("http://localhost:8088", &[&circuit_id1, &circuit_id2]);

    // List circuits and verify the new circuits are in the list
    let output = run_with_captured_output("splinter circuit list --url http://localhost:8088")
        .expect("Failed to get circuits");

    let circuit1 = output
        .split('\n')
        .find(|line| line.contains(&circuit_id1))
        .expect("Circuit1 not found")
        .to_string();
    let circuit2 = output
        .split('\n')
        .find(|line| line.contains(&circuit_id2))
        .expect("Circuit2 not found")
        .to_string();

    // Management
    assert!(circuit1.contains("custom"));
    assert!(circuit2.contains("custom"));
    // Members
    assert!(circuit1.contains("acme-node-000"));
    assert!(circuit1.contains("bubba-node-000"));
    assert!(circuit2.contains("acme-node-000"));
    assert!(circuit2.contains("bubba-node-000"));
}

/// Test that the human-readable format of `splinter circuit list` is correct and that it is the
/// default.
///
/// 1. Make sure at least one circuits exists by proposing and voting on a new circuit and waiting
///    for it to be created.
/// 2. Run the `splinter circuit list` command without any special arguments and verify that the
///    appropriate headers appear in the output separated by whitespace, and that each entry has
///    the correct number of fields separated by whitespace.
#[test]
#[serial(stdout)]
#[ignore]
fn list_format_human() {
    // Make sure at least one circuit exists
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
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id]);

    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");
    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);

    // List circuits and verify the output
    let output = run_with_captured_output("splinter circuit list --url http://localhost:8088")
        .expect("Failed to get circuits");
    let mut lines = output.split('\n');
    let mut headers = lines.next().expect("No list output").split_whitespace();
    assert_eq!(headers.next(), Some("ID"));
    assert_eq!(headers.next(), Some("MANAGEMENT"));
    assert_eq!(headers.next(), Some("MEMBERS"));

    for line in lines {
        if !line.is_empty() {
            assert!(line.split_whitespace().count() == 3);
        }
    }
}

/// Test that the `csv` format of `splinter circuit list` is correct.
///
/// 1. Make sure at least one circuits exists by proposing and voting on a new circuit and waiting
///    for it to be created.
/// 2. Run the `splinter circuit list` command with the `--format csv` argument and verify that the
///    appropriate headers appear in the output separated by commas, and that each entry has the
///    correct number of fields separated by commas.
#[test]
#[serial(stdout)]
#[ignore]
fn list_format_csv() {
    // Make sure at least one circuit exists
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
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8089", &[&circuit_id]);

    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");
    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);

    // List circuits and verify the output
    let output =
        run_with_captured_output("splinter circuit list --url http://localhost:8088 --format csv")
            .expect("Failed to get circuits");
    let mut lines = output.split('\n');
    let mut headers = lines.next().expect("No list output").split(',');
    assert_eq!(headers.next(), Some("ID"));
    assert_eq!(headers.next(), Some("MANAGEMENT"));
    assert_eq!(headers.next(), Some("MEMBERS"));

    for line in lines {
        if !line.is_empty() {
            assert!(line.split(',').count() == 3);
        }
    }
}

/// Test that a `splinter circuit list` command with an invalid format argument (`--format invalid`)
/// fails with a `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_invalid_format() {
    match run_with_captured_output(
        "splinter circuit list --url http://localhost:8088 --format invalid",
    ) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}
