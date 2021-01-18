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

#[cfg(feature = "rest-api-actix")]
pub mod actix;
pub mod resources;

#[cfg(feature = "authorization")]
use splinter::rest_api::auth::authorization::Permission;

#[cfg(all(feature = "authorization", feature = "rest-api-actix"))]
const SCABBARD_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "scabbard.read",
    permission_display_name: "Scabbard read",
    permission_description: "Allows the client to read scabbard services' state and batch statuses",
};
#[cfg(all(feature = "authorization", feature = "rest-api-actix"))]
const SCABBARD_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "scabbard.write",
    permission_display_name: "Scabbard write",
    permission_description: "Allows the client to submit batches to scabbard services",
};
