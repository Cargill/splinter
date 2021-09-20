/*
 * Copyright 2021 Cargill Incorporated
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 * -----------------------------------------------------------------------------
 */

//! A Sabre compatible Command family smart contract

#[macro_use]
extern crate sabre_sdk;

use protobuf::Message;

use sabre_sdk::ApplyError as SabreApplyError;
use sabre_sdk::TpProcessRequest as SabreTpProcessRequest;
use sabre_sdk::TransactionContext as SabreTransactionContext;
use sabre_sdk::{execute_entrypoint, WasmPtr};
use transact::families::command::CommandTransactionHandler;
use transact::handler::sabre::SabreContext;
use transact::handler::{ApplyError, TransactionHandler};
use transact::protocol::transaction::Transaction;
use transact::protos::transaction::TransactionHeader;

fn main() {}

// Sabre apply must return a bool
fn apply(
    request: &SabreTpProcessRequest,
    context: &mut dyn SabreTransactionContext,
) -> Result<bool, SabreApplyError> {
    // convert SabreTpProcessRequest into TransactionPair
    let commands = request.get_payload().to_vec();

    let mut header = TransactionHeader::new();
    header.set_signer_public_key(request.get_header().get_signer_public_key().to_string());

    let header_bytes = header.write_to_bytes().map_err(|_| {
        SabreApplyError::InvalidTransaction("Unable to convert header to bytes".to_string())
    })?;

    let txn = Transaction::new(header_bytes, request.get_signature(), commands);

    let txn_pair = txn
        .into_pair()
        .map_err(|err| SabreApplyError::InvalidTransaction(err.to_string()))?;

    // wrap SabreTransactionContext into SabreContext that can be passed to a transact
    // TransactionHandler
    let mut context = SabreContext { context };

    let handler = CommandTransactionHandler::new();

    match handler.apply(&txn_pair, &mut context) {
        Ok(_) => Ok(true),
        Err(err) => {
            info!("{}", err);

            match err {
                ApplyError::InvalidTransaction(msg) => {
                    Err(SabreApplyError::InvalidTransaction(msg))
                }
                ApplyError::InternalError(msg) => Err(SabreApplyError::InternalError(msg)),
            }
        }
    }
}

/// # Safety
///
/// This function is required to be able to execute the wasm smart contract
#[no_mangle]
pub unsafe fn entrypoint(payload: WasmPtr, signer: WasmPtr, signature: WasmPtr) -> i32 {
    execute_entrypoint(payload, signer, signature, apply)
}
