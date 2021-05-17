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

use std::fs::File;
use std::io::Write;
use std::time::Duration;

use clap::ArgMatches;
use transact::families::smallbank::workload::playlist::{
    generate_smallbank_playlist, process_smallbank_playlist,
};
use transact::workload::batch_gen::generate_signed_batches;
use transact::workload::{submit_batches_from_source, DEFAULT_LOG_TIME_SECS};

use crate::action::rate::Rate;
use crate::error::CliError;

use super::{create_cylinder_jwt_auth_signer_key, load_cylinder_signer_key, Action};

const DEFAULT_ACCOUNTS: &str = "10";
const DEFAULT_TRANSACTIONS: &str = "10";
const DEFAULT_BATCH_SIZE: &str = "1";
const DEFAULT_RATE: &str = "1/s";

pub struct CreatePlaylistAction;

impl Action for CreatePlaylistAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let mut output_writer: Box<dyn Write> = match args.value_of("output") {
            Some(file_name) => File::create(file_name).map(Box::new).map_err(|_| {
                CliError::ActionError("Unable to create provided output file".to_string())
            })?,
            None => Box::new(std::io::stdout()),
        };

        let workload = args
            .value_of("workload")
            .ok_or_else(|| CliError::ActionError("Workload type is required".into()))?;

        match workload {
            "smallbank" => {
                let num_accounts = args
                    .value_of("smallbank_num_account")
                    .unwrap_or(DEFAULT_ACCOUNTS)
                    .parse()
                    .map_err(|_| {
                        CliError::ActionError("Unable to parse number of accounts".into())
                    })?;

                if num_accounts < 2 {
                    return Err(CliError::ActionError(
                        "'accounts' must be a number greater than 2".to_string(),
                    ));
                }

                let num_transactions = args
                    .value_of("transactions")
                    .unwrap_or(DEFAULT_TRANSACTIONS)
                    .parse()
                    .map_err(|_| {
                        CliError::ActionError("Unable to parse number of accounts".into())
                    })?;

                let random_seed = match args.value_of("smallbank_seed") {
                    Some(seed) => match seed.parse::<i32>() {
                        Ok(n) => Some(n),
                        Err(_) => {
                            return Err(CliError::ActionError(
                                "'seed' must be a valid number".to_string(),
                            ))
                        }
                    },
                    None => None,
                };

                generate_smallbank_playlist(
                    &mut *output_writer,
                    num_accounts,
                    num_transactions,
                    random_seed,
                )
                .map_err(|err| {
                    CliError::ActionError(format!("Unable to generate smallbank playlist: {}", err))
                })?;

                Ok(())
            }
            _ => Err(CliError::ActionError(format!(
                "Unsupported workload type: {}",
                workload
            ))),
        }
    }
}

pub struct ProcessPlaylistAction;

impl Action for ProcessPlaylistAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let mut in_file = File::open(
            args.value_of("input")
                .ok_or_else(|| CliError::ActionError("'input' is required".into()))?,
        )
        .map_err(|_| CliError::ActionError("Unable to open input file".to_string()))?;

        let mut output_writer: Box<dyn Write> = match args.value_of("output") {
            Some(file_name) => File::create(file_name).map(Box::new).map_err(|_| {
                CliError::ActionError("Unable to create provided output file".to_string())
            })?,
            None => Box::new(std::io::stdout()),
        };

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let signer = load_cylinder_signer_key(key_path)?;

        let workload = args
            .value_of("workload")
            .ok_or_else(|| CliError::ActionError("Workload type is required".into()))?;

        match workload {
            "smallbank" => process_smallbank_playlist(&mut output_writer, &mut in_file, &*signer)
                .map_err(|err| {
                CliError::ActionError(format!("Unable to processes smallbank playlist: {}", err))
            })?,
            _ => {
                return Err(CliError::ActionError(format!(
                    "Unsupported workload type: {}",
                    workload
                )))
            }
        }

        Ok(())
    }
}

pub struct BatchPlaylistAction;

impl Action for BatchPlaylistAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let max_txns: usize = args
            .value_of("max-batch-size")
            .unwrap_or(DEFAULT_BATCH_SIZE)
            .parse()
            .map_err(|_| CliError::ActionError("Unable to parse provided max-batch-size".into()))?;

        if max_txns == 0 {
            return Err(CliError::ActionError(
                "max-batch-size must be a number greater than 0".to_string(),
            ));
        }

        let mut in_file = File::open(
            args.value_of("input")
                .ok_or_else(|| CliError::ActionError("'input' is required".into()))?,
        )
        .map_err(|_| CliError::ActionError("Unable to open input file".to_string()))?;

        let mut out_file = File::create(
            args.value_of("output")
                .ok_or_else(|| CliError::ActionError("'output' is required".into()))?,
        )
        .map_err(|_| CliError::ActionError("Unable to open output file".to_string()))?;

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let signer = load_cylinder_signer_key(key_path)?;

        generate_signed_batches(&mut in_file, &mut out_file, max_txns, &*signer).map_err(
            |err| CliError::ActionError(format!("Unable to generate signed batches: {}", err)),
        )?;

        Ok(())
    }
}

pub struct SubmitPlaylistAction;

impl Action for SubmitPlaylistAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let (auth, _) = create_cylinder_jwt_auth_signer_key(key_path)?;

        let rate_string = args.value_of("rate").unwrap_or(DEFAULT_RATE);
        let rate: Duration = if let Ok(interval) = rate_string.parse::<Rate>() {
            interval.into()
        } else {
            let raw_num = rate_string.parse::<f32>().map_err(|_| {
                CliError::UnparseableArg("time must be floating point value".into())
            })?;
            std::time::Duration::from_secs_f32(1.0 / raw_num)
        };

        if rate == Duration::from_secs(0) {
            return Err(CliError::ActionError(
                "rate must be a number greater than 0".to_string(),
            ));
        }

        let target = args
            .value_of("target")
            .ok_or_else(|| CliError::ActionError("'targets' are required".into()))?;

        let input = args
            .value_of("input")
            .ok_or_else(|| CliError::ActionError("'input' is required".into()))?;

        let mut in_file = File::open(&input)
            .map_err(|_| CliError::ActionError("Unable to open input file".to_string()))?;

        info!(
            "Input: {} Target: {:?} Rate: {}",
            input,
            target,
            1000 / rate.as_millis()
        );

        let update: u32 = args
            .value_of("update")
            .unwrap_or(&DEFAULT_LOG_TIME_SECS.to_string())
            .parse()
            .map_err(|_| CliError::ActionError("Unable to parse provided update time".into()))?;

        let target_vec: Vec<String> = target.split(';').map(String::from).collect();
        submit_batches_from_source(
            &mut in_file,
            input.to_string(),
            target_vec,
            rate,
            auth,
            update,
        );

        Ok(())
    }
}
