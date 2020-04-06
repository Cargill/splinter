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

//! Tests for the `splinter circuit vote` subcommand.

use serial_test::serial;

use crate::CliError;

use super::{
    get_circuit_id_from_propose_output, get_key, repeat_until_true, run_with_captured_output,
    wait_until_circuits_created, wait_until_proposals_committed,
};

/// Test that a valid `splinter circuit vote --accept` is successful.
///
/// 1. Create a new proposal and wait for it to be committed.
/// 2. Vote on the circuit and verify that the circuit is created.
#[test]
#[serial(stdout)]
#[ignore]
fn vote_accept_successful() {
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

    // Vote on the circuit and verify that it's created
    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");
    wait_until_circuits_created("http://localhost:8088", &[&circuit_id]);
}

/// Test that a valid `splinter circuit vote --reject` is successful.
///
/// 1. Create a new proposal and wait for it to be committed.
/// 2. Vote on the circuit and verify that the proposal is removed.
#[test]
#[serial(stdout)]
#[ignore]
fn vote_reject_successful() {
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

    // Vote on the circuit and verify that the proposal is removed
    run_with_captured_output(&format!(
        "splinter circuit vote {} \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --reject",
        circuit_id,
    ))
    .expect("Failed to vote on circuit");

    repeat_until_true(|| {
        let output =
            run_with_captured_output("splinter circuit proposals --url http://localhost:8089")
                .expect("Failed to get proposals");
        !output.contains(&circuit_id)
    })
}

/// Test that a `splinter circuit vote` for a non-existent circuit fails with a
/// `Err(CliError::ActionError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn vote_non_existent_circuit() {
    match run_with_captured_output(
        "splinter circuit vote abcde-01234 \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
    ) {
        Err(CliError::ActionError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a `splinter circuit vote` without a circuit ID fails with a
/// `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn vote_without_circuit_id() {
    match run_with_captured_output(
        "splinter circuit vote \
         --url http://localhost:8089 \
         --key /tmp/bob.priv \
         --accept",
    ) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a `splinter circuit vote` without `--accept` or `--reject` fails with a
/// `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn vote_without_accept_or_reject() {
    match run_with_captured_output(
        "splinter circuit vote circuit_id --url http://localhost:8089 --key /tmp/bob.priv",
    ) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}
