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

use augrim::Value;

#[derive(Clone)]
pub struct ScabbardValue(Vec<u8>);

impl ScabbardValue {
    pub fn new(val: Vec<u8>) -> Self {
        Self(val)
    }
}

impl Value for ScabbardValue {}

impl From<Vec<u8>> for ScabbardValue {
    fn from(val: Vec<u8>) -> Self {
        Self(val)
    }
}

impl From<ScabbardValue> for Vec<u8> {
    fn from(val: ScabbardValue) -> Self {
        val.0
    }
}
