// Copyright 2018-2020 Cargill Incorporated
// Copyright 2018 Intel Corporation
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

use std::fs::{metadata, File, OpenOptions};
use std::io::prelude::*;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(not(target_os = "linux"))]
use std::os::unix::fs::MetadataExt;

use clap::ArgMatches;
use cylinder::{secp256k1::Secp256k1Context, Context};

use crate::error::CliError;

use super::{chown, Action};

pub struct AdminKeyGenAction;

impl Action for AdminKeyGenAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_name = args.value_of("key_name").unwrap_or("splinter");
        let key_dir = args
            .value_of("key_dir")
            .or(Some("."))
            .map(Path::new)
            .unwrap();

        let private_key_path = key_dir.join(key_name).with_extension("priv");
        let public_key_path = key_dir.join(key_name).with_extension("pub");

        create_key_pair(
            &key_dir,
            private_key_path,
            public_key_path,
            args.is_present("force"),
            true,
        )?;

        Ok(())
    }
}

/// Creates a public/private key pair.
///
/// Returns the public key in hex, if successful.
fn create_key_pair(
    key_dir: &Path,
    private_key_path: PathBuf,
    public_key_path: PathBuf,
    force_create: bool,
    change_permissions: bool,
) -> Result<Vec<u8>, CliError> {
    if !force_create {
        if private_key_path.exists() {
            return Err(CliError::EnvironmentError(format!(
                "file exists: {:?}",
                private_key_path
            )));
        }
        if public_key_path.exists() {
            return Err(CliError::EnvironmentError(format!(
                "file exists: {:?}",
                public_key_path
            )));
        }
    }

    let context = Secp256k1Context::new();

    let private_key = context.new_random_private_key();
    let public_key = context
        .get_public_key(&private_key)
        .map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;

    let key_dir_info =
        metadata(key_dir).map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;

    #[cfg(not(target_os = "linux"))]
    let (key_dir_uid, key_dir_gid) = (key_dir_info.uid(), key_dir_info.gid());
    #[cfg(target_os = "linux")]
    let (key_dir_uid, key_dir_gid) = (key_dir_info.st_uid(), key_dir_info.st_gid());

    {
        if private_key_path.exists() {
            info!("overwriting file: {:?}", private_key_path);
        } else {
            info!("writing file: {:?}", private_key_path);
        }

        let mut private_key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o640)
            .open(private_key_path.as_path())
            .map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;
        write_hex_to_file(&private_key.as_hex(), &mut private_key_file)?;
    }

    {
        if public_key_path.exists() {
            info!("overwriting file: {:?}", public_key_path);
        } else {
            info!("writing file: {:?}", public_key_path);
        }

        let mut public_key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o644)
            .open(public_key_path.as_path())
            .map_err(|err| CliError::EnvironmentError(format!("{}", err)))?;
        write_hex_to_file(&public_key.as_hex(), &mut public_key_file)?;
    }
    if change_permissions {
        chown(private_key_path.as_path(), key_dir_uid, key_dir_gid)?;
        chown(public_key_path.as_path(), key_dir_uid, key_dir_gid)?;
    }

    Ok(public_key.into_bytes())
}

/// Write the given hex string to the given file, appending a newline at the end.
fn write_hex_to_file(hex: &str, file: &mut File) -> Result<(), CliError> {
    writeln!(file, "{}", hex).map_err(|err| CliError::EnvironmentError(format!("{}", err)))
}
