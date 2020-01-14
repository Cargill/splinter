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
    Action, CreateSmartPermissionActionBuilder, DeleteSmartPermissionActionBuilder, SabrePayload,
    SabrePayloadBuilder, UpdateSmartPermissionActionBuilder,
};

use super::Error;

pub fn create_smart_permission_creation_payload(
    org_id: &str,
    name: &str,
    smart_permission_function: Vec<u8>,
) -> Result<SabrePayload, Error> {
    let action = CreateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(smart_permission_function)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build CreateSmartPermissionAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateSmartPermission(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_smart_permission_update_payload(
    org_id: &str,
    name: &str,
    smart_permission_function: Vec<u8>,
) -> Result<SabrePayload, Error> {
    let action = UpdateSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .with_function(smart_permission_function)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build UpdateSmartPermissionAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::UpdateSmartPermission(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_smart_permission_delete_payload(
    org_id: &str,
    name: &str,
) -> Result<SabrePayload, Error> {
    let action = DeleteSmartPermissionActionBuilder::new()
        .with_name(name.to_string())
        .with_org_id(org_id.to_string())
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build DeleteSmartPermissionAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteSmartPermission(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}
