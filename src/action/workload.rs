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

use crate::action::time::{Time, TimeType, TimeUnit};
use std::time::Duration;

use clap::ArgMatches;
use cylinder::Signer;
use rand::Rng;
use transact::families::{
    command::workload::{CommandBatchWorkload, CommandGeneratingIter, CommandTransactionWorkload},
    smallbank::workload::{
        playlist::SmallbankGeneratingIter, SmallbankBatchWorkload, SmallbankTransactionWorkload,
    },
};
use transact::workload::WorkloadRunner;

use crate::error::CliError;

use super::{create_cylinder_jwt_auth_signer_key, Action, DEFAULT_LOG_TIME_SECS};

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

        let rate = args.value_of("target_rate").unwrap_or("1/s").to_string();

        let (min, max): (Time, Time) = {
            if rate.contains('-') {
                let split_rate: Vec<String> = rate.split('-').map(String::from).collect();
                let min_string = split_rate
                    .get(0)
                    .ok_or_else(|| CliError::ActionError("Min target rate not provided".into()))?;
                let max_string = split_rate
                    .get(1)
                    .ok_or_else(|| CliError::ActionError("Max target rate not provided".into()))?;

                let min = min_string
                    .parse::<Time>()
                    .or_else(|_| min_string.parse::<f64>().map(Time::from))
                    .map_err(|_| {
                        CliError::UnparseableArg("Unable to parse provided min target rate".into())
                    })?;

                let max = max_string
                    .parse::<Time>()
                    .or_else(|_| max_string.parse::<f64>().map(Time::from))
                    .map_err(|_| {
                        CliError::UnparseableArg("Unable to parse provided max target rate".into())
                    })?;

                (min, max)
            } else {
                let min = rate
                    .parse()
                    .or_else(|_| rate.parse::<f64>().map(Time::from))
                    .map_err(|_| {
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

        let seed = match args.value_of("seed").map(str::parse).unwrap_or_else(|| {
            let mut rng = rand::thread_rng();
            Ok(rng.gen::<u64>())
        }) {
            Ok(seed) => seed,
            Err(_) => {
                return Err(CliError::ActionError(
                    "Unable to get seed for workload".into(),
                ))
            }
        };

        let duration = args
            .value_of("duration")
            .map(|d| {
                Time::make_duration_type_time(d)
                    .map_err(|err| CliError::ActionError(format!("{}", err)))
            })
            .transpose()?;

        let mut workload_runner = WorkloadRunner::default();

        match workload {
            "smallbank" => {
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
                    duration,
                )?;
            }
            "command" => {
                start_command_workloads(
                    &mut workload_runner,
                    targets,
                    min,
                    max,
                    auth,
                    signer,
                    update,
                    seed,
                    duration,
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
        let workload_runner_shutdown_signaler = workload_runner.shutdown_signaler();

        ctrlc::set_handler(move || {
            if let Err(err) = workload_runner_shutdown_signaler.signal_shutdown() {
                error!("Unable to cleanly shutdown workload: {}", err);
            }
        })
        .map_err(|_| {
            CliError::ActionError("Unable to set up workload ctrlc handler".to_string())
        })?;

        if let Err(err) = workload_runner.wait_for_shutdown() {
            error!("Unable to cleanly shutdown workload runner: {}", err);
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn start_smallbank_workloads(
    workload_runner: &mut WorkloadRunner,
    targets: Vec<Vec<String>>,
    target_rate_min: Time,
    target_rate_max: Time,
    auth: String,
    signer: Box<dyn Signer>,
    update: u32,
    seed: u64,
    num_accounts: usize,
    total_duration: Option<Time>,
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
            // Calculate the amount of time, in milliseconds, to wait between batch submissions for
            // the min and max target rates and generate a random number between the two times
            let time_to_wait =
                rng.gen_range(target_rate_max.to_milli()..=target_rate_min.to_milli());
            // Calculate the number of batches that should be submitted per second with the new time
            let numeric = 1000.0 / time_to_wait;
            Time {
                numeric,
                unit: TimeUnit::Second,
                time_type: TimeType::Rate,
            }
        };

        info!(
            "Starting Smallbank-Workload-{} with target rate {} and duration {}",
            i,
            rate,
            total_duration.map_or("indefinite".into(), |t| format!("{}", t))
        );

        let duration = total_duration.map(Duration::from);

        workload_runner
            .add_workload(
                format!("Smallbank-Workload-{}", i),
                Box::new(smallbank_workload),
                target,
                rate.into(),
                auth.to_string(),
                update,
                false,
                duration,
            )
            .map_err(|err| CliError::ActionError(format!("Unable to start workload: {}", err)))?
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn start_command_workloads(
    workload_runner: &mut WorkloadRunner,
    targets: Vec<Vec<String>>,
    target_rate_min: Time,
    target_rate_max: Time,
    auth: String,
    signer: Box<dyn Signer>,
    update: u32,
    seed: u64,
    total_duration: Option<Time>,
) -> Result<(), CliError> {
    let mut rng = rand::thread_rng();

    for (i, target) in targets.into_iter().enumerate() {
        let command_generator = CommandGeneratingIter::new(seed);
        let transaction_workload =
            CommandTransactionWorkload::new(command_generator, signer.clone());
        let command_workload = CommandBatchWorkload::new(transaction_workload, signer.clone());

        let rate = if target_rate_min == target_rate_max {
            target_rate_min
        } else {
            // Calculate the amount of time, in milliseconds, to wait between batch submissions for
            // the min and max target rates and generate a random number between the two times
            let time_to_wait =
                rng.gen_range(target_rate_max.to_milli()..=target_rate_min.to_milli());
            // Calculate the number of batches that should be submitted per second with the new time
            let numeric = 1000.0 / time_to_wait;
            Time {
                numeric,
                unit: TimeUnit::Second,
                time_type: TimeType::Rate,
            }
        };

        info!(
            "Starting Command-Workload-{} with target rate {} and duration {}",
            i,
            rate,
            total_duration.map_or("indefinite".into(), |t| format!("{}", t))
        );

        let duration = total_duration.map(Duration::from);

        workload_runner
            .add_workload(
                format!("Command-Workload-{}", i),
                Box::new(command_workload),
                target,
                rate.into(),
                auth.to_string(),
                update,
                true,
                duration,
            )
            .map_err(|err| CliError::ActionError(format!("Unable to start workload: {}", err)))?
    }

    Ok(())
}
