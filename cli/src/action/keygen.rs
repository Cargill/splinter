// Copyright 2018-2021 Cargill Incorporated
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

use std::env;
use std::fs::{create_dir_all, metadata, OpenOptions};
use std::io::prelude::*;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(not(target_os = "linux"))]
use std::os::unix::fs::MetadataExt;

use clap::ArgMatches;
use cylinder::{secp256k1::Secp256k1Context, Context};
use cylinder::{PrivateKey, PublicKey};

use crate::error::CliError;

use super::{chown, Action};

const SYSTEM_KEY_PATH: &str = "/etc/splinter/keys";
const SPLINTER_HOME_ENV: &str = "SPLINTER_HOME";
const CONFIG_DIR_ENV: &str = "SPLINTER_CONFIG_DIR";
const DEFAULT_SYSTEM_KEY_NAME: &str = "splinterd";

pub struct KeyGenAction;

impl Action for KeyGenAction {
    fn run<'a>(&mut self, arg_matches: Option<&ArgMatches<'a>>) -> Result<(), CliError> {
        let args = arg_matches.ok_or(CliError::RequiresArgs)?;

        let key_name = args
            .value_of("key-name")
            .map(String::from)
            .unwrap_or_else(|| {
                if args.is_present("system") {
                    DEFAULT_SYSTEM_KEY_NAME.to_string()
                } else {
                    whoami::username()
                }
            });

        let key_dir = if let Some(dir) = args.value_of("key_dir") {
            PathBuf::from(dir)
        } else if args.is_present("system") {
            if let Ok(config_dir) = env::var(CONFIG_DIR_ENV) {
                let opt_path = Path::new(&config_dir).join("keys");
                if !opt_path.is_dir() {
                    create_dir_all(&opt_path).map_err(|_| {
                        CliError::ActionError(format!(
                            "Unable to create directory: {}",
                            opt_path.display()
                        ))
                    })?;
                }
                opt_path
            } else if let Ok(splinter_home) = env::var(SPLINTER_HOME_ENV) {
                let opt_path = Path::new(&splinter_home).join("etc").join("keys");
                if !opt_path.is_dir() {
                    create_dir_all(&opt_path).map_err(|_| {
                        CliError::ActionError(format!(
                            "Unable to create directory: {}",
                            opt_path.display()
                        ))
                    })?;
                }
                opt_path
            } else {
                PathBuf::from(SYSTEM_KEY_PATH)
            }
        } else {
            dirs::home_dir()
                .map(|mut p| {
                    p.push(".cylinder/keys");
                    p
                })
                .ok_or_else(|| CliError::EnvironmentError("Home directory not found".into()))?
        };

        create_dir_all(key_dir.as_path()).map_err(|err| {
            CliError::EnvironmentError(format!("Failed to create keys directory: {}", err))
        })?;

        let private_key_path = key_dir.join(&key_name).with_extension("priv");
        let public_key_path = key_dir.join(&key_name).with_extension("pub");

        write_keys(
            create_key_pair()?,
            &key_dir,
            private_key_path,
            public_key_path,
            args.is_present("force"),
            args.is_present("skip"),
            true,
        )?;

        Ok(())
    }
}

fn write_keys(
    keys: (PrivateKey, PublicKey),
    key_dir: &Path,
    private_key_path: PathBuf,
    public_key_path: PathBuf,
    force_create: bool,
    skip_create: bool,
    change_permissions: bool,
) -> Result<(), CliError> {
    let (private_key, public_key) = keys;
    if !force_create {
        match (private_key_path.exists(), public_key_path.exists()) {
            (true, true) => {
                if skip_create {
                    info!(
                        "Skipping, key already exists: {}",
                        private_key_path.display()
                    );
                    return Ok(());
                } else {
                    return Err(CliError::EnvironmentError(format!(
                        "Files already exists: private_key: {:?}, public_key: {:?}",
                        private_key_path, public_key_path
                    )));
                }
            }
            (true, false) => {
                if skip_create {
                    return Err(CliError::EnvironmentError(format!(
                        "Cannot skip, private key exists but not the public key: {:?}",
                        private_key_path
                    )));
                } else {
                    return Err(CliError::EnvironmentError(format!(
                        "File already exists: {:?}",
                        private_key_path
                    )));
                }
            }
            (false, true) => {
                if skip_create {
                    return Err(CliError::EnvironmentError(format!(
                        "Cannot skip, public key exists but not the private key: {:?}",
                        public_key_path
                    )));
                } else {
                    return Err(CliError::EnvironmentError(format!(
                        "File already exists: {:?}",
                        public_key_path
                    )));
                }
            }
            (false, false) => (),
        }
    }
    let key_dir_info = metadata(key_dir).map_err(|err| {
        CliError::EnvironmentError(format!(
            "Failed to read key directory '{}': {}",
            key_dir.display(),
            err
        ))
    })?;

    #[cfg(not(target_os = "linux"))]
    let (key_dir_uid, key_dir_gid) = (key_dir_info.uid(), key_dir_info.gid());
    #[cfg(target_os = "linux")]
    let (key_dir_uid, key_dir_gid) = (key_dir_info.st_uid(), key_dir_info.st_gid());

    {
        if private_key_path.exists() {
            info!(
                "Overwriting private key file: {}",
                private_key_path.display()
            );
        } else {
            info!("Writing private key file: {}", private_key_path.display());
        }

        let private_key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o640)
            .open(private_key_path.as_path())
            .map_err(|err| {
                CliError::EnvironmentError(format!(
                    "Failed to open private key file '{}': {}",
                    private_key_path.display(),
                    err
                ))
            })?;

        writeln!(&private_key_file, "{}", private_key.as_hex()).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to write to private key file '{}': {}",
                private_key_path.display(),
                err
            ))
        })?;
    }

    {
        if public_key_path.exists() {
            info!("Overwriting public key file: {}", public_key_path.display());
        } else {
            info!("Writing public key file: {}", public_key_path.display());
        }

        let public_key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .mode(0o644)
            .open(public_key_path.as_path())
            .map_err(|err| {
                CliError::EnvironmentError(format!(
                    "Failed to open public key file '{}': {}",
                    public_key_path.display(),
                    err
                ))
            })?;

        writeln!(&public_key_file, "{}", public_key.as_hex()).map_err(|err| {
            CliError::ActionError(format!(
                "Failed to write to public key file '{}': {}",
                public_key_path.display(),
                err
            ))
        })?;
    }
    if change_permissions {
        chown(private_key_path.as_path(), key_dir_uid, key_dir_gid)?;
        chown(public_key_path.as_path(), key_dir_uid, key_dir_gid)?;
    }

    Ok(())
}

/// Creates a public/private key pair.
///
/// Returns both keys if successful
fn create_key_pair() -> Result<(PrivateKey, PublicKey), CliError> {
    let context = Secp256k1Context::new();

    let private_key = context.new_random_private_key();
    let public_key = context
        .get_public_key(&private_key)
        .map_err(|err| CliError::ActionError(format!("Failed to get public key: {}", err)))?;
    Ok((private_key, public_key))
}
