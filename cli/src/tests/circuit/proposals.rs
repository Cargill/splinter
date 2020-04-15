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

//! Tests for the `splinter circuit proposals` subcommand.

use serial_test::serial;

use crate::CliError;

use super::{
    get_circuit_id_from_propose_output, get_key, run_with_captured_output,
    wait_until_proposals_committed,
};

/// Test that a basic `splinter circuit proposals` command is successful.
///
/// 1. Make sure at least one proposal exists by proposing a new circuit and waiting for the
///    proposal to be committed.
/// 2. Run the `splinter circuit proposals` command without any special arguments and verify that
///    the proposal appears in the output along with the correct circuit management type, members,
///    and proposal comment.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_successful_basic() {
    // Submit a new proposal, get the circuit ID, and wait for the proposal to be committed
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);

    // List the proposals and verify the new proposal is in the list
    let output = run_with_captured_output("splinter circuit proposals --url http://localhost:8088")
        .expect("Failed to get proposals");
    let proposal = output
        .split('\n')
        .find(|line| line.contains(&circuit_id))
        .expect("Proposal not found")
        .to_string();

    // Management
    assert!(proposal.contains("custom"));
    // Members
    assert!(proposal.contains("acme-node-000"));
    assert!(proposal.contains("bubba-node-000"));
    // Comments
    assert!(proposal.contains("test_comment"));
}

/// Test that a `splinter circuit proposals` command with `--management-type` filter works properly.
///
/// 1. Propose two new circuits with different management types and wait for the proposals to be
///    committed.
/// 2. Run the `splinter circuit proposals` command with the `--management-type` option and verify
///    that the proposal with the matching management type appears in the output, but not the other
///    proposal.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_successful_with_management_type() {
    // Submit two new proposals with different management types, get the circuit IDs, and wait for
    // the proposals to be committed
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management test1 \
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
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management test2 \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit2");
    let circuit_id2 = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id1, &circuit_id2]);

    // List the proposals with `--management-type test2` and verify that only the second proposal
    // shows up
    let output = run_with_captured_output(
        "splinter circuit proposals --url http://localhost:8088 --management-type test2",
    )
    .expect("Failed to get proposals");
    let proposal = output
        .split('\n')
        .find(|line| line.contains(&circuit_id2))
        .expect("Proposal not found")
        .to_string();

    assert!(proposal.contains("test2"));
    assert!(!output.contains(&circuit_id1));
}

/// Test that the human-readable output of the `splinter circuit proposals` command is correct and
/// that it is the default.
///
/// 1. Make sure at least one proposal exists by proposing a new circuit and waiting for the
///    proposal to be committed.
/// 2. Run the `splinter circuit list` command without any special arguments and verify that the
///    appropriate headers appear in the output separated by whitespace, and that each entry has
///    the correct number of fields separated by whitespace.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_format_human() {
    // Make sure at least one proposal exists
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);

    // Verify the human-readable output
    let output = run_with_captured_output("splinter circuit proposals --url http://localhost:8088")
        .expect("Failed to get proposals");
    let mut lines = output.split('\n');
    let mut headers = lines
        .next()
        .expect("No proposals output")
        .split_whitespace();
    assert_eq!(headers.next(), Some("ID"));
    assert_eq!(headers.next(), Some("MANAGEMENT"));
    assert_eq!(headers.next(), Some("MEMBERS"));
    assert_eq!(headers.next(), Some("COMMENTS"));

    for line in lines {
        if !line.is_empty() {
            // If a proposal doesn't have a comment, there will only be three entries
            let num_entries = line.split_whitespace().count();
            assert!(num_entries == 3 || num_entries == 4);
        }
    }
}

/// Test that the csv output of the `splinter circuit proposals` command is correct.
///
/// 1. Make sure at least one proposal exists by proposing a new circuit and waiting for the
///    proposal to be committed.
/// 2. Run the `splinter circuit list` command with the `--format csv` argument and verify that the
///    appropriate headers appear in the output separated by commas, and that each entry has the
///    correct number of fields separated by commas.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_format_csv() {
    // Make sure at least one proposal exists
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");
    let circuit_id = get_circuit_id_from_propose_output(&output);
    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);

    // Verify the csv output
    let output = run_with_captured_output(
        "splinter circuit proposals --url http://localhost:8088 --format csv",
    )
    .expect("Failed to get proposals");
    let mut lines = output.split('\n');
    let mut headers = lines.next().expect("No proposals output").split(',');
    assert_eq!(headers.next(), Some("ID"));
    assert_eq!(headers.next(), Some("MANAGEMENT"));
    assert_eq!(headers.next(), Some("MEMBERS"));
    assert_eq!(headers.next(), Some("COMMENTS"));

    for line in lines {
        if !line.is_empty() {
            // If a proposal doesn't have a comment, there will only be three entries
            let num_entries = line.split(',').count();
            assert!(num_entries == 3 || num_entries == 4);
        }
    }
}

/// Test that a `splinter circuit proposals` command with an invalid format argument
/// (`--format invalid`) fails with a `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn proposals_invalid_format() {
    match run_with_captured_output(
        "splinter circuit proposals --url http://localhost:8088 --format invalid",
    ) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}
