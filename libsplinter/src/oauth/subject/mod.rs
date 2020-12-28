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

//! APIs and implementations for fetching subject identifiers from OAuth servers

#[cfg(feature = "oauth-github")]
mod github;
#[cfg(feature = "oauth-openid")]
mod openid;

use crate::error::InternalError;

#[cfg(feature = "oauth-github")]
pub use github::GithubSubjectProvider;
#[cfg(feature = "oauth-openid")]
pub use openid::OpenIdSubjectProvider;

/// A service that fetches subject identifiers from a backing OAuth server
pub trait SubjectProvider: Send + Sync {
    /// Attempts to get the subject that the given access token is for. This method will return
    /// `Ok(None)` if the access token could not be resolved to a subject.
    fn get_subject(&self, access_token: &str) -> Result<Option<String>, InternalError>;

    /// Clone implementation for `SubjectProvider`. The implementation of the `Clone` trait for
    /// `Box<dyn SubjectProvider>` calls this method.
    ///
    /// # Example
    ///
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn SubjectProvider> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn SubjectProvider>;
}

impl Clone for Box<dyn SubjectProvider> {
    fn clone(&self) -> Box<dyn SubjectProvider> {
        self.clone_box()
    }
}
