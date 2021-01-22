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

use std::convert::TryFrom;

use crate::error::InvalidStateError;
use crate::rest_api::{
    auth::rbac::store::{Assignment, AssignmentBuilder, Identity},
    paging::Paging,
};

#[derive(Serialize)]
pub struct ListAssignmentsResponse<'a> {
    pub data: Vec<AssignmentResponse<'a>>,
    pub paging: Paging,
}

#[derive(Serialize)]
pub struct AssignmentResponse<'a> {
    #[serde(flatten)]
    identity: IdentityResponse<'a>,
    roles: &'a [String],
}

#[derive(Serialize)]
#[serde(tag = "identity_type", content = "identity")]
#[serde(rename_all = "lowercase")]
pub enum IdentityResponse<'a> {
    Key(&'a str),
    User(&'a str),
}

impl<'a> From<&'a Assignment> for AssignmentResponse<'a> {
    fn from(assignment: &'a Assignment) -> Self {
        Self {
            identity: assignment.identity().into(),
            roles: assignment.roles(),
        }
    }
}

impl<'a> From<&'a Identity> for IdentityResponse<'a> {
    fn from(identity: &'a Identity) -> Self {
        match identity {
            Identity::User(user) => IdentityResponse::User(user),
            Identity::Key(key) => IdentityResponse::Key(key),
        }
    }
}

#[derive(Deserialize)]
pub struct AssignmentPayload {
    #[serde(flatten)]
    identity: IdentityPayload,
    roles: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "identity_type", content = "identity")]
#[serde(rename_all = "lowercase")]
pub enum IdentityPayload {
    Key(String),
    User(String),
}

impl TryFrom<AssignmentPayload> for Assignment {
    type Error = InvalidStateError;

    fn try_from(
        AssignmentPayload { identity, roles }: AssignmentPayload,
    ) -> Result<Self, Self::Error> {
        AssignmentBuilder::new()
            .with_identity(match identity {
                IdentityPayload::Key(key) => Identity::Key(key),
                IdentityPayload::User(user) => Identity::User(user),
            })
            .with_roles(roles)
            .build()
    }
}
