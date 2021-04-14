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

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::action::api::ServerError;
use crate::error::CliError;

use super::{Pageable, RBAC_PROTOCOL_VERSION};

#[derive(Deserialize, Serialize)]
#[serde(tag = "identity_type", content = "identity")]
#[serde(rename_all = "lowercase")]
pub enum Identity {
    Key(String),
    User(String),
}

impl Identity {
    /// Returns a tuple of the parts (id, id_type)
    /// Type can be "key" or "user"
    pub fn parts(&self) -> (&str, &str) {
        match self {
            Identity::Key(key) => (key, "key"),
            Identity::User(user) => (user, "user"),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Assignment {
    #[serde(flatten)]
    pub identity: Identity,
    pub roles: Vec<String>,
}

impl Pageable for Assignment {
    fn label() -> &'static str {
        "assignment list"
    }
}

#[derive(Deserialize)]
struct AssignmentGet {
    #[serde(rename = "data")]
    assignment: Assignment,
}

#[derive(Default)]
pub struct AssignmentBuilder {
    identity: Option<Identity>,
    roles: Vec<String>,
}

impl AssignmentBuilder {
    pub fn with_identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }

    pub fn build(self) -> Result<Assignment, CliError> {
        let AssignmentBuilder { identity, roles } = self;

        if roles.is_empty() {
            return Err(CliError::ActionError(
                "An assignment must have at least on role".into(),
            ));
        }
        let identity = identity.ok_or_else(|| {
            CliError::ActionError("An assignment must have an associated identity".into())
        })?;

        match &identity {
            Identity::Key(key) => {
                if key.is_empty() {
                    return Err(CliError::ActionError("A key must not be empty".into()));
                }
            }
            Identity::User(user) => {
                if user.is_empty() {
                    return Err(CliError::ActionError("A user ID must not be empty".into()));
                }
            }
        }

        Ok(Assignment { identity, roles })
    }
}

#[derive(Serialize)]
pub struct AssignmentUpdate {
    #[serde(skip)]
    identity: Identity,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<String>>,
}

#[derive(Default)]
pub struct AssignmentUpdateBuilder {
    identity: Option<Identity>,
    roles: Option<Vec<String>>,
}

impl AssignmentUpdateBuilder {
    pub fn with_identity(mut self, identity: Identity) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_roles(mut self, roles: Option<Vec<String>>) -> Self {
        self.roles = roles;
        self
    }

    pub fn build(self) -> Result<AssignmentUpdate, CliError> {
        let AssignmentUpdateBuilder { identity, roles } = self;

        let identity = identity.ok_or_else(|| {
            CliError::ActionError("An assignment must have an associated identity".into())
        })?;

        if let Some(roles) = roles.as_ref() {
            if roles.is_empty() {
                return Err(CliError::ActionError(
                    "An assignment must have at least on role".into(),
                ));
            }
        }

        Ok(AssignmentUpdate { identity, roles })
    }
}

pub fn create_assignment(
    base_url: &str,
    auth: &str,
    assignment: Assignment,
) -> Result<(), CliError> {
    Client::new()
        .post(&format!("{}/authorization/assignments", base_url))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .json(&assignment)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to create assignment: {}", err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else if status.as_u16() == 409 {
                Err(CliError::ActionError("One or more of the roles provided does not exist".into()))
            } else {
                let message = res
                    .json::<ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Create assignment request failed with status code '{}', but error response \
                            was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to create assignment: {}",
                    message
                )))
            }
        })
}

pub fn get_assignment(
    base_url: &str,
    auth: &str,
    identity: &Identity,
) -> Result<Option<Assignment>, CliError> {
    let (id_value, id_type) = identity.parts();

    Client::new()
        .get(&format!(
            "{}/authorization/assignments/{}/{}",
            base_url, id_type, id_value
        ))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .send()
        .map_err(|err| {
            CliError::ActionError(format!(
                "Failed to fetch authorized identity {} {}: {}",
                id_type, id_value, err
            ))
        })
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                res.json::<AssignmentGet>()
                    .map_err(|_| {
                        CliError::ActionError(
                            "Request was successful, but received an invalid response".into(),
                        )
                    })
                    .map(|wrapper| Some(wrapper.assignment))
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else if status.as_u16() == 404 {
                Ok(None)
            } else {
                let message = res
                    .json::<ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Get authorized identity request failed with status code '{}', but \
                            error response was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to get authorized identity {} {}: {}",
                    id_type, id_value, message
                )))
            }
        })
}

pub fn update_assignment(
    base_url: &str,
    auth: &str,
    assignment_update: AssignmentUpdate,
) -> Result<(), CliError> {
    let (id_value, id_type) = assignment_update.identity.parts();

    Client::new()
        .patch(&format!("{}/authorization/assignments/{}/{}", base_url, id_type, id_value))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .json(&assignment_update)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to update assignment: {}", err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else if status.as_u16() == 404 {
                Err(CliError::ActionError(format!(
                    "Authorized identity {} {} does not exist",
                    id_type, id_value,
                )))
            } else if status.as_u16() == 409 {
                Err(CliError::ActionError("One or more of the roles provided does not exist".into()))
            } else {
                let message = res
                    .json::<ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Update assignment request failed with status code '{}', but error response \
                            was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to update assignment: {}",
                    message
                )))
            }
        })
}

pub fn delete_assignment(base_url: &str, auth: &str, identity: &Identity) -> Result<(), CliError> {
    let (id_value, id_type) = identity.parts();

    Client::new()
        .delete(&format!(
            "{}/authorization/assignments/{}/{}",
            base_url, id_type, id_value
        ))
        .header("SplinterProtocolVersion", RBAC_PROTOCOL_VERSION)
        .header("Authorization", auth)
        .send()
        .map_err(|err| CliError::ActionError(format!("Failed to delete assignment: {}", err)))
        .and_then(|res| {
            let status = res.status();
            if status.is_success() {
                Ok(())
            } else if status.as_u16() == 401 {
                Err(CliError::ActionError("Not Authorized".into()))
            } else if status.as_u16() == 404 {
                Err(CliError::ActionError(format!(
                    "Authorized identity {} {} does not exist",
                    id_type, id_value,
                )))
            } else if status.as_u16() == 409 {
                Err(CliError::ActionError(
                    "One or more of the roles provided does not exist".into(),
                ))
            } else {
                let message = res
                    .json::<ServerError>()
                    .map_err(|_| {
                        CliError::ActionError(format!(
                            "Delete assignment request failed with status code '{}', but error \
                            response was not valid",
                            status
                        ))
                    })?
                    .message;

                Err(CliError::ActionError(format!(
                    "Failed to delete assignment: {}",
                    message
                )))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests the assignment builder in both Ok and Err scenarios
    /// 1. Construct a valid assignment (key)
    /// 2. Construct a valid assignment (user)
    /// 3. Fail with no identity
    /// 4. Fail with empty identity value (key)
    /// 5. Fail with empty identity value (user)
    /// 6. Fail with empty roles
    #[test]
    fn test_assignment_builder() {
        // Valid assignment with key
        let assignment = AssignmentBuilder::default()
            .with_identity(Identity::Key("abcd".into()))
            .with_roles(vec!["role1".to_string(), "role2".to_string()])
            .build()
            .expect("Could not build a valid role");

        assert!(matches!(assignment.identity, Identity::Key(key) if key == "abcd"));
        assert_eq!(
            vec!["role1".to_string(), "role2".to_string()],
            assignment.roles
        );

        // Valid assignment with user
        let assignment = AssignmentBuilder::default()
            .with_identity(Identity::User("user-123".into()))
            .with_roles(vec!["role1".to_string(), "role2".to_string()])
            .build()
            .expect("Could not build a valid role");

        assert!(matches!(assignment.identity, Identity::User(user) if user == "user-123"));
        assert_eq!(
            vec!["role1".to_string(), "role2".to_string()],
            assignment.roles
        );

        // Fail with missing identity
        let res = AssignmentBuilder::default()
            .with_roles(vec!["role1".to_string(), "role2".to_string()])
            .build();
        assert!(res.is_err());

        // Fail with empty key
        let res = AssignmentBuilder::default()
            .with_identity(Identity::Key(String::new()))
            .with_roles(vec!["role1".to_string(), "role2".to_string()])
            .build();
        assert!(res.is_err());

        // Fail with empty user
        let res = AssignmentBuilder::default()
            .with_identity(Identity::User(String::new()))
            .with_roles(vec!["role1".to_string(), "role2".to_string()])
            .build();
        assert!(res.is_err());

        // Fail with empty roles
        let res = AssignmentBuilder::default()
            .with_identity(Identity::Key("abcd".into()))
            .build();
        assert!(res.is_err());
    }

    /// Tests the assignment builder in both Ok and Err scenarios
    /// 1. Construct valid update (key)
    /// 2. Construct valid update (user)
    /// 3. Construct valid no roles
    /// 3. Fail with no identity
    /// 4. Fail with empty roles
    #[test]
    fn test_assignment_update_builder() {
        // Valid assignment with key
        let assignment = AssignmentUpdateBuilder::default()
            .with_identity(Identity::Key("abcd".into()))
            .with_roles(Some(vec!["role1".to_string(), "role2".to_string()]))
            .build()
            .expect("Could not build a valid role");

        assert!(matches!(assignment.identity, Identity::Key(key) if key == "abcd"));
        assert_eq!(
            Some(vec!["role1".to_string(), "role2".to_string()]),
            assignment.roles
        );

        // Valid assignment with user
        let assignment = AssignmentUpdateBuilder::default()
            .with_identity(Identity::User("user-123".into()))
            .with_roles(Some(vec!["role1".to_string(), "role2".to_string()]))
            .build()
            .expect("Could not build a valid role");

        assert!(matches!(assignment.identity, Identity::User(user) if user == "user-123"));
        assert_eq!(
            Some(vec!["role1".to_string(), "role2".to_string()]),
            assignment.roles
        );

        // Valid assignment with user, no roles
        let assignment = AssignmentUpdateBuilder::default()
            .with_identity(Identity::User("user-123".into()))
            .build()
            .expect("Could not build a valid role");

        assert!(matches!(assignment.identity, Identity::User(user) if user == "user-123"));
        assert_eq!(None, assignment.roles);

        // Fail with missing identity
        let res = AssignmentUpdateBuilder::default()
            .with_roles(Some(vec!["role1".to_string(), "role2".to_string()]))
            .build();
        assert!(res.is_err());
    }
}
