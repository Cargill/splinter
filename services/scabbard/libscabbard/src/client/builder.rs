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

//! A convenient client for interacting with scabbard services on a Splinter node.

use super::error::ScabbardClientError;
use super::ScabbardClient;

/// Builder for building a [`ScabbardClient`](crate::client::ScabbardClient).
#[derive(Default)]
pub struct ScabbardClientBuilder {
    url: Option<String>,
}

impl ScabbardClientBuilder {
    /// Creates a new `ScabbardClientBuilder`.
    pub fn new() -> Self {
        ScabbardClientBuilder::default()
    }

    /// Sets the `url` field of the `ScabbardClientBuilder`. The url will be used
    /// as the bind endpoint for the Splinter REST API.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the bind endpoint of the Splinter REST API.
    pub fn with_url(mut self, url: &str) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Builds a `ScabbardClient`.
    ///
    /// # Errors
    ///
    /// Returns an error in any of the following cases:
    /// * Returns an error if url is not set
    pub fn build(self) -> Result<ScabbardClient, ScabbardClientError> {
        Ok(ScabbardClient {
            url: self.url.ok_or_else(|| {
                ScabbardClientError::new("Failed to build client, url not provided")
            })?,
        })
    }
}
