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
    Action, CreateNamespaceRegistryActionBuilder, CreateNamespaceRegistryPermissionActionBuilder,
    DeleteNamespaceRegistryActionBuilder, DeleteNamespaceRegistryPermissionActionBuilder,
    SabrePayload, SabrePayloadBuilder, UpdateNamespaceRegistryOwnersActionBuilder,
};

use super::Error;

pub fn create_namespace_creation_payload(
    namespace: &str,
    owners: Vec<String>,
) -> Result<SabrePayload, Error> {
    let action = CreateNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build CreateNamespaceRegistryAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateNamespaceRegistry(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_namespace_update_payload(
    namespace: &str,
    owners: Vec<String>,
) -> Result<SabrePayload, Error> {
    let action = UpdateNamespaceRegistryOwnersActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build UpdateNamespaceRegistryOwnersAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::UpdateNamespaceRegistryOwners(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_namespace_delete_payload(namespace: &str) -> Result<SabrePayload, Error> {
    let action = DeleteNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build DeleteNamespaceRegistryAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteNamespaceRegistry(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_namespace_permission_creation_payload(
    namespace: &str,
    contract: &str,
    read: bool,
    write: bool,
) -> Result<SabrePayload, Error> {
    let action = CreateNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .with_contract_name(contract.into())
        .with_read(read)
        .with_write(write)
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build CreateNamespaceRegistryPermissionAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::CreateNamespaceRegistryPermission(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}

pub fn create_namespace_permission_deletion_payload(
    namespace: &str,
) -> Result<SabrePayload, Error> {
    let action = DeleteNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .build()
        .map_err(|err| {
            Error(format!(
                "failed to build DeleteNamespaceRegistryPermissionAction: {}",
                err
            ))
        })?;

    let payload = SabrePayloadBuilder::new()
        .with_action(Action::DeleteNamespaceRegistryPermission(action))
        .build()
        .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

    Ok(payload)
}
