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

use super::{Paging, DEFAULT_LIMIT, DEFAULT_OFFSET};

pub struct PagingBuilder {
    link: String,
    limit: Option<usize>,
    offset: Option<usize>,
    query_count: usize,
}

impl PagingBuilder {
    pub fn new(link: String, query_count: usize) -> PagingBuilder {
        PagingBuilder {
            link,
            limit: None,
            offset: None,
            query_count,
        }
    }
}

impl PagingBuilder {
    pub fn with_limit(self, limit: usize) -> Self {
        Self {
            limit: Some(limit),
            ..self
        }
    }

    pub fn with_offset(self, offset: usize) -> Self {
        Self {
            offset: Some(offset),
            ..self
        }
    }

    pub fn build(self) -> Paging {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = self.offset.unwrap_or(DEFAULT_OFFSET);
        let link = self.link;

        let base_link = {
            // if the link does not already contain ? add it to the end
            if !link.contains('?') {
                format!("{}?limit={}&", link, limit)
            } else {
                format!("{}limit={}&", link, limit)
            }
        };

        let current_link = format!("{}offset={}", base_link, offset);

        let first_link = format!("{}offset=0", base_link);

        let previous_offset = if offset > limit { offset - limit } else { 0 };

        let previous_link = format!("{}offset={}", base_link, previous_offset);

        let query_count = self.query_count;
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
            offset,
            limit,
            total: query_count,
            first: first_link,
            prev: previous_link,
            next: next_link,
            last: last_link,
        }
    }
}
