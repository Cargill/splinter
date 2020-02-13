// Copyright 2018 Cargill Incorporated
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
    CreateNamespaceRegistryActionBuilder, CreateNamespaceRegistryPermissionActionBuilder,
    DeleteNamespaceRegistryActionBuilder, DeleteNamespaceRegistryPermissionActionBuilder,
    UpdateNamespaceRegistryOwnersActionBuilder,
};

use crate::error::CliError;
use crate::key::new_signer;
use crate::submit::submit_batches;

pub fn do_ns_create(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let signer = new_signer(key_name)?;
    let batch = CreateNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}

pub fn do_ns_update(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    owners: Vec<String>,
) -> Result<String, CliError> {
    let signer = new_signer(key_name)?;
    let batch = UpdateNamespaceRegistryOwnersActionBuilder::new()
        .with_namespace(namespace.into())
        .with_owners(owners)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}

pub fn do_ns_delete(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
) -> Result<String, CliError> {
    let signer = new_signer(key_name)?;
    let batch = DeleteNamespaceRegistryActionBuilder::new()
        .with_namespace(namespace.into())
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}

pub fn do_perm_create(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
    contract: &str,
    read: bool,
    write: bool,
) -> Result<String, CliError> {
    let signer = new_signer(key_name)?;
    let batch = CreateNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .with_contract_name(contract.into())
        .with_read(read)
        .with_write(write)
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}
pub fn do_perm_delete(
    key_name: Option<&str>,
    url: &str,
    namespace: &str,
) -> Result<String, CliError> {
    let signer = new_signer(key_name)?;
    let batch = DeleteNamespaceRegistryPermissionActionBuilder::new()
        .with_namespace(namespace.into())
        .into_payload_builder()?
        .into_transaction_builder(&signer)?
        .into_batch_builder(&signer)?
        .build(&signer)?;

    submit_batches(url, vec![batch])
}
