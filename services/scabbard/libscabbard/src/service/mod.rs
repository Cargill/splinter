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

//! Scabbard is a Splinter `Service` that runs the Sawtooth Sabre smart contract engine using the
//! `transact` library for state. Scabbard uses two-phase consensus to reach agreement on
//! transactions.

mod error;
#[cfg(feature = "rest-api")]
mod rest_api;
mod state;
mod v2;
mod version;

pub use state::{
    BatchInfo, BatchInfoIter, BatchStatus, Events, StateChange, StateChangeEvent, StateIter,
};
#[cfg(feature = "rest-api")]
use v2::SERVICE_TYPE;
pub use v2::{
    ConnectionUri, Scabbard, ScabbardArgValidator, ScabbardFactory, ScabbardFactoryBuilder,
    ScabbardStatePurgeHandler, ScabbardStorageConfiguration,
};
pub use version::ScabbardVersion;
