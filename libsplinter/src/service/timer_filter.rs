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

//! Contains `TimerFilter` trait.

use crate::error::InternalError;

use super::{FullyQualifiedServiceId, Routable};

/// Return service IDs for services that have pending work.
///
/// Every service type will implement a `TimerFilter` to figure out which services
/// need to be woken up to handle pending work. The `TimerFilter` must also be `Routable`
/// for certain service types.
pub trait TimerFilter: Routable {
    /// Return a list of `FullyQualifiedServiceId` for service that have work to perform
    fn filter(&self) -> Result<Vec<FullyQualifiedServiceId>, InternalError>;
}
