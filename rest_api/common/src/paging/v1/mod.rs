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

mod builder;

use serde::{Deserialize, Serialize};

pub use builder::PagingBuilder;

pub const DEFAULT_LIMIT: usize = 100;
pub const DEFAULT_OFFSET: usize = 0;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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

impl Paging {
    pub fn builder(link: String, query_count: usize) -> PagingBuilder {
        PagingBuilder::new(link, query_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LINK: &str = "/api/test?";

    #[test]
    fn test_default_paging_response() {
        // Create paging response from default limit, default offset, a total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000).build();
        let generated_paging_response =
            create_test_paging_response(DEFAULT_OFFSET, DEFAULT_LIMIT, 100, 0, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50offset_paging_response() {
        // Create paging response from default limit, offset of 50, and a total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000)
            .with_offset(50)
            .build();
        let generated_paging_response = create_test_paging_response(50, DEFAULT_LIMIT, 150, 0, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_550offset_paging_response() {
        // Create paging response from default limit, offset value of 550, and a total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000)
            .with_offset(550)
            .build();
        let generated_paging_response =
            create_test_paging_response(550, DEFAULT_LIMIT, 650, 450, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_950offset_paging_response() {
        // Create paging response from default limit, offset value of 950, and a total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000)
            .with_offset(950)
            .build();
        let generated_paging_response =
            create_test_paging_response(950, DEFAULT_LIMIT, 900, 850, 900);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50limit_paging_response() {
        // Create paging response from default limit, offset of 50, and a total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000)
            .with_limit(50)
            .build();
        let generated_paging_response = create_test_paging_response(DEFAULT_OFFSET, 50, 50, 0, 950);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    #[test]
    fn test_50limit_150offset_paging_response() {
        // Create paging response from limit of 50, offset of 150, and total of 1000
        let test_paging_response = Paging::builder(TEST_LINK.to_string(), 1000)
            .with_limit(50)
            .with_offset(150)
            .build();
        let generated_paging_response = create_test_paging_response(150, 50, 200, 100, 950);
        assert_eq!(test_paging_response, generated_paging_response);
    }

    fn create_test_paging_response(
        offset: usize,
        limit: usize,
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
