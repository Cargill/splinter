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
use std::fmt;

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

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Id: {}", self.role_id)?;
        write!(f, "\n    Name: {}", self.display_name)?;
        f.write_str("\n    Permissions:")?;

        for perm in self.permissions.iter() {
            write!(f, "\n        {}", perm)?;
        }

        Ok(())
    }
}

/// Constructs roles for submission to a splinter node.
#[derive(Default)]
pub struct RoleBuilder {
    role_id: Option<String>,
    display_name: Option<String>,
    permissions: Vec<String>,
}

impl RoleBuilder {
    /// Sets the role id of the resulting Role.
    ///
    /// Must not be empty.
    pub fn with_role_id(mut self, role_id: String) -> Self {
        self.role_id = Some(role_id);
        self
    }

    /// Sets the display name of the resulting Role.
    pub fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    /// Sets the permissions included in the resulting Role.
    ///
    /// Must not be empty.
    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.permissions = permissions;
        self
    }

    /// Constructs the Role.
    pub fn build(self) -> Result<Role, CliError> {
        let RoleBuilder {
            role_id,
            display_name,
            permissions,
        } = self;

        if permissions.is_empty() {
            return Err(CliError::ActionError(
                "A role must have at least one permission".into(),
            ));
        }

        let role_id =
            role_id.ok_or_else(|| CliError::ActionError("A role must have a role ID".into()))?;
        if role_id.is_empty() {
            return Err(CliError::ActionError("A role ID must not be blank".into()));
        }

        let display_name = display_name
            .ok_or_else(|| CliError::ActionError("A role must have a display name".into()))?;

        Ok(Role {
            role_id,
            display_name,
            permissions,
        })
    }
}

#[derive(Deserialize)]
struct RoleGet {
    #[serde(rename = "data")]
    role: Role,
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

pub fn get_role(base_url: &str, auth: &str, role_id: &str) -> Result<Role, CliError> {
    Client::new()
        .get(&format!("{}/authorization/roles/{}", base_url, role_id))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to fetch role {}: {}", role_id, err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                res.json::<RoleGet>().map_err(|_| {
                    CliError::ActionError(
                        "Request was successful, but received an invalid response".into(),
                    )
                })
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else if status.as_u16() == 404 {
                Err(CliError::ActionError(format!(
                    "Role {} does not exist",
                    role_id
                )))
            } else {
                let message = res
                    .json::<super::ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Get role fetch request failed with status code '{}', but error \
                                 response was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to get role {}: {}",
                    role_id, message
                )))
            }
        })
        .map(|wrapper| wrapper.role)
}

pub fn create_role(base_url: &str, auth: &str, role: Role) -> Result<(), CliError> {
    Client::new()
        .post(&format!("{}/authorization/roles", base_url))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .json(&role)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to create role: {}", err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else {
                let message = res
                    .json::<super::ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Create role request failed with status code '{}', but error response \
                            was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to create role: {}",
                    message
                )))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests the role builder in both Ok and Err scenarios
    /// 1. Construct a valid role
    /// 2. Fail with no role_id
    /// 3. Fail with an empty role_id
    /// 4. Fail with no display name
    /// 4. Succeed with empty display name
    /// 5. Fail with empty permissions
    #[test]
    fn test_role_builder() {
        // Ok Role
        let role = RoleBuilder::default()
            .with_role_id("valid_role".into())
            .with_display_name("Valid Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build()
            .expect("could not build a valid role");

        assert_eq!("valid_role", &role.role_id);
        assert_eq!("Valid Role", &role.display_name);
        assert_eq!(vec!["a".to_string(), "b".to_string()], role.permissions);

        // Missing role_id
        let res = RoleBuilder::default()
            .with_display_name("No ID Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build();

        assert!(res.is_err());

        // Empty role_id
        let res = RoleBuilder::default()
            .with_role_id("".into())
            .with_display_name("Empty ID Role".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build();
        assert!(res.is_err());

        // No display name
        let res = RoleBuilder::default()
            .with_role_id("no_display_name".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build();
        assert!(res.is_err());

        // Empty display name
        RoleBuilder::default()
            .with_role_id("empty_display_name".into())
            .with_display_name("".into())
            .with_permissions(vec!["a".to_string(), "b".to_string()])
            .build()
            .expect("Could not build a role with an empty display name");

        // Empty permissions
        let res = RoleBuilder::default()
            .with_role_id("empty_permissions".into())
            .with_display_name("Empty Permissions".into())
            .with_permissions(vec![])
            .build();
        assert!(res.is_err());
    }
}
