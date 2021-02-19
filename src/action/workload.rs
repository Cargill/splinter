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
// limitations under the License

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::ArgMatches;
use cylinder::Signer;
use rand::Rng;
use transact::families::smallbank::workload::{
    playlist::SmallbankGeneratingIter, SmallbankBatchWorkload, SmallbankTransactionWorkload,
};
use transact::workload::{WorkloadRunner, DEFAULT_LOG_TIME_SECS};

use crate::error::CliError;

use super::{create_cylinder_jwt_auth_signer_key, Action};

pub struct WorkloadAction;

impl Action for WorkloadAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let (auth, signer) = create_cylinder_jwt_auth_signer_key(key_path)?;

        let targets_vec: Vec<String> = args
            .values_of("targets")
            .map(|values| values.map(String::from).collect::<Vec<String>>())
            .ok_or_else(|| CliError::ActionError("'targets' are required".into()))?;

        let targets: Vec<Vec<String>> = targets_vec
            .iter()
            .map(|target| target.split(';').map(String::from).collect::<Vec<String>>())
            .collect::<Vec<Vec<String>>>();

        let rate = args.value_of("target_rate").unwrap_or("1").to_string();

        let (min, max): (u32, u32) = {
            if rate.contains('-') {
                let split_rate: Vec<String> = rate.split('-').map(String::from).collect();
                let min = split_rate
                    .get(0)
                    .ok_or_else(|| CliError::ActionError("Min target rate not provided".into()))?
                    .parse()
                    .map_err(|_| {
                        CliError::ActionError("Unable to parse provided min target rate".into())
                    })?;

                let max = split_rate
                    .get(1)
                    .ok_or_else(|| CliError::ActionError("Max target rate not provided".into()))?
                    .parse()
                    .map_err(|_| {
                        CliError::ActionError("Unable to parse provided max target rate".into())
                    })?;

                (min, max)
            } else {
                let min = rate.parse().map_err(|_| {
                    CliError::ActionError("Unable to parse provided target rate".into())
                })?;

                (min, min)
            }
        };

        let workload = args
            .value_of("workload")
            .ok_or_else(|| CliError::ActionError("Workload type is required".into()))?;

        let update: u32 = args
            .value_of("update")
            .unwrap_or(&DEFAULT_LOG_TIME_SECS.to_string())
            .parse()
            .map_err(|_| CliError::ActionError("Unable to parse provided update time".into()))?;

        let mut workload_runner = WorkloadRunner::default();

        match workload {
            "smallbank" => {
                let seed = match args
                    .value_of("smallbank_seed")
                    .map(str::parse)
                    .unwrap_or_else(|| {
                        let mut rng = rand::thread_rng();
                        Ok(rng.gen::<u64>())
                    }) {
                    Ok(seed) => seed,
                    Err(_) => {
                        return Err(CliError::ActionError(
                            "Unable to get seed for smallbank workload".into(),
                        ))
                    }
                };

                let num_accounts: usize = args
                    .value_of("smallbank_num_accounts")
                    .unwrap_or("100")
                    .parse()
                    .map_err(|_| {
                        CliError::ActionError("Unable to parse number of accounts".into())
                    })?;

                start_smallbank_workloads(
                    &mut workload_runner,
                    targets,
                    min,
                    max,
                    auth,
                    signer,
                    update,
                    seed,
                    num_accounts,
                )?;
            }
            _ => {
                return Err(CliError::ActionError(format!(
                    "Unsupported workload type: {}",
                    workload
                )))
            }
        }

        // setup control-c handling
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .map_err(|_| {
            CliError::ActionError("Unable to set up workload ctrlc handler".to_string())
        })?;

        while running.load(Ordering::SeqCst) {}
        // shutdown all workloads
        workload_runner.shutdown();

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn start_smallbank_workloads(
    workload_runner: &mut WorkloadRunner,
    targets: Vec<Vec<String>>,
    target_rate_min: u32,
    target_rate_max: u32,
    auth: String,
    signer: Box<dyn Signer>,
    update: u32,
    seed: u64,
    num_accounts: usize,
) -> Result<(), CliError> {
    let mut rng = rand::thread_rng();

    for (i, target) in targets.into_iter().enumerate() {
        let smallbank_generator = SmallbankGeneratingIter::new(num_accounts, seed);
        let transaction_workload =
            SmallbankTransactionWorkload::new(smallbank_generator, signer.clone());
        let smallbank_workload = SmallbankBatchWorkload::new(transaction_workload, signer.clone());

        let rate = if target_rate_min == target_rate_max {
            target_rate_min
        } else {
            rng.gen_range(target_rate_min..=target_rate_max)
        };

        info!(
            "Starting Smallbank-Workload-{} with target rate {}",
            i, rate
        );
        workload_runner
            .add_workload(
                format!("Smallbank-Workload-{}", i),
                Box::new(smallbank_workload),
                target,
                rate,
                auth.to_string(),
                update,
            )
            .map_err(|err| CliError::ActionError(format!("Unable to start workload: {}", err)))?
    }

    Ok(())
}
