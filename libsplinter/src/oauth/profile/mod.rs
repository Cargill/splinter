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

//! APIs and implementations for fetching profile details from OAuth servers

mod github;
mod openid;

use crate::error::InternalError;
use crate::oauth::Profile;

pub use github::GithubProfileProvider;
pub use openid::OpenIdProfileProvider;

/// A service that fetches profile details from a backing OAuth server
pub trait ProfileProvider: Send + Sync {
    /// Attempts to get the profile details for the account that the given access token is for.
    fn get_profile(&self, access_token: &str) -> Result<Option<Profile>, InternalError>;

    /// Clone implementation for `ProfileProvider`. The implementation of the `Clone` trait for
    /// `Box<dyn ProfileProvider>` calls this method.
    fn clone_box(&self) -> Box<dyn ProfileProvider>;
}

impl Clone for Box<dyn ProfileProvider> {
    fn clone(&self) -> Box<dyn ProfileProvider> {
        self.clone_box()
    }
}
