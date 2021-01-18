// Copyright (c) 2019 Target Brands, Inc.
// Copyright 2018-2020 Cargill Incorporated
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

use std::collections::HashMap;

use crate::actix_web::{web, Error, HttpResponse};
use crate::futures::{future::IntoFuture, Future};
use crate::rest_api::{
    paging::{PagingQuery, DEFAULT_LIMIT, DEFAULT_OFFSET},
    ErrorResponse,
};

/// Return the PagingQuery from a Query object of key-value pairs.
pub fn get_paging_query(
    query: &web::Query<HashMap<String, String>>,
) -> Result<PagingQuery, BadPagingRequest> {
    let offset = match query.get("offset") {
        Some(value) => value.parse::<usize>().map_err(|err| {
            BadPagingRequest::new(format!(
                "Invalid offset value passed: {}. Error: {}",
                value, err
            ))
        })?,
        None => DEFAULT_OFFSET,
    };

    let limit = match query.get("limit") {
        Some(value) => value.parse::<usize>().map_err(|err| {
            BadPagingRequest::new(format!(
                "Invalid limit value passed: {}. Error: {}",
                value, err
            ))
        })?,
        None => DEFAULT_LIMIT,
    };

    Ok(PagingQuery { offset, limit })
}

#[derive(Debug)]
pub struct BadPagingRequest {
    message: String,
}

impl std::fmt::Display for BadPagingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BadPagingRequest {}

impl BadPagingRequest {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl IntoFuture for BadPagingRequest {
    type Future = Box<(dyn Future<Item = HttpResponse, Error = Error> + 'static)>;
    type Item = HttpResponse;
    type Error = Error;

    fn into_future(self) -> Self::Future {
        Box::new(
            HttpResponse::BadRequest()
                .json(ErrorResponse::bad_request(&self.message))
                .into_future(),
        )
    }
}
