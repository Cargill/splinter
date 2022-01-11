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

pub mod assignments;
pub mod roles;

use std::collections::VecDeque;

use reqwest::blocking::Client;
use serde::{de::DeserializeOwned, Deserialize};

use crate::CliError;

const RBAC_PROTOCOL_VERSION: u32 = 1;

#[derive(Deserialize)]
struct Page<T: DeserializeOwned> {
    #[serde(bound = "T: DeserializeOwned")]
    pub data: VecDeque<T>,
    pub paging: Paging,
}

#[derive(Deserialize)]
pub struct Paging {
    next: String,
    total: usize,
    limit: usize,
    offset: usize,
}

impl Paging {
    fn has_next(&self) -> bool {
        self.total - self.offset > self.limit
    }
}

pub trait Pageable: Sized + DeserializeOwned {
    fn label() -> &'static str;
}

pub struct PagingIter<'a, T>
where
    T: Pageable,
{
    url: &'a str,
    auth: &'a str,
    current_page: Option<Result<Page<T>, CliError>>,
    consumed: bool,
}

impl<'a, T> PagingIter<'a, T>
where
    T: Pageable,
{
    pub fn new(base_url: &'a str, auth: &'a str, initial_link: &str) -> PagingIter<'a, T> {
        PagingIter {
            url: base_url,
            auth,
            current_page: Some(load_page(base_url, auth, initial_link, T::label())),
            consumed: false,
        }
    }
}

impl<'a, T> Iterator for PagingIter<'a, T>
where
    T: Pageable,
{
    type Item = Result<T, CliError>;

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

            // Check to see if all the values from a page have been returned to the caller. If so,
            // and if there are still values on the server, load the next page. If not, mark the
            // iterator as consumed.
            if let Ok(current_page) = self.current_page.as_ref()?.as_ref() {
                if current_page.data.is_empty() {
                    let paging = &current_page.paging;
                    if paging.has_next() {
                        self.current_page =
                            Some(load_page(self.url, self.auth, &paging.next, T::label()));
                    } else {
                        self.consumed = true;
                    }
                    continue;
                }
            }

            // There are still roles in the current page, and it's not an error, so pop the next
            // value off of the page.
            break self
                .current_page
                .as_mut()?
                .as_mut()
                .map(|page| page.data.pop_front())
                // We've examined the result, earlier, so this is unreachable. We still need to map
                // the error to make the compiler happy.
                .map_err(|_| unreachable!())
                // flip it from Result<Option<_>, _> to Option<Result<_, _>>
                .transpose();
        }
    }
}

fn load_page<T>(base_url: &str, auth: &str, link: &str, label: &str) -> Result<Page<T>, CliError>
where
    T: DeserializeOwned,
{
    Client::new()
        .get(&format!("{}{}", base_url, link))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to fetch {} page: {}", label, err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                res.json::<Page<T>>().map_err(|_| {
                    CliError::ActionError(
                        "Request was successful, but received an invalid response".into(),
                    )
                })
            } else {
                let message = res
                    .json::<super::ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Fetch {} request failed with status code '{}', but error \
                             response was not valid",
                            label, status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to fetch {} page: {}",
                    label, message
                )))
            }
        })
}
