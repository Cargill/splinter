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
    Action, CreateContractRegistryActionBuilder, DeleteContractRegistryActionBuilder, SabrePayload,
    SabrePayloadBuilder, UpdateContractRegistryOwnersActionBuilder,
};

use super::Error;

pub fn create_contract_registry_creation_payload(
    name: &str,
    owners: Vec<String>,
) -> Result<SabrePayload, Error> {
    let action = CreateContractRegistryActionBuilder::new()
        .with_name(name.into())
        .with_owners(owners)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build CreateContractRegistryAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateContractRegistry(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_contract_registry_update_payload(
    name: &str,
    owners: Vec<String>,
) -> Result<SabrePayload, Error> {
    let action = UpdateContractRegistryOwnersActionBuilder::new()
        .with_name(name.into())
        .with_owners(owners)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build UpdateContractRegistryOwnersAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::UpdateContractRegistryOwners(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_contract_registry_delete_payload(name: &str) -> Result<SabrePayload, Error> {
    let action = DeleteContractRegistryActionBuilder::new()
        .with_name(name.into())
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build DeleteContractRegistryAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteContractRegistry(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}
