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

use super::schema::refresh_tokens;

#[derive(Queryable, Identifiable, PartialEq, Debug)]
#[table_name = "refresh_tokens"]
#[primary_key(id)]
pub struct RefreshToken {
    pub id: i64,
    pub user_id: String,
    pub token: String,
}

#[derive(AsChangeset, Insertable, PartialEq, Debug)]
#[table_name = "refresh_tokens"]
pub struct NewRefreshToken<'a> {
    pub user_id: &'a str,
    pub token: &'a str,
}
