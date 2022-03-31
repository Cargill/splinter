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

use splinter::{error::InternalError, service::ArgumentsConverter};

use super::ScabbardArguments;

pub struct ScabbardArgumentsVecConverter {}

impl ArgumentsConverter<ScabbardArguments, Vec<(String, String)>>
    for ScabbardArgumentsVecConverter
{
    fn to_right(&self, _left: ScabbardArguments) -> Result<Vec<(String, String)>, InternalError> {
        Ok(vec![])
    }

    fn to_left(&self, _right: Vec<(String, String)>) -> Result<ScabbardArguments, InternalError> {
        ScabbardArguments::new().map_err(|e| InternalError::from_source(Box::new(e)))
    }
}
