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
#[cfg(feature = "metrics")]
pub mod influx;

// no-op `counter` macro for when the `metrics` feature is not enabled
#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! counter {
    ($t:tt, $v:expr) => {};
    ($t:tt, $v:expr, $($key:expr => $value:expr)*) => {};
}

// no-op `gauge` macro for when the `metrics` feature is not enabled
#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! gauge {
    ($t:tt, $v:expr) => {};
    ($t:tt, $v:expr, $($key:expr => $value:expr)*) => {};
}

// no-op `histogram` macro for when the `metrics` feature is not enabled
#[cfg(not(feature = "metrics"))]
#[macro_export]
macro_rules! histogram {
    ($t:tt, $v:expr) => {};
    ($t:tt, $v:expr, $($key:expr => $value:expr)*) => {};
}
