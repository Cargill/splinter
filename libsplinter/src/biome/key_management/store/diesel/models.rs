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

use super::schema::keys;

#[derive(Insertable, Queryable, Identifiable, PartialEq, Eq, Debug)]
#[table_name = "keys"]
#[primary_key(public_key, user_id)]
pub struct KeyModel {
    pub public_key: String,
    pub encrypted_private_key: String,
    pub user_id: String,
    pub display_name: String,
}
