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

use std::error::Error;
use std::fmt;

use actix_web::Error as ActixError;

#[derive(Debug)]
pub enum ResponseError {
    ActixError(ActixError),
    InternalError(String),
}

impl Error for ResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ResponseError::ActixError(err) => Some(err),
            ResponseError::InternalError(_) => None,
        }
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResponseError::ActixError(err) => write!(
                f,
                "Failed to get response when setting up websocket: {}",
                err
            ),
            ResponseError::InternalError(msg) => f.write_str(&msg),
        }
    }
}

impl From<ActixError> for ResponseError {
    fn from(err: ActixError) -> Self {
        ResponseError::ActixError(err)
    }
}
