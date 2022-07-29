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

pub mod batch_statuses;
pub mod batches;
pub mod state;

#[cfg(feature = "authorization")]
use crate::auth::Permission;

#[cfg(feature = "authorization")]
pub const SCABBARD_READ_PERMISSION: Permission = Permission::Check {
    permission_id: "scabbard.read",
    permission_display_name: "Scabbard read",
    permission_description: "Allows the client to read scabbard services' state and batch statuses",
};
#[cfg(feature = "authorization")]
pub const SCABBARD_WRITE_PERMISSION: Permission = Permission::Check {
    permission_id: "scabbard.write",
    permission_display_name: "Scabbard write",
    permission_description: "Allows the client to submit batches to scabbard services",
};

pub const SCABBARD_SUBSCRIBE_PROTOCOL_MIN: u32 = 1;
pub const SCABBARD_ADD_BATCHES_PROTOCOL_MIN: u32 = 1;
pub const SCABBARD_BATCH_STATUSES_PROTOCOL_MIN: u32 = 1;
pub const SCABBARD_GET_STATE_PROTOCOL_MIN: u32 = 1;
pub const SCABBARD_LIST_STATE_PROTOCOL_MIN: u32 = 1;
pub const SCABBARD_STATE_ROOT_PROTOCOL_MIN: u32 = 1;
