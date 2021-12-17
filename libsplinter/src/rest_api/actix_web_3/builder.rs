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

//! Contains the implementation of `RestApiBuilder`.

use crate::error::InvalidStateError;
use crate::rest_api::BindConfig;

use super::{ResourceProvider, RunnableRestApi};

/// Builds a `RunnableRestApi`.
///
/// This builder's primary function is to create the runnable REST API in a valid state.
#[derive(Default)]
pub struct RestApiBuilder {
    resource_providers: Vec<Box<dyn ResourceProvider>>,
    bind: Option<BindConfig>,
}

impl RestApiBuilder {
    /// Constructs a new `RestApiBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the bind value, which will be used when binding to one or more ports.
    pub fn with_bind(mut self, value: BindConfig) -> Self {
        self.bind = Some(value);
        self
    }

    /// Appends a resource provider to the internal list kept by the builder. The resource
    /// providers' resources will be used when starting up the REST API, and thus determine the
    /// available endpoints.
    pub fn add_resource_provider(mut self, resource_provider: Box<dyn ResourceProvider>) -> Self {
        self.resource_providers.push(resource_provider);
        self
    }

    /// Validate the arguments and build the `RunnableRestApi` struct.
    pub fn build(self) -> Result<RunnableRestApi, InvalidStateError> {
        let bind = self.bind.ok_or_else(|| {
            InvalidStateError::with_message("Missing required field: 'bind'".to_string())
        })?;

        Ok(RunnableRestApi {
            bind,
            resource_providers: self.resource_providers,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Verifies that the `RestApiThreadBuilder` builds succesfully when all required configuration
    /// is provided.
    #[test]
    fn rest_api_thread_builder_successful() {
        let builder = RestApiBuilder::new().with_bind(BindConfig::Http("test".into()));

        assert!(builder.build().is_ok())
    }
}
