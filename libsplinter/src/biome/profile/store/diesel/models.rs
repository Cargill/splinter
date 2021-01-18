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

use crate::biome::profile::store::Profile;

use super::schema::user_profile;

#[derive(Insertable, Queryable, Identifiable, Associations, PartialEq, Debug)]
#[table_name = "user_profile"]
#[primary_key(user_id)]
pub struct ProfileModel {
    pub user_id: String,
    pub subject: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: Option<String>,
    pub picture: Option<String>,
}

impl From<Profile> for ProfileModel {
    fn from(profile: Profile) -> Self {
        ProfileModel {
            user_id: profile.user_id,
            subject: profile.subject,
            name: profile.name,
            given_name: profile.given_name,
            family_name: profile.family_name,
            email: profile.email,
            picture: profile.picture,
        }
    }
}
