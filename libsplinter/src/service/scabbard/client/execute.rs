// Copyright 2020 Cargill Incorporated
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

use sabre_sdk::protocol::payload::{
    Action, ExecuteContractActionBuilder, SabrePayload, SabrePayloadBuilder,
};

use super::Error;

pub fn create_contract_execution_payload(
    name: &str,
    version: &str,
    inputs: Vec<String>,
    outputs: Vec<String>,
    contract_payload: Vec<u8>,
) -> Result<SabrePayload, Error> {
    let exec_contract = ExecuteContractActionBuilder::new()
        .with_name(name.into())
        .with_version(version.into())
        .with_inputs(inputs)
        .with_outputs(outputs)
        .with_payload(contract_payload)
        .build()
        .map_err(|err| Error(format!("failed to build ExecuteContractAction: {}", err)))?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::ExecuteContract(exec_contract))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}
