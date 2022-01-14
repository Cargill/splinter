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

//! Framework of builders for integration testing

mod builder;
mod error;
mod veil;

pub use self::builder::{CircuitBuilder, CircuitData, CircuitService};
pub use self::error::{AddScabbardServiceError, CircuitBuildError};
pub use self::veil::scabbard::ScabbardCircuitBuilderVeil;

use crate::framework::network::Network;
use splinter::error::InvalidArgumentError;
use splinterd::node::Node;

/// A generic node collection
pub trait NodeCollection {
    /// Get a running node that is a part of the network
    fn node(&self, id: usize) -> Result<&Node, InvalidArgumentError>;
}

impl NodeCollection for Network {
    fn node(&self, id: usize) -> Result<&Node, InvalidArgumentError> {
        Network::node(&self, id)
    }
}
