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

//! Role-based Authorization REST API resources.

#[cfg(feature = "rest-api-actix")]
mod actix_web_1;
mod resources;

use crate::rest_api::auth::Permission;

#[cfg(feature = "rest-api-actix")]
pub use actix_web_1::RoleBasedAuthorizationResourceProvider;

#[cfg(feature = "rest-api-actix")]
const RBAC_READ_PERMISSION: Permission = Permission::Check("authorization.rbac.read");
