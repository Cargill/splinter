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

mod authenticate;
mod gameroom;
mod gameroom_websocket;
mod node;
mod notification;
mod proposal;
mod submit;
mod xo_games;

pub use authenticate::*;
pub use gameroom::*;
pub use gameroom_websocket::*;
pub use node::*;
pub use notification::*;
pub use proposal::*;
pub use submit::*;
pub use xo_games::*;

use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};

pub const DEFAULT_LIMIT: usize = 100;
pub const DEFAULT_OFFSET: usize = 0;
const MAX_LIMIT: usize = 1000;

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'=')
    .add(b'!')
    .add(b'{')
    .add(b'}')
    .add(b'[')
    .add(b']')
    .add(b':')
    .add(b',');

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse<T: Serialize> {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

impl ErrorResponse<String> {
    pub fn internal_error() -> ErrorResponse<String> {
        ErrorResponse {
            code: "500".to_string(),
            message: "The server encountered an error".to_string(),
            data: None,
        }
    }

    pub fn bad_request(message: &str) -> ErrorResponse<String> {
        ErrorResponse {
            code: "400".to_string(),
            message: message.to_string(),
            data: None,
        }
    }

    pub fn not_found(message: &str) -> ErrorResponse<String> {
        ErrorResponse {
            code: "404".to_string(),
            message: message.to_string(),
            data: None,
        }
    }

    pub fn unauthorized(message: &str) -> ErrorResponse<String> {
        ErrorResponse {
            code: "401".to_string(),
            message: message.to_string(),
            data: None,
        }
    }
}

impl<T: Serialize> ErrorResponse<T> {
    pub fn bad_request_with_data(message: &str, data: T) -> ErrorResponse<T> {
        ErrorResponse {
            code: "400".to_string(),
            message: message.to_string(),
            data: Some(data),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessResponse<T: Serialize> {
    data: T,
    #[serde(skip_serializing_if = "Option::is_none")]
    paging: Option<Paging>,
}

impl<T: Serialize> SuccessResponse<T> {
    pub fn new(data: T) -> SuccessResponse<T> {
        SuccessResponse { data, paging: None }
    }

    pub fn list(data: T, paging: Paging) -> SuccessResponse<T> {
        SuccessResponse {
            data,
            paging: Some(paging),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Paging {
    current: String,
    offset: usize,
    limit: usize,
    total: usize,
    first: String,
    prev: String,
    next: String,
    last: String,
}

pub fn get_response_paging_info(
    limit: usize,
    offset: usize,
    link: &str,
    query_count: usize,
) -> Paging {
    let limit = validate_limit(limit);
    let offset = offset as i64;
    let query_count = query_count as i64;

    let base_link = format!("{}limit={}&", link, limit);

    let current_link = format!("{}offset={}", base_link, offset);

    let first_link = format!("{}offset=0", base_link);

    let previous_offset = if offset > limit { offset - limit } else { 0 };
    let previous_link = format!("{}offset={}", base_link, previous_offset);

    let last_offset = if query_count > 0 {
        ((query_count - 1) / limit) * limit
    } else {
        0
    };
    let last_link = format!("{}offset={}", base_link, last_offset);

    let next_offset = if offset + limit > last_offset {
        last_offset
    } else {
        offset + limit
    };

    let next_link = format!("{}offset={}", base_link, next_offset);

    Paging {
        current: current_link,
        offset: offset as usize,
        limit: limit as usize,
        total: query_count as usize,
        first: first_link,
        prev: previous_link,
        next: next_link,
        last: last_link,
    }
}

pub fn validate_limit(limit: usize) -> i64 {
    if limit > MAX_LIMIT {
        DEFAULT_LIMIT as i64
    } else {
        limit as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LINK: &str = "/api/test?";

    #[test]
    fn test_default_paging_response() {
        // Create paging response from default limit, default offset, a total of 1000
        let test_paging_response =
            get_response_paging_info(DEFAULT_LIMIT, DEFAULT_OFFSET, TEST_LINK, 1000);
        let generated_paging_response =
            create_test_paging_response(DEFAULT_LIMIT, DEFAULT_OFFSET, 100, 0, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50offset_paging_response() {
        // Create paging response from default limit, offset of 50, and a total of 1000
        let test_paging_response = get_response_paging_info(DEFAULT_LIMIT, 50, TEST_LINK, 1000);
        let generated_paging_response = create_test_paging_response(DEFAULT_LIMIT, 50, 150, 0, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_650offset_paging_response() {
        // Create paging response from default limit, offset value of 650, and a total of 1000
        let test_paging_response = get_response_paging_info(DEFAULT_LIMIT, 650, TEST_LINK, 1000);
        let generated_paging_response =
            create_test_paging_response(DEFAULT_LIMIT, 650, 750, 550, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50limit_paging_response() {
        // Create paging response from limit of 50, default offset, and a total of 1000
        let test_paging_response = get_response_paging_info(50, DEFAULT_OFFSET, TEST_LINK, 1000);
        let generated_paging_response = create_test_paging_response(50, DEFAULT_OFFSET, 50, 0, 950);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50limit_250offset_paging_response() {
        // Create paging response from limit of 50, offset of 250, and total of 1000
        let test_paging_response = get_response_paging_info(50, 250, TEST_LINK, 1000);
        let generated_paging_response = create_test_paging_response(50, 250, 300, 200, 950);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_maximum_invalid_limit() {
        // Pass an invalid limit, greater than the maximum i64 value which should cause the
        // paging response to include the DEFAULT_LIMIT.
        // Also uses the DEFAULT_OFFSET and a total of 1000.
        let max_limit: usize = 1001;
        let test_paging_response =
            get_response_paging_info(max_limit, DEFAULT_OFFSET, TEST_LINK, 1000);
        let generated_paging_response =
            create_test_paging_response(DEFAULT_LIMIT, DEFAULT_OFFSET, 100, 0, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    fn create_test_paging_response(
        limit: usize,
        offset: usize,
        next_offset: usize,
        previous_offset: usize,
        last_offset: usize,
    ) -> Paging {
        // Creates a generated paging response from the limit and offset values passed into the function
        let base_link = format!("{}limit={}&", TEST_LINK, limit);
        let current_link = format!("{}offset={}", base_link, offset);
        let first_link = format!("{}offset=0", base_link);
        let next_link = format!("{}offset={}", base_link, next_offset);
        let previous_link = format!("{}offset={}", base_link, previous_offset);
        let last_link = format!("{}offset={}", base_link, last_offset);

        Paging {
            current: current_link,
            offset,
            limit,
            total: 1000,
            first: first_link,
            prev: previous_link,
            next: next_link,
            last: last_link,
        }
    }
}
