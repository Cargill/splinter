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

mod contract_registry;
mod error;
mod execute;
mod key;
mod namespace;
mod smart_permission;
mod submit;
mod transaction;
mod upload;

use std::fs::File;
use std::io::{BufReader, Read};

use sabre_sdk::protocol::payload::SabrePayload;
pub use sawtooth_sdk::signing::secp256k1::Secp256k1PrivateKey;

pub use error::Error;
use submit::submit_batch_list;
use transaction::batch_payloads;
pub use upload::{SmartContract, SmartContractMetadata};

/// A client that can be used to submit transactions to a scabbard service.
pub struct ScabbardClient {
    url: String,
    signing_key: Secp256k1PrivateKey,
    unsent_payloads: Vec<SabrePayload>,
}

impl ScabbardClient {
    /// Create a new `ScabbardClient` with the given `url` and a signing key loaded from the given
    /// `key_file`.
    pub fn new_with_local_signing_key(url: &str, key_file: &str) -> Result<Self, Error> {
        Ok(Self::new(url, Self::load_signing_key_from_file(key_file)?))
    }

    /// Create a new `ScabbardClient` with the given `url` and `signing_key`.
    pub fn new(url: &str, signing_key: Secp256k1PrivateKey) -> Self {
        Self {
            url: url.into(),
            signing_key,
            unsent_payloads: vec![],
        }
    }

    /// Submit all unsent transactions to the scabbard service.
    pub fn submit(mut self) -> Result<(), Error> {
        let batch_list = batch_payloads(&mut self.unsent_payloads.drain(..), &self.signing_key)?;
        let _batch_link = submit_batch_list(&self.url, &batch_list)?;
        Ok(())
    }

    /// Queue a transaction to upload a smart contract loaded from the given `.scar`. See
    /// `load_scar` for more on how the `.scar` is loaded.
    pub fn upload_contract_from_scar(self, scar: &str) -> Result<Self, Error> {
        self.upload_contract(SmartContract::new_from_scar(scar)?)
    }

    /// Queue a transaction to upload the given smart contract.
    pub fn upload_contract(mut self, sc: SmartContract) -> Result<Self, Error> {
        self.unsent_payloads.push(sc.try_into_sabre_payload()?);
        Ok(self)
    }

    /// Queue a transaction to execute a smart contract, loading the payload from the specified
    /// `payload_file`.
    pub fn execute_contract_from_file(
        self,
        name: &str,
        version: &str,
        inputs: Vec<String>,
        outputs: Vec<String>,
        payload_file: &str,
    ) -> Result<Self, Error> {
        self.execute_contract(
            name,
            version,
            inputs,
            outputs,
            load_file_into_bytes(payload_file)?,
        )
    }

    /// Queue a transaction to execute a smart contract with the specified `contract_payload`.
    pub fn execute_contract(
        mut self,
        name: &str,
        version: &str,
        inputs: Vec<String>,
        outputs: Vec<String>,
        contract_payload: Vec<u8>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(execute::create_contract_execution_payload(
                name,
                version,
                inputs,
                outputs,
                contract_payload,
            )?);
        Ok(self)
    }

    /// Queue a transaction to create a namespace registry.
    pub fn create_namespace_registry(
        mut self,
        namespace: &str,
        owners: Vec<String>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(namespace::create_namespace_creation_payload(
                namespace, owners,
            )?);
        Ok(self)
    }

    /// Queue a transaction to update a namespace registry.
    pub fn update_namespace_registry(
        mut self,
        namespace: &str,
        owners: Vec<String>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(namespace::create_namespace_update_payload(
                namespace, owners,
            )?);
        Ok(self)
    }

    /// Queue a transaction to delete a namespace registry.
    pub fn delete_namespace_registry(mut self, namespace: &str) -> Result<Self, Error> {
        self.unsent_payloads
            .push(namespace::create_namespace_delete_payload(namespace)?);
        Ok(self)
    }

    /// Queue a transaction to create a namespace registry permission.
    pub fn create_namespace_registry_permission(
        mut self,
        namespace: &str,
        contract: &str,
        read: bool,
        write: bool,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(namespace::create_namespace_permission_creation_payload(
                namespace, contract, read, write,
            )?);
        Ok(self)
    }

    /// Queue a transaction to delete a namespace registry permission.
    pub fn delete_namespace_registry_permission(mut self, namespace: &str) -> Result<Self, Error> {
        self.unsent_payloads
            .push(namespace::create_namespace_permission_deletion_payload(
                namespace,
            )?);
        Ok(self)
    }

    /// Queue a transaction to create a contract registry.
    pub fn create_contract_registry(
        mut self,
        name: &str,
        owners: Vec<String>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(contract_registry::create_contract_registry_creation_payload(name, owners)?);
        Ok(self)
    }

    /// Queue a transaction to update a contract registry.
    pub fn update_contract_registry(
        mut self,
        name: &str,
        owners: Vec<String>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(contract_registry::create_contract_registry_update_payload(
                name, owners,
            )?);
        Ok(self)
    }

    /// Queue a transaction to delete a contract registry.
    pub fn delete_contract_registry(mut self, name: &str) -> Result<Self, Error> {
        self.unsent_payloads
            .push(contract_registry::create_contract_registry_delete_payload(
                name,
            )?);
        Ok(self)
    }

    /// Queue a transaction to create a smart permssion, loaded from the specified
    /// `smart_permission_file`.
    pub fn create_smart_permission_from_file(
        self,
        org_id: &str,
        name: &str,
        smart_permission_file: &str,
    ) -> Result<Self, Error> {
        self.create_smart_permission(org_id, name, load_file_into_bytes(smart_permission_file)?)
    }

    /// Queue a transaction to create a smart permssion, with the specified
    /// `smart_permission_function`.
    pub fn create_smart_permission(
        mut self,
        org_id: &str,
        name: &str,
        smart_permission_function: Vec<u8>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(smart_permission::create_smart_permission_creation_payload(
                org_id,
                name,
                smart_permission_function,
            )?);
        Ok(self)
    }

    /// Queue a transaction to update a smart permssion, loaded from the specified
    /// `smart_permission_file`.
    pub fn update_smart_permission_from_file(
        self,
        org_id: &str,
        name: &str,
        smart_permission_file: &str,
    ) -> Result<Self, Error> {
        self.update_smart_permission(org_id, name, load_file_into_bytes(smart_permission_file)?)
    }

    /// Queue a transaction to update a smart permssion, with the specified
    /// `smart_permission_function`.
    pub fn update_smart_permission(
        mut self,
        org_id: &str,
        name: &str,
        smart_permission_function: Vec<u8>,
    ) -> Result<Self, Error> {
        self.unsent_payloads
            .push(smart_permission::create_smart_permission_update_payload(
                org_id,
                name,
                smart_permission_function,
            )?);
        Ok(self)
    }

    /// Queue a transaction to delete a smart permssion.
    pub fn delete_smart_permission(mut self, org_id: &str, name: &str) -> Result<Self, Error> {
        self.unsent_payloads
            .push(smart_permission::create_smart_permission_delete_payload(
                org_id, name,
            )?);
        Ok(self)
    }

    /// Load the signing key in the given key file.
    pub fn load_signing_key_from_file(filename: &str) -> Result<Secp256k1PrivateKey, Error> {
        key::load_signing_key_from_file(filename)
    }
}

/// Load the contents of a file into a bytes vector.
fn load_file_into_bytes(payload_file: &str) -> Result<Vec<u8>, Error> {
    let file =
        File::open(payload_file).map_err(|err| Error(format!("failed to load file: {}", err)))?;
    let mut buf_reader = BufReader::new(file);
    let mut contents = Vec::new();
    buf_reader
        .read_to_end(&mut contents)
        .map_err(|err| Error(format!("failed to read file: {}", err)))?;
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;

    pub const MOCK_PRIV_KEY_HEX: &str =
        "d31e395bed0d9b2277b25d57523063d7d6b9db802d80549bc1362875cdcb83c6";

    pub fn new_temp_dir() -> TempDir {
        let thread_id = format!("{:?}", std::thread::current().id());
        TempDir::new(&thread_id).expect("failed to create temp dir")
    }
}
