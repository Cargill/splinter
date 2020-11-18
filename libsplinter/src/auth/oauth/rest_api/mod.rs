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

//! OAuth REST API endpoints

#[cfg(feature = "rest-api-actix")]
mod actix;
mod resources;

use crate::error::InternalError;
use crate::rest_api::{Resource, RestResourceProvider};

use super::{OAuthClient, UserInfo};

/// Saves a set of user info.
pub trait SaveUserInfoOperation: Sync + Send {
    /// Executes a save operation on the given user info.
    fn save_user_info(&self, user_info: &UserInfo) -> Result<(), InternalError>;

    /// Clone implementation for `SaveUserInfoOperation`. The implementation of the `Clone` trait
    /// for`Box<dyn SaveUserInfoOperation>` calls this method.
    ///
    /// # Example
    ///
    ///```ignore
    ///  fn clone_box(&self) -> Box<dyn SaveUserInfoOperation> {
    ///     Box::new(self.clone())
    ///  }
    ///```
    fn clone_box(&self) -> Box<dyn SaveUserInfoOperation>;
}

impl Clone for Box<dyn SaveUserInfoOperation> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// A no-op implementation of `SaveUserInfoOperation`.
pub struct SaveUserInfoToNull;

impl SaveUserInfoOperation for SaveUserInfoToNull {
    fn save_user_info(&self, _user_info: &UserInfo) -> Result<(), InternalError> {
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn SaveUserInfoOperation> {
        Box::new(SaveUserInfoToNull)
    }
}

/// Provides the REST API [Resource](../../../rest_api/struct.Resource.html) definitions for OAuth
/// endpoints. The following endpoints are provided:
///
/// * `GET /oauth/login` - Get the URL for requesting authorization from the provider
/// * `GET /oauth/callback` - Receive the authorization code from the provider
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
#[derive(Clone)]
pub(crate) struct OAuthResourceProvider {
    client: OAuthClient,
    save_user_info_operation: Box<dyn SaveUserInfoOperation>,
}

impl OAuthResourceProvider {
    /// Creates a new `OAuthResourceProvider`
    pub fn new(
        client: OAuthClient,
        save_user_info_operation: Box<dyn SaveUserInfoOperation>,
    ) -> Self {
        Self {
            client,
            save_user_info_operation,
        }
    }
}

/// `OAuthResourceProvider` provides the following endpoints as REST API resources:
///
/// * `GET /oauth/login` - Get the URL for requesting authorization from the provider
/// * `GET /oauth/callback` - Receive the authorization code from the provider
///
/// These endpoints are only available if the following REST API backend feature is enabled:
///
/// * `rest-api-actix`
impl RestResourceProvider for OAuthResourceProvider {
    fn resources(&self) -> Vec<Resource> {
        // Allowing unused_mut because resources must be mutable if feature `rest-api-actix` is
        // enabled
        #[allow(unused_mut)]
        let mut resources = Vec::new();

        #[cfg(feature = "rest-api-actix")]
        {
            resources.append(&mut vec![
                actix::login::make_login_route(self.client.clone()),
                actix::callback::make_callback_route(
                    self.client.clone(),
                    self.save_user_info_operation.clone(),
                ),
            ]);
        }

        resources
    }
}
