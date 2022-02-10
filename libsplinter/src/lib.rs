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

#![cfg_attr(feature = "benchmark", feature(test))]

#[macro_use]
extern crate log;
#[cfg(any(feature = "admin-service", feature = "rest-api", feature = "registry"))]
#[macro_use]
extern crate serde_derive;
#[macro_use]
#[cfg(feature = "rest-api-actix-web-1")]
extern crate serde_json;
#[macro_use]
#[cfg(feature = "diesel")]
extern crate diesel;
#[macro_use]
#[cfg(feature = "diesel")]
extern crate diesel_migrations;
#[cfg(feature = "tap")]
#[macro_use]
extern crate metrics;

// macros_use must come before any modules that make use of the macro
#[macro_use]
pub mod tap;

#[doc(hidden)]
#[macro_export]
macro_rules! rwlock_read_unwrap {
    ($lock:expr) => {
        match $lock.read() {
            Ok(d) => d,
            Err(e) => panic!("RwLock error: {:?}", e),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! rwlock_write_unwrap {
    ($lock:expr) => {
        match $lock.write() {
            Ok(d) => d,
            Err(e) => panic!("RwLock error: {:?}", e),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! mutex_lock_unwrap {
    ($lock:expr) => {
        match $lock.lock() {
            Ok(guard) => guard,
            Err(e) => panic!("Mutex error: {:?}", e),
        }
    };
}

#[cfg(feature = "admin-service")]
pub mod admin;
mod base62;
#[cfg(feature = "biome")]
pub mod biome;
pub(crate) mod channel;
pub mod circuit;
mod collections;
pub mod consensus;
pub mod error;
#[cfg(feature = "events")]
pub mod events;
mod hex;
pub mod keys;
pub mod mesh;
pub mod migrations;
pub mod network;
#[cfg(feature = "node-id-store")]
pub mod node_id;
#[cfg(feature = "oauth")]
pub mod oauth;
pub mod orchestrator;
pub mod peer;
pub mod protocol;
pub mod protos;
pub mod public_key;
#[cfg(feature = "registry")]
pub mod registry;
#[cfg(feature = "rest-api")]
pub mod rest_api;
pub mod service;
#[cfg(feature = "store")]
pub mod store;
pub mod threading;
pub mod transport;

#[cfg(feature = "rest-api-actix-web-1")]
pub use actix_web;
#[cfg(feature = "rest-api-actix-web-1")]
pub use futures;
