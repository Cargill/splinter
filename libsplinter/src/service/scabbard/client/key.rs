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

use std::fs::File;
use std::io::{BufRead, BufReader};

use sawtooth_sdk::signing::secp256k1::Secp256k1PrivateKey;

use super::Error;

/// Load a private key from the local filesystem.
pub fn load_signing_key_from_file(filename: &str) -> Result<Secp256k1PrivateKey, Error> {
    let key_file = File::open(filename)
        .map_err(|err| Error(format!("failed to open private key file: {}", err)))?;
    let mut key_file_reader = BufReader::new(key_file);
    let mut raw_key_string = String::new();
    key_file_reader
        .read_line(&mut raw_key_string)
        .map_err(|err| Error(format!("failed to read private key file: {}", err)))?;
    let key_string = raw_key_string.trim();
    if key_string.is_empty() {
        return Err(Error("private key file is empty".into()));
    }
    let signing_key = Secp256k1PrivateKey::from_hex(key_string).map_err(|err| {
        Error(format!(
            "failed to read valid private key from file: {}",
            err
        ))
    })?;
    Ok(signing_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::Path;

    use sawtooth_sdk::signing::PrivateKey;

    use crate::service::scabbard::client::tests::{new_temp_dir, MOCK_PRIV_KEY_HEX};

    const MOCK_KEY_FILENAME: &str = "mock.priv";

    #[test]
    fn file_not_found() {
        assert!(load_signing_key_from_file("non_existent_file.priv").is_err());
    }

    #[test]
    fn file_empty() {
        let temp_dir = new_temp_dir();
        let filename = add_mock_key_to_dir(temp_dir.path(), "");
        assert!(load_signing_key_from_file(&filename).is_err());
    }

    #[test]
    fn key_in_file_invalid() {
        let temp_dir = new_temp_dir();
        let filename = add_mock_key_to_dir(temp_dir.path(), "not_valid_hex");
        assert!(load_signing_key_from_file(&filename).is_err());
    }

    #[test]
    fn successful() {
        let temp_dir = new_temp_dir();
        let filename = add_mock_key_to_dir(temp_dir.path(), MOCK_PRIV_KEY_HEX);
        let signing_key =
            load_signing_key_from_file(&filename).expect("failed to get key from file");
        assert_eq!(&signing_key.as_hex(), MOCK_PRIV_KEY_HEX);
    }

    fn add_mock_key_to_dir(dir: &Path, key: &str) -> String {
        let key_file_path = dir.join(MOCK_KEY_FILENAME);
        let mut key_file =
            File::create(key_file_path.as_path()).expect("failed to create key file");
        write!(&mut key_file, "{}", key).expect("failed to write key file");

        key_file_path.to_string_lossy().into_owned()
    }
}
