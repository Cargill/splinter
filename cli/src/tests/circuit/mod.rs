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

//! This module provides integration tests for the `splinter circuit` subcommands.
//!
//! These tests are currently disabled because they are unreliable and/or they require special
//! environment setup like modifying local files, running docker containers, and not capturing
//! output.
//!
//! The tests in this module are run serially using the `serial_test` crate with the
//! `#[serial(stdout)]` annotation added to the individual tests. This is required because stdout
//! cannot be capture more than once at a time and stdout output from one test could show up in
//! another if run in parallel.

mod list;
mod proposals;
mod propose;
mod show;
mod vote;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::thread::sleep;
use std::time::{Duration, Instant};

use gag::BufferRedirect;
use serde::{Deserialize, Serialize};

use crate::{run, CliError};

/// Call the `run` function with the given command and return any error that occurs. Return stdout
/// on success.
fn run_with_captured_output(command: &str) -> Result<String, CliError> {
    let mut stdout = BufferRedirect::stdout().expect("Failed to capture stdout");
    run(command.split_whitespace())?;
    let mut stdout_output = String::new();
    stdout
        .read_to_string(&mut stdout_output)
        .expect("Failed to read stdout");
    Ok(stdout_output)
}

/// Get the ciricuit ID from the output of a successful `splinter circuit propose`.
fn get_circuit_id_from_propose_output(output: &str) -> String {
    output
        .split_whitespace()
        .skip_while(|item| item != &"Circuit:")
        .skip(1)
        .next()
        .expect("Circuit ID not found in output")
        .to_string()
}

/// Polls the splinter REST API at the given URL until all of the given circuit IDs show up in the
/// node's list of proposals. The poll occurs every second up to 60 seconds before timing out.
fn wait_until_proposals_committed(url: &str, circuit_ids: &[&str]) {
    repeat_until_true(|| {
        let output = run_with_captured_output(&format!("splinter circuit proposals --url {}", url))
            .expect("Failed to get proposals");
        circuit_ids
            .iter()
            .all(|id| output.split('\n').find(|line| line.contains(id)).is_some())
    })
}

/// Polls the splinter REST API at the given URL until all of the given circuit IDs show up in the
/// node's list of circuits. The poll occurs every second up to 60 seconds before timing out.
fn wait_until_circuits_created(url: &str, circuit_ids: &[&str]) {
    repeat_until_true(|| {
        let output = run_with_captured_output(&format!("splinter circuit list --url {}", url))
            .expect("Failed to get circuits");
        circuit_ids
            .iter()
            .all(|id| output.split('\n').find(|line| line.contains(id)).is_some())
    })
}

/// Call the given function every second until it returns `true`. If the function does not return
/// `true` after 60 seconds, the function will timeout and panic.
fn repeat_until_true<F: Fn() -> bool>(func: F) {
    let timeout = Instant::now() + Duration::from_secs(60);
    loop {
        if func() {
            return;
        } else if Instant::now() >= timeout {
            panic!("Timed out");
        } else {
            sleep(Duration::from_millis(1000))
        }
    }
}

/// Load the key from the key file at the given path.
fn get_key(path: &str) -> String {
    let mut key = String::new();
    let mut file = File::open(path).expect("Failed to open key file");
    file.read_to_string(&mut key).expect("Failed to read key");
    key = key.trim().into();
    key
}

/// Local representation of the `splinter circuit show` response
#[derive(Serialize, Deserialize)]
struct Circuit {
    pub id: String,
    pub members: Vec<String>,
    pub roster: Vec<Service>,
    pub management_type: String,
}

#[derive(Serialize, Deserialize)]
struct Service {
    pub service_id: String,
    pub service_type: String,
    pub allowed_nodes: Vec<String>,
    pub arguments: BTreeMap<String, String>,
}
