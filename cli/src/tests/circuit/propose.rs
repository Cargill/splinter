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

//! Tests for the `splinter circuit propose` subcommand.

use serial_test::serial;

use crate::CliError;

use super::{
    get_circuit_id_from_propose_output, get_key, run_with_captured_output,
    wait_until_proposals_committed,
};

/// Test that a basic circuit proposal with two nodes (specified with `--node`) and two services is
/// submitted successfully.
///
/// 1. Submit a proposal using the `splinter circuit propose` command and verify that the command
///    is successfully run.
/// 2. Verify that the proposal summary is output correctly by inspecting the output for the node
///    IDs, service IDs, service types, and `admin_keys` service argument.
/// 3. Verify that the proposal is committed.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_successful_basic() {
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
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);
}

/// Test that a valid circuit proposal using the `--node-file` argument is submitted successfully.
///
/// 1. Submit a proposal using the `splinter circuit propose` command with the `--node-file`
///    argument and verify that the command is successfully run.
/// 2. Verify that the proposal summary is output correctly by inspecting the output for the node
///    IDs, service IDs, service types, and `admin_keys` service argument.
/// 3. Verify that the proposal is committed.
///
/// NOTE: this test is currently broken, since the registry file used no longer exists and must be
/// generated.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_successful_with_node_file() {
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node-file {}/../examples/gameroom/registry/registry.yaml \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01",
        env!("CARGO_MANIFEST_DIR"),
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);
}

/// Test that a valid circuit proposal using individual service types (rather than a wildcard for
/// service IDs) is submitted successfully.
///
/// 1. Submit a proposal using the `splinter circuit propose` command with individually specified
///    service types and verify that the command is successfully run.
/// 2. Verify that the proposal summary is output correctly by inspecting the output for the node
///    IDs, service IDs, service types, and `admin_keys` service argument.
/// 3. Verify that the proposal is committed.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_successful_with_individual_service_types() {
    let output = run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type sc00::scabbard \
         --service-type sc01::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);
}

/// Test that a valid circuit proposal using individual service args (rather than a wildcard for
/// service IDs) is submitted successfully.
///
/// 1. Submit a proposal using the `splinter circuit propose` command with individually specified
///    service arguments and verify that the command is successfully run.
/// 2. Verify that the proposal summary is output correctly by inspecting the output for the node
///    IDs, service IDs, service types, and `admin_keys` service argument.
/// 3. Verify that the proposal is committed.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_successful_with_individual_service_args() {
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
         --service-arg sc00::admin_keys={} \
         --service-arg sc01::admin_keys={} \
         --service-peer-group sc00,sc01",
        get_key("/tmp/alice.pub"),
        get_key("/tmp/bob.pub"),
    ))
    .expect("Failed to propose circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);
}

/// Test that a valid circuit proposal with JSON metadata is submitted successfully.
///
/// 1. Submit a proposal using the `splinter circuit propose` command with two key/value metadata
///    entries and the `--metadata-encoding json` argument; verify that the command is successfully
///    run.
/// 2. Verify that the proposal summary is output correctly by inspecting the output for the node
///    IDs, service IDs, service types, and `admin_keys` service argument.
/// 3. Verify that the proposal is committed.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_successful_with_json_metadata() {
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
         --metadata key1=value1 \
         --metadata key2=value2 \
         --metadata-encoding json",
        get_key("/tmp/alice.pub"),
    ))
    .expect("Failed to propose circuit");

    assert!(output.contains("acme-node-000"));
    assert!(output.contains("bubba-node-000"));
    assert!(output.contains("sc00"));
    assert!(output.contains("sc01"));
    assert!(output.contains("scabbard"));
    assert!(output.contains(&get_key("/tmp/alice.pub")));

    let circuit_id = get_circuit_id_from_propose_output(&output);

    wait_until_proposals_committed("http://localhost:8088", &[&circuit_id]);
}

/// Test that a circuit proposal without either the `--node` or `--node-file` arguments fails with
/// a `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_without_nodes() {
    match run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --service sc00::acme-node-000 \
         --service sc01::bubba-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00,sc01 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    )) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a circuit proposal with fewer than 2 services fails with a
/// `Err(CliError::ClapError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_too_few_services() {
    // 0 services
    match run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    )) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };

    // 1 service
    match run_with_captured_output(&format!(
        "splinter circuit propose \
         --url http://localhost:8088 \
         --key /tmp/alice.priv \
         --node acme-node-000::tcps://splinterd-node-acme:8044 \
         --node bubba-node-000::tcps://splinterd-node-bubba:8044 \
         --service sc00::acme-node-000 \
         --service-type *::scabbard \
         --management custom \
         --service-arg *::admin_keys={} \
         --service-peer-group sc00 \
         --metadata test_metadata \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    )) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a circuit proposal with multiple string metadata entries fails with a
/// `Err(CliError::ActionError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_multiple_string_metadata() {
    match run_with_captured_output(&format!(
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
         --metadata test_metadata1 \
         --metadata test_metadata2 \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    )) {
        Err(CliError::ActionError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}

/// Test that a circuit proposal with `--metadata-encoding` set but not `--metadata` fails with a
/// `Err(CliError::ActionError(_))` result.
#[test]
#[serial(stdout)]
#[ignore]
fn propose_metadata_encoding_without_metadata() {
    match run_with_captured_output(&format!(
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
         --metadata-encoding json \
         --comments test_comment",
        get_key("/tmp/alice.pub"),
    )) {
        Err(CliError::ClapError(_)) => {}
        res => panic!("Got unexpected result: {:?}", res),
    };
}
