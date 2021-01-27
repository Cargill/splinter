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

use std::collections::VecDeque;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::CliError;

const RBAC_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
pub struct Role {
    pub role_id: String,
    pub display_name: String,
    pub permissions: Vec<String>,
}

#[derive(Deserialize)]
pub struct RoleList {
    #[serde(rename = "data")]
    pub roles: VecDeque<Role>,
    pub paging: Paging,
}

#[derive(Deserialize)]
pub struct Paging {
    next: String,
    total: usize,
    limit: usize,
    offset: usize,
}

pub struct RoleIter<'a> {
    url: &'a str,
    auth: &'a str,
    current_page: Option<Result<RoleList, CliError>>,
    consumed: bool,
}

impl<'a> RoleIter<'a> {
    pub fn new(base_url: &'a str, auth: &'a str) -> Self {
        Self {
            url: base_url,
            auth,
            current_page: Self::load_page(base_url, auth, "/authorization/roles"),
            consumed: false,
        }
    }

    fn load_page(base_url: &str, auth: &str, link: &str) -> Option<Result<RoleList, CliError>> {
        let result = Client::new()
            .get(&format!("{}{}", base_url, link))
            .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
            .header("Authorization", auth)
            .send()
            .map_err(|err| {
                CliError::ActionError(format!("Failed to fetch role list page: {}", err))
            })
            .and_then(|res| {
                let status = res.status();
                if status.is_success() {
                    res.json::<RoleList>().map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                } else {
                    let message = res
                        .json::<super::ServerError>()
                        .map_err(|_| {
                            CliError::ActionError(format!(
                                "List roles fetch request failed with status code '{}', but error \
                                 response was not valid",
                                status
                            ))
                        })?
                        .message;

                    Err(CliError::ActionError(format!(
                        "Failed to fetch role list page: {}",
                        message
                    )))
                }
            });

        Some(result)
    }
}

impl<'a> Iterator for RoleIter<'a> {
    type Item = Result<Role, CliError>;

    fn next(&mut self) -> Option<Self::Item> {
        // This method loops to allow for a cache load.  At most, it will iterate twice.
        loop {
            // If the pages have all been consumed, return None
            if self.consumed {
                break None;
            }

            // Check to see if the page load resulted in an error.  If so, return the error and
            // mark the iterator as consumed.
            if self.current_page.as_ref()?.is_err() {
                // we have to destructure this to make the compiler happy, but don't
                // have to deal with the alternate branch, as we already know it's an
                // error.
                if let Some(Err(err)) = self.current_page.take() {
                    self.consumed = true;
                    break Some(Err(err));
                }
            }

            // Check to see if all the roles from a page have been returned to the caller. If so,
            // and if there are still roles on the server, load the next page. If not, mark the
            // iterator as consumed.
            if let Ok(current_page) = self.current_page.as_ref()?.as_ref() {
                if current_page.roles.is_empty() {
                    if current_page.paging.total - current_page.paging.offset
                        > current_page.paging.limit
                    {
                        self.current_page =
                            Self::load_page(self.url, self.auth, &current_page.paging.next);
                    } else {
                        self.consumed = true;
                    }
                    continue;
                }
            }

            // There are still roles in the current page, and it's not an error, so pop the next
            // role off of the page's deque.
            break self
                .current_page
                .as_mut()?
                .as_mut()
                .map(|page| page.roles.pop_front())
                // We've examined the result, earlier, so this is unreachable. We still need to map
                // the error to make the compiler happy.
                .map_err(|_| unreachable!())
                // flip it from Result<Option<_>, _> to Option<Result<_, _>>
                .transpose();
        }
    }
}
