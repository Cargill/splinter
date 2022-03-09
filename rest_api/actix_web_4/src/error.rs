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

use std::error::Error;
use std::fmt::Display;

use actix_web::http::StatusCode;
use actix_web::ResponseError;
use splinter::rest_api::RequestError;
use splinter_rest_api_common::error::ResponseError as CommonError;

#[derive(Debug)]
pub enum RestError {
    BadRequest(String),
    NotFound(String),
    InternalError(String, Option<Box<dyn Error>>),
    NotAuthorized,
}

impl Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestError::BadRequest(msg) => write!(f, "Bad Request: {}", msg),
            RestError::NotFound(url) => write!(f, "Could not find resource for: {}", url),
            RestError::InternalError(msg, Some(err)) => {
                write!(f, "Internal Error: {}: {}", msg, err)
            }
            RestError::InternalError(msg, None) => write!(f, "Internal Error: {}", msg),
            RestError::NotAuthorized => write!(f, "Not Authorized"),
        }
    }
}

impl From<CommonError> for RestError {
    fn from(source: CommonError) -> Self {
        match source {
            CommonError::BadRequest(msg) => RestError::BadRequest(msg),
            CommonError::NotFound(url) => RestError::NotFound(url),
            CommonError::InternalError(msg, err) => RestError::InternalError(msg, err),
            CommonError::NotAuthorized => RestError::NotAuthorized,
            err => RestError::InternalError("".to_string(), Some(Box::new(err))),
        }
    }
}

impl From<RequestError> for RestError {
    fn from(source: RequestError) -> Self {
        RestError::BadRequest(format!("{source}"))
    }
}

impl ResponseError for RestError {
    fn status_code(&self) -> StatusCode {
        match self {
            RestError::BadRequest(_) => StatusCode::BAD_REQUEST,
            RestError::NotFound(_) => StatusCode::NOT_FOUND,
            RestError::InternalError(_, _) => StatusCode::INTERNAL_SERVER_ERROR,
            RestError::NotAuthorized => StatusCode::UNAUTHORIZED,
        }
    }
}
