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

use crate::events::ParseError;

pub trait ParseBytes<T: 'static>: Send + Sync + Clone {
    fn from_bytes(bytes: &[u8]) -> Result<T, ParseError>;
}

impl ParseBytes<Vec<u8>> for Vec<u8> {
    fn from_bytes(bytes: &[u8]) -> Result<Vec<u8>, ParseError> {
        Ok(bytes.to_vec())
    }
}
