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

/// An identity that may be assigned roles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Identity {
    /// A public key-based identity.
    Key(String),
    /// A user ID-based identity.
    User(String),
}

impl From<&crate::rest_api::auth::identity::Identity> for Option<Identity> {
    fn from(identity: &crate::rest_api::auth::identity::Identity) -> Self {
        match identity {
            // RoleBasedAuthorization does not currently support custom identities
            crate::rest_api::auth::identity::Identity::Custom(_) => None,
            crate::rest_api::auth::identity::Identity::Key(key) => {
                Some(Identity::Key(key.to_string()))
            }
            crate::rest_api::auth::identity::Identity::User(user_id) => {
                Some(Identity::User(user_id.to_string()))
            }
        }
    }
}
