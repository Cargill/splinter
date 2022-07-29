// Copyright 2018-2022 Cargill Incorporated
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

//! A file-backed authorization handler for defining admin keys

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use log::{error, warn};
use splinter::error::InternalError;

use crate::auth::identity::Identity;

use super::{AuthorizationHandler, AuthorizationHandlerResult};

/// A file-backed authorization handler that permits admin keys
///
/// The authorization handler only accepts [`Identity::Key`] identities; if a different type of
/// identity is provided, the handler will return [`AuthorizationHandlerResult::Continue`]. If a key
/// is provided, the handler will check if the key is present in the backing file. If the key is in
/// the backing file, the handler will return [`AuthorizationHandlerResult::Allow`]; if not, it will
/// return [`AuthorizationHandlerResult::Continue`]. The `permission_id` argument for
/// [`AuthorizationHandler::has_permission`] is ignored because this authorization handler provides
/// admin privileges (all permissions).
///
/// The authorization handler's backing file must be a list of keys separated by newlines.
///
/// The list of keys in the file are cached in-memory by the authorization handler; this means that
/// the handler will not have to read from the file every time permissions are checked. Instead,
/// each time the handler checks for permissions, it will check the backing file for any changes
/// since the last read, refreshing the internal cache if necessary. If the backing file does not
/// exist, is removed, or becomes unavailable, the authorization handler will treat the list of keys
/// as empty (all permission checks will receive a [`AuthorizationHandlerResult::Continue`] result).
#[derive(Clone)]
pub struct AllowKeysAuthorizationHandler {
    internal: Arc<Mutex<Internal>>,
}

impl AllowKeysAuthorizationHandler {
    /// Constructs a new `AllowKeysAuthorizationHandler`.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the backing allow keys file.
    pub fn new(file_path: &str) -> Result<Self, InternalError> {
        Ok(Self {
            internal: Arc::new(Mutex::new(Internal::new(file_path)?)),
        })
    }
}

impl AuthorizationHandler for AllowKeysAuthorizationHandler {
    fn has_permission(
        &self,
        identity: &Identity,
        _permission_id: &str,
    ) -> Result<AuthorizationHandlerResult, InternalError> {
        match identity {
            Identity::Key(key)
                if self
                    .internal
                    .lock()
                    .map_err(|_| {
                        InternalError::with_message(
                            "allow keys authorization handler internal lock poisoned".into(),
                        )
                    })?
                    .get_keys()
                    .contains(key) =>
            {
                Ok(AuthorizationHandlerResult::Allow)
            }
            _ => Ok(AuthorizationHandlerResult::Continue),
        }
    }

    fn clone_box(&self) -> Box<dyn AuthorizationHandler> {
        Box::new(self.clone())
    }
}

/// Internal state of the authorization handler
struct Internal {
    file_path: String,
    cached_keys: Vec<String>,
    last_read: SystemTime,
}

impl Internal {
    fn new(file_path: &str) -> Result<Self, InternalError> {
        let mut internal = Self {
            file_path: file_path.into(),
            cached_keys: vec![],
            last_read: SystemTime::UNIX_EPOCH,
        };

        // Read the file if it exists; otherwise just set the read the time.
        if PathBuf::from(file_path).is_file() {
            internal.read_keys()?;
        } else {
            internal.last_read = SystemTime::now();
        }

        Ok(internal)
    }

    /// Gets the internal list of keys. If the backing file has been modified since the last read,
    /// attempts to refresh the cache. If the file is unavailable, clears the cache.
    fn get_keys(&mut self) -> &[String] {
        let file_read_result = std::fs::metadata(&self.file_path)
            .and_then(|metadata| metadata.modified())
            .map_err(|err| {
                InternalError::from_source_with_message(
                    Box::new(err),
                    "failed to read allow keys file's last modification time".into(),
                )
            })
            .and_then(|last_modified| {
                if last_modified > self.last_read {
                    self.read_keys()
                } else {
                    Ok(())
                }
            });

        // If an error occurred with checking or reading the backing file, treat the file as empty
        // (clear the cache)
        if let Err(err) = file_read_result {
            warn!("Failed to read from allow keys file: {}", err);
            self.cached_keys.clear();
        }

        &self.cached_keys
    }

    /// Reads the backing file and caches its contents, logging an error for any key that can't be
    /// read
    fn read_keys(&mut self) -> Result<(), InternalError> {
        let file = File::open(&self.file_path).map_err(|err| {
            InternalError::from_source_with_message(
                Box::new(err),
                "failed to open allow keys file".into(),
            )
        })?;
        let keys = BufReader::new(file)
            .lines()
            .enumerate()
            .filter_map(|(idx, res)| {
                match res {
                    Ok(line) => Some(line),
                    Err(err) => {
                        error!(
                            "Failed to read key from line {} of allow keys file: {}",
                            idx + 1, // Lines are 1-indexed, iterators are 0-indexed
                            err
                        );
                        None
                    }
                }
            })
            .collect();

        self.cached_keys = keys;
        self.last_read = SystemTime::now();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::remove_file;
    use std::io::Write;
    use std::thread::sleep;
    use std::time::Duration;

    use tempfile::Builder;

    const KEY1: &str = "012345";
    const KEY2: &str = "abcdef";

    /// Verifies that the `AllowKeysAuthorizationHandler` returns `AuthorizationResult::Continue`
    /// when an unexpected identity (not a key) is passed in.
    ///
    /// 1. Create a new allow keys file in a temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by the file
    /// 3. Call `has_permission` with identities that aren't keys and verify the correct result is
    ///    returned
    #[test]
    fn auth_handler_unexpected_identity() {
        let temp_dir = Builder::new()
            .prefix("auth_handler_unexpected_identity")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();
        write_to_file(&[KEY1], &path);

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        assert!(matches!(
            handler.has_permission(&Identity::Custom("identity".into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));
        #[cfg(any(feature = "biome-credentials", feature = "oauth"))]
        assert!(matches!(
            handler.has_permission(&Identity::User("user_id".into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));
    }

    /// Verifies that the `AllowKeysAuthorizationHandler` returns `AuthorizationResult::Continue`
    /// when an unknown key is passed in.
    ///
    /// 1. Create a new allow keys file in a temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by the file
    /// 3. Call `has_permission` with a key that isn't in the file and verify the correct result is
    ///    returned
    #[test]
    fn auth_handler_unknown_key() {
        let temp_dir = Builder::new()
            .prefix("auth_handler_unknown_key")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();
        write_to_file(&[KEY1], &path);

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY2.into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));
    }

    /// Verifies that the `AllowKeysAuthorizationHandler` returns `AuthorizationResult::Allow` when
    /// when a known key is passed in.
    ///
    /// 1. Create a new allow keys file in a temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by the file
    /// 3. Call `has_permission` with keys that are in the file and verify the correct results are
    ///    returned
    #[test]
    fn auth_handler_allow() {
        let temp_dir = Builder::new()
            .prefix("auth_handler_allow")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();
        write_to_file(&[KEY1, KEY2], &path);

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY1.into()), "permission"),
            Ok(AuthorizationHandlerResult::Allow),
        ));
        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY2.into()), "permission"),
            Ok(AuthorizationHandlerResult::Allow),
        ));
    }

    /// Verifies that the `AllowKeysAuthorizationHandler` reloads the keys from the backing file if
    /// it was modified since the last read.
    ///
    /// 1. Create a new, empty allow keys file in a temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by the file
    /// 3. Write some keys to the file
    /// 4. Call `has_permission` with the keys that were written to the file and verify the correct
    ///    results are returned
    /// 5. Remove a key from the file (overwrite the file without the key)
    /// 6. Call `has_permission` with the key that was removed and verify the correct result is
    ///    returned
    #[test]
    fn reload_modified_file() {
        let temp_dir = Builder::new()
            .prefix("reload_modified_file")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();
        write_to_file(&[], &path);

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        // Allow some time before writing the file to make sure the read time is earlier than the
        // write time; the system clock may not be very precise.
        sleep(Duration::from_secs(1));

        write_to_file(&[KEY1, KEY2], &path);

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY1.into()), "permission"),
            Ok(AuthorizationHandlerResult::Allow),
        ));
        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY2.into()), "permission"),
            Ok(AuthorizationHandlerResult::Allow),
        ));

        // Allow some time before writing the file to make sure the read time is earlier than the
        // write time; the system clock may not be very precise.
        sleep(Duration::from_secs(1));

        write_to_file(&[KEY1], &path);

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY2.into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));
    }

    /// Verifies that the `AllowKeysAuthorizationHandler` treats the list of keys as empty if the
    /// backing file is removed.
    ///
    /// 1. Create a new allow keys file in a temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by the file
    /// 3. Remove the backing file
    /// 4. Call `has_permission` with a key that was in the file and verify that `Continue` is
    ///    returned
    #[test]
    fn file_removed() {
        let temp_dir = Builder::new()
            .prefix("file_removed")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();
        write_to_file(&[KEY1], &path);

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        remove_file(&path).expect("Failed to remove file");
        assert!(!PathBuf::from(&path).exists());

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY1.into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));
    }

    fn write_to_file(keys: &[&str], file_path: &str) {
        let mut file = File::create(file_path).expect("Failed to create allow keys file");
        for key in keys {
            writeln!(file, "{}", key).expect("Failed to write key to file");
        }
    }

    /// Verifies that the `AllowKeysAuthorizationHandler` is able to start without an existing file
    /// and load the file once it's created.
    ///
    /// 1. Create a new temp directory
    /// 2. Create a new `AllowKeysAuthorizationHandler` backed by a non-existent file in the temp
    ///    directory
    /// 3. Verify that `has_permission` returns `Continue`
    /// 3. Create the backing file with a key
    /// 4. Call `has_permission` with the key in the file and verify that `Allow` is returned
    #[test]
    fn load_after_file_created() {
        let temp_dir = Builder::new()
            .prefix("load_after_file_created")
            .tempdir()
            .expect("Failed to create temp dir");
        let path = temp_dir
            .path()
            .join("allow_keys")
            .to_str()
            .expect("Failed to get path")
            .to_string();

        let handler = AllowKeysAuthorizationHandler::new(&path).expect("Failed to create handler");

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY1.into()), "permission"),
            Ok(AuthorizationHandlerResult::Continue),
        ));

        // Allow some time before writing the file to make sure the last read time is earlier than
        // the write time; the system clock may not be very precise.
        sleep(Duration::from_secs(1));

        write_to_file(&[KEY1], &path);

        assert!(matches!(
            handler.has_permission(&Identity::Key(KEY1.into()), "permission"),
            Ok(AuthorizationHandlerResult::Allow),
        ));
    }
}
