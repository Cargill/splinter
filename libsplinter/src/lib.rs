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

#![cfg_attr(feature = "benchmark", feature(test))]

#[macro_use]
extern crate log;
#[cfg(any(feature = "admin-service", feature = "rest-api", feature = "registry"))]
#[macro_use]
extern crate serde_derive;
#[macro_use]
#[cfg(feature = "rest-api")]
extern crate serde_json;
#[macro_use]
#[cfg(feature = "diesel")]
extern crate diesel;
#[macro_use]
#[cfg(feature = "diesel")]
extern crate diesel_migrations;

#[macro_export]
macro_rules! rwlock_read_unwrap {
    ($lock:expr) => {
        match $lock.read() {
            Ok(d) => d,
            Err(e) => panic!("RwLock error: {:?}", e),
        }
    };
}

#[macro_export]
macro_rules! rwlock_write_unwrap {
    ($lock:expr) => {
        match $lock.write() {
            Ok(d) => d,
            Err(e) => panic!("RwLock error: {:?}", e),
        }
    };
}

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
pub mod channel;
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
#[cfg(feature = "oauth")]
pub mod oauth;
pub mod orchestrator;
pub mod peer;
pub mod protocol;
pub mod protos;
#[cfg(feature = "registry")]
pub mod registry;
#[cfg(feature = "rest-api")]
pub mod rest_api;
#[cfg(feature = "run")]
pub mod run;
pub mod service;
pub mod sets;
#[cfg(feature = "store-factory")]
pub mod store;
pub mod threading;
pub mod transport;

#[cfg(feature = "rest-api")]
pub use actix_web;
#[cfg(feature = "rest-api")]
pub use futures;
