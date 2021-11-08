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

//! Scabbard is a Splinter service that runs the Sawtooth Sabre smart contract engine using
//! Hyperledger Transact for state management. Scabbard uses two-phase consensus to reach agreement
//! on transactions.

#[cfg(feature = "log")]
#[macro_use]
extern crate log;
#[cfg(feature = "diesel")]
#[macro_use]
extern crate diesel;
#[cfg(feature = "diesel_migrations")]
#[macro_use]
extern crate diesel_migrations;

#[cfg(feature = "metrics")]
#[macro_use]
extern crate metrics;

// pull in `no-op` metric macros if `metrics` is not enabled
#[cfg_attr(all(not(feature = "metrics"), feature = "splinter-service"), macro_use)]
extern crate splinter;

#[cfg(feature = "client")]
pub mod client;
#[cfg(any(feature = "client-reqwest", feature = "splinter-service"))]
mod hex;
#[cfg(feature = "diesel_migrations")]
pub mod migrations;
pub mod protocol;
pub mod protos;
#[cfg(feature = "splinter-service")]
pub mod service;
pub mod store;
