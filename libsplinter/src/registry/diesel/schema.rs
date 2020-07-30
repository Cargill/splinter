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

//! Provides database schemas for the `DieselRegistry`.

table! {
    splinter_nodes (identity) {
        identity -> Text,
        display_name -> Text,
    }
}

table! {
    splinter_nodes_endpoints (identity, endpoint) {
        identity -> Text,
        endpoint -> Text,
    }
}

table! {
    splinter_nodes_keys (identity, key) {
        identity -> Text,
        key -> Text,
    }
}

table! {
    splinter_nodes_metadata (identity, key) {
        identity -> Text,
        key -> Text,
        value -> Text,
    }
}
