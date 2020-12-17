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

use crate::biome::oauth::store::{InsertableOAuthUserSession, OAuthUser};

use super::schema::{oauth_user_sessions, oauth_users};

#[derive(Debug, PartialEq, Identifiable, Insertable, Queryable)]
#[table_name = "oauth_users"]
#[primary_key(subject)]
pub struct OAuthUserModel {
    pub subject: String,
    pub user_id: String,
}

#[derive(Debug, PartialEq, Associations, Identifiable, Queryable)]
#[table_name = "oauth_user_sessions"]
#[belongs_to(OAuthUserModel, foreign_key = "subject")]
#[primary_key(splinter_access_token)]
pub struct OAuthUserSessionModel {
    pub splinter_access_token: String,
    pub subject: String,
    pub oauth_access_token: String,
    pub oauth_refresh_token: Option<String>,
    pub last_authenticated: i64,
}

#[derive(Debug, PartialEq, Insertable)]
#[table_name = "oauth_user_sessions"]
pub struct InsertableOAuthUserSessionModel {
    pub splinter_access_token: String,
    pub subject: String,
    pub oauth_access_token: String,
    pub oauth_refresh_token: Option<String>,
}

impl From<OAuthUser> for OAuthUserModel {
    fn from(user: OAuthUser) -> Self {
        let OAuthUser { subject, user_id } = user;
        OAuthUserModel { subject, user_id }
    }
}

impl From<OAuthUserModel> for OAuthUser {
    fn from(user: OAuthUserModel) -> Self {
        let OAuthUserModel { subject, user_id } = user;
        OAuthUser { subject, user_id }
    }
}

impl From<InsertableOAuthUserSession> for InsertableOAuthUserSessionModel {
    fn from(session: InsertableOAuthUserSession) -> Self {
        let InsertableOAuthUserSession {
            splinter_access_token,
            subject,
            oauth_access_token,
            oauth_refresh_token,
        } = session;
        InsertableOAuthUserSessionModel {
            splinter_access_token,
            subject,
            oauth_access_token,
            oauth_refresh_token,
        }
    }
}
