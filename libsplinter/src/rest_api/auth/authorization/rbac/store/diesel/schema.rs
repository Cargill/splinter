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

table! {
   rbac_roles (id) {
        id -> Text,
        display_name -> Text,
    }
}

table! {
    rbac_role_permissions (role_id, permission) {
        role_id -> Text,
        permission -> Text,
    }
}

joinable!(rbac_role_permissions -> rbac_roles (role_id));
allow_tables_to_appear_in_same_query!(rbac_roles, rbac_role_permissions);

table! {
    rbac_identities (identity) {
        identity -> Text,
        identity_type ->
            // the macro output can't find this type if it isn't fully qualified.
            crate::rest_api::auth::authorization::rbac::store::diesel::models::IdentityModelTypeMapping,
    }
}

table! {
    rbac_assignments (identity, role_id) {
        identity -> Text,
        role_id -> Text,
    }
}
