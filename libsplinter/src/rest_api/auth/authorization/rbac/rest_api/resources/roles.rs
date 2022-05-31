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

//! REST API Response structs for Role-based authorization structs.

use std::convert::TryFrom;

use crate::error::InvalidStateError;
use crate::rbac::store::{Role, RoleBuilder};
use crate::rest_api::paging::Paging;

#[derive(Serialize)]
pub struct ListRoleResponse<'a> {
    pub data: Vec<RoleResponse<'a>>,
    pub paging: Paging,
}

#[derive(Serialize)]
pub struct RoleResponse<'a> {
    pub role_id: &'a str,
    pub display_name: &'a str,
    pub permissions: &'a [String],
}

#[derive(Deserialize)]
pub struct RolePayload {
    pub role_id: String,
    pub display_name: String,
    pub permissions: Vec<String>,
}

#[derive(Deserialize)]
pub struct RoleUpdatePayload {
    pub display_name: Option<String>,
    pub permissions: Option<Vec<String>>,
}

impl<'a> From<&'a Role> for RoleResponse<'a> {
    fn from(role: &'a Role) -> Self {
        Self {
            role_id: role.id(),
            display_name: role.display_name(),
            permissions: role.permissions(),
        }
    }
}

impl TryFrom<RolePayload> for Role {
    type Error = InvalidStateError;

    fn try_from(payload: RolePayload) -> Result<Self, Self::Error> {
        RoleBuilder::new()
            .with_id(payload.role_id)
            .with_display_name(payload.display_name)
            .with_permissions(payload.permissions)
            .build()
    }
}
