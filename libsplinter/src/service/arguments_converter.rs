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

//! Contains `ArgumentsConverter` trait.

use crate::error::InternalError;

/// Convert between two different argument formats.
///
/// Commonly this conversion will be serialization and deserialization; for example, when
/// converting between a internal struct format and `Vec<(String, String)>`.
///
/// When implementing `ArgumentsConverter`, two generic type parameters must be specified: `L` and
/// `R`. `L` is for the left side, `R` is for the right side. The functions `to_left` and
/// `to_right` convert in the desired direction.
pub trait ArgumentsConverter<L, R> {
    /// Convert from generic type parameter `R` to type `L`.
    fn to_left(&self, right: R) -> Result<L, InternalError>;
    /// Convert from generic type parameter `L` to type `R`.
    fn to_right(&self, left: L) -> Result<R, InternalError>;
}
