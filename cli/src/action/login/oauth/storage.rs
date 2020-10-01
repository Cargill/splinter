// Copyright 2018-2020 Cargill Incorporated
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

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::BufReader;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use super::error::OAuth2TokenStorageError;
use super::UserTokens;

const ACCESS_TOKEN_FILENAME: &str = "access_token";

/// Struct for saving UserTokens to the file.
#[derive(serde::Serialize)]
struct SaveTokens<'a> {
    provider_type: &'a str,
    access_token: &'a str,
    refresh_token: Option<&'a str>,
}

impl<'a> From<&'a UserTokens> for SaveTokens<'a> {
    fn from(other: &'a UserTokens) -> Self {
        Self {
            provider_type: &other.provider_type,
            access_token: &other.access_token,
            refresh_token: other.refresh_token.as_deref(),
        }
    }
}
/// Struct for reading UserTokens from the file.
#[derive(serde::Deserialize)]
struct ReadTokens {
    provider_type: String,
    access_token: String,
    refresh_token: Option<String>,
}

impl From<ReadTokens> for UserTokens {
    fn from(read_tokens: ReadTokens) -> Self {
        Self {
            provider_type: read_tokens.provider_type,
            access_token: read_tokens.access_token,
            refresh_token: read_tokens.refresh_token,
        }
    }
}

/// Read user authentication tokens from the user's splinter directory.
///
/// Note, this is currently prefixed with an underscore, as it is not yet used.
pub fn _read_tokens(
    user_splinter_dir: &PathBuf,
) -> Result<Option<UserTokens>, OAuth2TokenStorageError> {
    let mut token_path = user_splinter_dir.clone();
    token_path.push(ACCESS_TOKEN_FILENAME);
    match File::open(token_path) {
        Ok(file) => {
            let buf_reader = BufReader::new(file);
            match serde_yaml::from_reader::<_, ReadTokens>(buf_reader) {
                Ok(read_tokens) => Ok(Some(read_tokens.into())),
                Err(err) => {
                    debug!("Unable to read OAuth2 credential file: {}", err);
                    Err(OAuth2TokenStorageError(
                        "Unable to read OAuth2 credentials: File Corrupted".into(),
                    ))
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => Err(
            OAuth2TokenStorageError("Unable to read OAuth2 credentials: Permission Denied".into()),
        ),
        Err(_) => Err(OAuth2TokenStorageError(
            "Unable to read OAuth2 credentials".into(),
        )),
    }
}

/// Save user authentication tokens from the user's splinter directory.
pub fn save_tokens(
    user_splinter_dir: &PathBuf,
    tokens: &UserTokens,
) -> Result<(), OAuth2TokenStorageError> {
    match DirBuilder::new().create(user_splinter_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(OAuth2TokenStorageError(format!(
                "Unable to create {}: Permission Denied",
                user_splinter_dir.as_path().display()
            )))
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(err) => {
            debug!(
                "Failed to create {}: {}",
                user_splinter_dir.as_path().display(),
                err
            );
            return Err(OAuth2TokenStorageError(format!(
                "Unable to create {}",
                user_splinter_dir.as_path().display()
            )));
        }
        Ok(()) => (),
    }

    let mut token_path = user_splinter_dir.clone();
    token_path.push(ACCESS_TOKEN_FILENAME);

    let mut opts = OpenOptions::new();
    opts.write(true);
    opts.create(true);

    #[cfg(unix)]
    {
        opts.mode(0o640);
    }

    match opts.open(token_path) {
        Ok(file) => serde_yaml::to_writer(file, &SaveTokens::from(tokens))
            .map_err(|_| OAuth2TokenStorageError("Unable to write OAuth2 credentials".into())),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => Err(
            OAuth2TokenStorageError("Unable to write OAuth2 credentials: Permission Denied".into()),
        ),
        Err(_err) => Err(OAuth2TokenStorageError(
            "Unable to write OAuth2 credentials".into(),
        )),
    }
}
