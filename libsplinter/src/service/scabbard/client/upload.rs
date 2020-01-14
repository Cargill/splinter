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

use std::env::{split_paths, var_os};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use bzip2::read::BzDecoder;
use sabre_sdk::protocol::payload::{
    Action, CreateContractActionBuilder, SabrePayload, SabrePayloadBuilder,
};
use tar::Archive;

use super::Error;

const SCAR_FILE_EXTENSION: &str = "scar";
const SCAR_PATH_ENV_VAR: &str = "SCAR_PATH";
const MANIFEST_FILENAME: &str = "manifest.yaml";
const WASM_FILE_EXTENSION: &str = "wasm";

/// The definition of a WASM smart contract, including the bytes of the smart contract itself and
/// the associated metadata that is required for submitting the smart contract to scabbard.
#[derive(Debug)]
pub struct SmartContract {
    contract: Vec<u8>,
    metadata: SmartContractMetadata,
}

impl SmartContract {
    /// Load a `SmartContract` from a .scar file on the local filesystem.
    ///
    /// If the argument is a file path (contains a '/'), this will attempt to load the .scar from
    /// the specified location. If the argument is not a file path, this will attempt to load the
    /// .scar from the directories listed in the SCAR_PATH environment variable. When loading from
    /// a directory in SCAR_PATH, the '.scar' file extension is optional.
    pub fn new_from_scar(scar: &str) -> Result<SmartContract, Error> {
        let scar_file_path = determine_scar_file_path(scar)?;
        load_smart_contract_from_file(&scar_file_path)
    }

    /// Attempt to convert the smart contract definition into a `SabrePayload`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Sabre `Action` or `SabrePayload` builders fail.
    pub fn try_into_sabre_payload(self) -> Result<SabrePayload, Error> {
        let create_contract = CreateContractActionBuilder::new()
            .with_name(self.metadata.name)
            .with_version(self.metadata.version)
            .with_inputs(self.metadata.inputs)
            .with_outputs(self.metadata.outputs)
            .with_contract(self.contract)
            .build()
            .map_err(|err| Error(format!("failed to build CreateContractAction: {}", err)))?;

        let payload = SabrePayloadBuilder::new()
            .with_action(Action::CreateContract(create_contract))
            .build()
            .map_err(|err| Error(format!("failed to build SabrePayload: {}", err)))?;

        Ok(payload)
    }
}

/// The metadata of a smart contract that needs to be included in the Sabre transaction.
#[derive(Debug, Deserialize, Serialize)]
pub struct SmartContractMetadata {
    name: String,
    version: String,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

fn determine_scar_file_path(scar: &str) -> Result<PathBuf, Error> {
    if arg_is_file_path(scar) {
        Ok(PathBuf::from(scar))
    } else {
        let scar_paths = var_os(SCAR_PATH_ENV_VAR).ok_or_else(|| {
            Error(format!(
                "cannot find .scar file: {} not set",
                SCAR_PATH_ENV_VAR
            ))
        })?;
        split_paths(&scar_paths)
            .find_map(|mut path| {
                path.push(scar);
                if path.exists() {
                    Some(path)
                } else {
                    path.set_extension(SCAR_FILE_EXTENSION);
                    if path.exists() {
                        Some(path)
                    } else {
                        None
                    }
                }
            })
            .ok_or_else(|| Error(format!("{} not found in {}", scar, SCAR_PATH_ENV_VAR)))
    }
}

fn arg_is_file_path(arg: &str) -> bool {
    arg.contains('/')
}

fn load_smart_contract_from_file(file_path: &Path) -> Result<SmartContract, Error> {
    let scar_file = File::open(file_path).map_err(|err| {
        Error(format!(
            "failed to open file {}: {}",
            file_path.display(),
            err
        ))
    })?;
    let mut archive = Archive::new(BzDecoder::new(scar_file));
    let archive_entries = archive
        .entries()
        .map_err(|err| Error(format!("failed to read .scar file: {}", err)))?;

    let mut metadata = None;
    let mut contract = None;

    for entry in archive_entries {
        let mut entry = entry.map_err(|err| {
            Error(format!(
                "invalid .scar: failed to read archive entry: {}",
                err
            ))
        })?;
        let path = entry
            .path()
            .map_err(|err| {
                Error(format!(
                    "invalid .scar: failed to get path of archive entry: {}",
                    err
                ))
            })?
            .into_owned();
        if path_is_manifest(&path) {
            metadata =
                Some(serde_yaml::from_reader(entry).map_err(|err| {
                    Error(format!("invalid .scar: manifest.yaml invalid: {}", err))
                })?);
        } else if path_is_wasm(&path) {
            let mut contract_bytes = vec![];
            entry.read_to_end(&mut contract_bytes).map_err(|err| {
                Error(format!(
                    "invalid .scar: failed to read smart contract: {}",
                    err
                ))
            })?;
            contract = Some(contract_bytes);
        }
    }

    Ok(SmartContract {
        metadata: metadata.ok_or_else(|| Error("invalid .scar: manifest.yaml not found".into()))?,
        contract: contract
            .ok_or_else(|| Error("invalid .scar: smart contract not found".into()))?,
    })
}

fn path_is_manifest(path: &std::path::Path) -> bool {
    path.file_name()
        .map(|file_name| file_name == MANIFEST_FILENAME)
        .unwrap_or(false)
}

fn path_is_wasm(path: &std::path::Path) -> bool {
    match path.extension() {
        Some(extension) => extension == WASM_FILE_EXTENSION,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::Path;

    use bzip2::write::BzEncoder;
    use bzip2::Compression;
    use serde::Serialize;
    use serial_test::serial;
    use tar::Builder;
    use tempdir::TempDir;

    use crate::service::scabbard::client::tests::new_temp_dir;

    const MOCK_CONTRACT_BYTES: &[u8] = &[0x00, 0x01, 0x02, 0x03];
    const MOCK_CONTRACT_FILENAME: &str = "mock.wasm";
    const MOCK_SCAR_FILENAME: &str = "mock.scar";

    // The tests in this module must run serially because some tests modify environment variable(s)
    // that are used by all tests. Each test is annotated with `#[serial(scar_path)]` to enforce
    // this.

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_from_path_with_file_extension_successful() {
        let setup = UploadTestSetup::new().build();
        SmartContract::new_from_scar(&setup.scar).expect("failed to perform upload action");
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_from_path_without_file_extension_successful() {
        let setup = UploadTestSetup::new().with_scar_without_extension().build();
        SmartContract::new_from_scar(&setup.scar).expect("failed to perform upload action");
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_from_file_successful() {
        let setup = UploadTestSetup::new().with_scar_from_file().build();
        SmartContract::new_from_scar(&setup.scar).expect("failed to perform upload action");
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_file_not_found() {
        let setup = UploadTestSetup::new()
            .with_scar("/non_existent_dir/mock.scar".into())
            .build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_path_not_set() {
        let setup = UploadTestSetup::new().with_scar_path_env_var(None).build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_not_found_in_path() {
        let setup = UploadTestSetup::new()
            .with_scar_path_env_var(Some("".into()))
            .build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_manifest_not_found() {
        let setup = UploadTestSetup::new()
            .with_manifest::<SmartContractMetadata>(None)
            .build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_manifest_invalid() {
        let setup = UploadTestSetup::new().with_manifest(Some("")).build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn load_smart_contract_contract_not_found() {
        let setup = UploadTestSetup::new().set_contract(false).build();
        assert!(SmartContract::new_from_scar(&setup.scar).is_err());
    }

    #[test]
    #[serial(scar_path)]
    fn smart_contract_into_sabre_payload() {
        let sc = get_mock_smart_contract();
        sc.try_into_sabre_payload()
            .expect("failed to convert smart contract into sabre payload");
    }

    struct UploadTestSetup {
        temp_dir: TempDir,
        set_contract: bool,
        manifest: Option<Vec<u8>>,
        scar_path_env_var: Option<String>,
        scar: String,
    }

    impl UploadTestSetup {
        fn new() -> Self {
            let temp_dir = new_temp_dir();
            let scar_path_env_var = temp_dir.path().to_string_lossy().into_owned();
            let scar = MOCK_SCAR_FILENAME.into();
            Self {
                temp_dir,
                set_contract: true,
                manifest: Some(
                    serde_yaml::to_vec(&get_mock_smart_contract_metadata())
                        .expect("failed to serialize manifest"),
                ),
                scar_path_env_var: Some(scar_path_env_var),
                scar,
            }
        }

        fn set_contract(mut self, set_contract: bool) -> Self {
            self.set_contract = set_contract;
            self
        }

        fn with_manifest<T: Serialize>(mut self, manifest: Option<T>) -> Self {
            self.manifest = manifest.map(|manifest| {
                serde_yaml::to_vec(&manifest).expect("failed to serialize manifest")
            });
            self
        }

        fn with_scar_path_env_var(mut self, scar_path_env_var: Option<String>) -> Self {
            self.scar_path_env_var = scar_path_env_var;
            self
        }

        fn with_scar_from_file(mut self) -> Self {
            self.scar = self
                .temp_dir
                .path()
                .join(MOCK_SCAR_FILENAME)
                .to_string_lossy()
                .into_owned();
            self
        }

        fn with_scar_without_extension(mut self) -> Self {
            self.scar = MOCK_SCAR_FILENAME
                .split(".")
                .next()
                .expect("failed to get stem from mock .scar filename")
                .into();
            self
        }

        fn with_scar(mut self, scar: String) -> Self {
            self.scar = scar;
            self
        }

        fn build(self) -> SetupHandle {
            match self.scar_path_env_var {
                Some(scar_path_env_var) => std::env::set_var(SCAR_PATH_ENV_VAR, scar_path_env_var),
                None => std::env::remove_var(SCAR_PATH_ENV_VAR),
            }

            add_mock_scar_to_dir(self.temp_dir.path(), self.manifest, self.set_contract);

            SetupHandle {
                _temp_dir: self.temp_dir,
                scar: self.scar,
            }
        }
    }

    struct SetupHandle {
        _temp_dir: TempDir,
        scar: String,
    }

    fn add_mock_scar_to_dir(dir: &Path, manifest: Option<Vec<u8>>, add_contract: bool) {
        let scar_file_path = dir.join(MOCK_SCAR_FILENAME);
        let scar = File::create(scar_file_path.as_path()).expect("failed to create .scar");
        let mut scar_builder = Builder::new(BzEncoder::new(scar, Compression::Default));

        if let Some(manifest) = manifest {
            let manifest_file_path = dir.join(MANIFEST_FILENAME);
            let mut manifest_file =
                File::create(manifest_file_path.as_path()).expect("failed to create manifest file");
            manifest_file
                .write_all(manifest.as_slice())
                .expect("failed to write manifest file");
            scar_builder
                .append_path_with_name(manifest_file_path, MANIFEST_FILENAME)
                .expect("failed to add manifest to .scar");
        }

        if add_contract {
            let contract_file_path = dir.join(MOCK_CONTRACT_FILENAME);
            let mut contract_file =
                File::create(contract_file_path.as_path()).expect("failed to create contract file");
            contract_file
                .write_all(MOCK_CONTRACT_BYTES)
                .expect("failed to write contract file");
            scar_builder
                .append_path_with_name(contract_file_path, MOCK_CONTRACT_FILENAME)
                .expect("failed to add contract to .scar");
        }

        scar_builder.finish().expect("failed to write .scar");
    }

    fn get_mock_smart_contract() -> SmartContract {
        SmartContract {
            contract: MOCK_CONTRACT_BYTES.to_vec(),
            metadata: get_mock_smart_contract_metadata(),
        }
    }

    fn get_mock_smart_contract_metadata() -> SmartContractMetadata {
        SmartContractMetadata {
            name: "mock".into(),
            version: "1.0".into(),
            inputs: vec!["abcdef".into()],
            outputs: vec!["012345".into()],
        }
    }
}
