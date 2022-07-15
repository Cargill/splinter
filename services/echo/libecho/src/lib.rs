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

#![allow(clippy::extra_unused_lifetimes)]

#[cfg(feature = "diesel_migrations")]
pub mod migrations;
pub mod service;
pub mod store;

#[macro_use]
#[cfg(any(feature = "sqlite", feature = "postgres"))]
extern crate diesel;
#[cfg(feature = "diesel_migrations")]
#[macro_use]
extern crate diesel_migrations;
