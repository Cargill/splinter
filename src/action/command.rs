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

use clap::ArgMatches;
use protobuf::{Message, RepeatedField};
use reqwest::{blocking::Client, header};
use serde::Deserialize;
use transact::protocol::batch::BatchPair;
use transact::protocol::sabre::ExecuteContractActionBuilder;
use transact::protos::FromProto;
use transact::protos::{
    batch::{Batch, BatchHeader},
    command::{BytesEntry, Command, CommandPayload, Command_CommandType, GetState, SetState},
    IntoBytes, IntoProto,
};

use crate::error::CliError;

use super::{create_cylinder_jwt_auth_signer_key, Action};

pub struct CommandSetStateAction;

impl Action for CommandSetStateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let (auth, signer) = create_cylinder_jwt_auth_signer_key(key_path)?;

        let target = args
            .value_of("target")
            .ok_or_else(|| CliError::ActionError("'target' is required".into()))?;

        let bytes_entries = args
            .values_of("state-entry")
            .ok_or_else(|| CliError::ActionError("'state-entry' is required".into()))?;

        let mut state_writes = Vec::new();
        let mut addresses = Vec::new();

        for bytes_entry in bytes_entries {
            let (address, value) = parse_bytes_entry(bytes_entry)?;

            let mut entry = BytesEntry::new();
            entry.set_key(address.clone());
            entry.set_value(value);

            state_writes.push(entry);

            addresses.push(address);
        }

        let mut set_state = SetState::new();
        set_state.set_state_writes(RepeatedField::from_vec(state_writes));

        let mut command = Command::new();
        command.set_command_type(Command_CommandType::SET_STATE);
        command.set_set_state(set_state);

        // build the command payload
        let mut payload = CommandPayload::new();
        payload.set_commands(RepeatedField::from_vec(vec![command]));

        let payload_bytes = payload.write_to_bytes().map_err(|err| {
            CliError::ActionError(format!("Unable to convert payload to bytes: {}", err))
        })?;

        // build the transaction
        let txn = ExecuteContractActionBuilder::new()
            .with_name(String::from("command"))
            .with_version(String::from("1.0"))
            .with_inputs(addresses.clone())
            .with_outputs(addresses)
            .with_payload(payload_bytes)
            .into_payload_builder()
            .map_err(|err| {
                CliError::ActionError(format!(
                    "Unable to convert execute action into sabre payload: {}",
                    err
                ))
            })?
            .into_transaction_builder()
            .map_err(|err| {
                CliError::ActionError(format!(
                    "Unable to convert execute payload into transaction: {}",
                    err
                ))
            })?
            .build(&*signer)
            .map_err(|err| CliError::ActionError(format!("Unable to build transaction: {}", err)))?
            .into_proto()
            .map_err(|err| {
                CliError::ActionError(format!(
                    "Unable to convert transaction to protobuf: {}",
                    err
                ))
            })?;

        // create the batch header
        let mut batch_header = BatchHeader::new();
        let txn_id = txn.get_header_signature().to_string();
        batch_header.set_transaction_ids(RepeatedField::from_vec(vec![txn_id]));
        batch_header.set_signer_public_key(
            signer
                .public_key()
                .map_err(|err| {
                    CliError::ActionError(format!("Unable to get signer public key: {}", err))
                })?
                .as_hex(),
        );

        let header_bytes = batch_header.write_to_bytes().map_err(|err| {
            CliError::ActionError(format!("Unable to convert header to bytes: {}", err))
        })?;

        // build the batch
        let mut batch = Batch::new();
        let signature = signer
            .sign(&header_bytes)
            .map_err(|err| CliError::ActionError(format!("Unable to sign batch: {}", err)))?
            .as_hex();
        batch.set_header_signature(signature);
        batch.set_header(header_bytes.to_vec());
        batch.set_transactions(RepeatedField::from_vec(vec![txn]));

        let batch_pair = BatchPair::from_proto(batch).map_err(|err| {
            CliError::ActionError(format!(
                "Unable to convert to batch pair from batch proto: {}",
                err
            ))
        })?;

        let batch_bytes = match vec![batch_pair.batch().clone()].into_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(CliError::ActionError(format!(
                    "Unable to convert batch to bytes: {}",
                    err
                )))
            }
        };

        // send batch to target
        Client::new()
            .post(&format!("{}/batches", target))
            .header(header::CONTENT_TYPE, "octet-stream")
            .header("Authorization", auth)
            .body(batch_bytes)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to submit set state transaction: {}", err))
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Set state request failed with status code '{}', but \
                                    error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to submit set state transaction: {}",
                        message
                    )))
                }
            })
    }
}

pub struct CommandGetStateAction;

impl Action for CommandGetStateAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_path = args
            .value_of("key")
            .ok_or_else(|| CliError::ActionError("'key' is required".into()))?;
        let (auth, signer) = create_cylinder_jwt_auth_signer_key(key_path)?;

        let target = args
            .value_of("target")
            .ok_or_else(|| CliError::ActionError("'target' is required".into()))?;

        let addresses = args
            .values_of("address")
            .ok_or_else(|| CliError::ActionError("'address' is required".into()))?;

        let mut get_state = GetState::new();
        let state_keys = addresses.map(String::from).collect::<Vec<String>>();

        get_state.set_state_keys(RepeatedField::from_vec(state_keys.clone()));

        let mut command = Command::new();
        command.set_command_type(Command_CommandType::GET_STATE);
        command.set_get_state(get_state);

        // build the command payload
        let mut payload = CommandPayload::new();
        payload.set_commands(RepeatedField::from_vec(vec![command]));

        let payload_bytes = payload.write_to_bytes().map_err(|err| {
            CliError::ActionError(format!("Unable to convert payload to bytes: {}", err))
        })?;

        // build the transaction
        let txn = ExecuteContractActionBuilder::new()
            .with_name(String::from("command"))
            .with_version(String::from("1.0"))
            .with_inputs(state_keys.clone())
            .with_outputs(state_keys)
            .with_payload(payload_bytes)
            .into_payload_builder()
            .map_err(|err| {
                CliError::ActionError(format!(
                    "Unable to convert execute action into sabre payload: {}",
                    err
                ))
            })?
            .into_transaction_builder()
            .map_err(|err| {
                CliError::ActionError(format!(
                    "Unable to convert execute payload into transaction: {}",
                    err
                ))
            })?
            .build(&*signer)
            .map_err(|err| {
                CliError::ActionError(format!("Unable to build transaction: {}", err))
            })?;

        let txn_proto = txn.into_proto().map_err(|err| {
            CliError::ActionError(format!(
                "Unable to convert transaction to protobuf: {}",
                err
            ))
        })?;

        let txn_id = txn_proto.get_header_signature().to_string();

        // create the batch header
        let mut batch_header = BatchHeader::new();
        batch_header.set_transaction_ids(protobuf::RepeatedField::from_vec(vec![txn_id]));
        batch_header.set_signer_public_key(
            signer
                .public_key()
                .map_err(|err| {
                    CliError::ActionError(format!("Unable to get signer public key: {}", err))
                })?
                .as_hex(),
        );

        let header_bytes = batch_header.write_to_bytes().map_err(|err| {
            CliError::ActionError(format!("Unable to convert header to bytes: {}", err))
        })?;

        // build the batch
        let mut batch = Batch::new();
        let signature = signer
            .sign(&header_bytes)
            .map_err(|err| CliError::ActionError(format!("Unable to sign batch: {}", err)))?
            .as_hex();
        batch.set_header_signature(signature);
        batch.set_header(header_bytes);
        batch.set_transactions(protobuf::RepeatedField::from_vec(vec![txn_proto]));

        let batch_pair = BatchPair::from_proto(batch).map_err(|err| {
            CliError::ActionError(format!(
                "Unable to convert to batch pair from batch proto: {}",
                err
            ))
        })?;

        let batch_bytes = match vec![batch_pair.batch().clone()].into_bytes() {
            Ok(bytes) => bytes,
            Err(err) => {
                return Err(CliError::ActionError(format!(
                    "Unable to convert batch to bytes: {}",
                    err
                )))
            }
        };

        // send batch to target
        Client::new()
            .post(&format!("{}/batches", target))
            .header(header::CONTENT_TYPE, "octet-stream")
            .header("Authorization", auth)
            .body(batch_bytes)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to submit get state transaction: {}", err))
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    Ok(())
                } else {
                    let message = res
                        .json::<ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "Get state request failed with status code '{}', but \
                                    error response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to submit get state transaction: {}",
                        message
                    )))
                }
            })
    }
}

fn parse_bytes_entry(bytes_entry: &str) -> Result<(String, Vec<u8>), CliError> {
    let mut parts = bytes_entry.splitn(2, ':');
    match (parts.next(), parts.next()) {
        (Some(key), Some(value)) => match key {
            "" => Err(CliError::ActionError(
                "Empty '--bytes-entry' argument detected".into(),
            )),
            _ => match value {
                "" => Err(CliError::ActionError(format!(
                    "Empty value detected for address: {}",
                    key
                ))),
                _ => Ok((key.to_string(), value.as_bytes().to_vec())),
            },
        },
        (Some(key), None) => Err(CliError::ActionError(format!(
            "Missing value for address '{}'",
            key
        ))),
        _ => unreachable!(), // splitn always returns at least one item
    }
}

#[derive(Deserialize)]
pub struct ServerError {
    pub message: String,
}
