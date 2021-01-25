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

table! {
   roles (id) {
        id -> Text,
        display_name -> Text,
    }
}

table! {
    role_permissions (role_id, permission) {
        role_id -> Text,
        permission -> Text,
    }
}

joinable!(role_permissions -> roles (role_id));
allow_tables_to_appear_in_same_query!(roles, role_permissions);

table! {
    identities (identity) {
        identity -> Text,
        identity_type -> SmallInt,
    }
}

table! {
    assignments (identity, role_id) {
        identity -> Text,
        role_id -> Text,
    }
}
