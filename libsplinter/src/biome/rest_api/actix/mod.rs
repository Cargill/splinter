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

#[cfg(any(feature = "biome-key-management", feature = "biome-credentials"))]
pub(crate) mod authorize;
#[cfg(feature = "biome-key-management")]
pub(super) mod key_management;
#[cfg(feature = "biome-credentials")]
pub(super) mod login;
#[cfg(feature = "biome-credentials")]
pub(super) mod logout;
#[cfg(feature = "biome-credentials")]
pub(super) mod register;
#[cfg(feature = "biome-credentials")]
pub(super) mod token;
#[cfg(feature = "biome-credentials")]
pub(super) mod user;
#[cfg(feature = "biome-credentials")]
pub(super) mod verify;
