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

use std::sync::Arc;

use crate::error::InternalError;
use crate::rest_api::auth::authorization::Permission;

use super::HandlerFunction;
use super::Method;

#[derive(Clone)]
pub(super) struct ResourceMethod {
    method: Method,
    permission: Permission,
    handler: Arc<HandlerFunction>,
}

impl ResourceMethod {
    pub fn new(method: Method, permission: Permission, handler: Arc<HandlerFunction>) -> Self {
        Self {
            method,
            permission,
            handler,
        }
    }

    pub fn builder() -> ResourceMethodBuilder {
        ResourceMethodBui lder::default()
    }

    pub fn method(&self) -> Method {
        self.method
    }

    #[cfg(feature = "authorization")]
    pub fn permission(&self) -> Permission {
        self.permission
    }

    pub fn handler(&self) -> Arc<HandlerFunction> {
        self.handler.clone()
    }
}

#[derive(Default)]
pub(super) struct ResourceMethodBuilder {
    method: Option<Method>,
    #[cfg(feature = "authorization")]
    permission: Option<Permission>,
    handler: Option<Arc<HandlerFunction>>,
}

impl ResourceMethodBuilder {
    pub(super) fn with_method(mut self, method: Method) -> Self {
        self.method = Some(method);
        self
    }

    pub(super) fn with_handler(mut self, handler: Arc<HandlerFunction>) -> Self {
        self.handler = Some(handler);
        self
    }

    #[cfg(feature = "authorization")]
    pub(super) fn with_permission(mut self, permission: Permission) -> Self {
        self.permission = Some(permission);
        self
    }

    pub(super) fn build(self) -> Result<ResourceMethod, InternalError> {
        let method = self
            .method
            .ok_or_else(|| InternalError::with_message("Method must be specified".to_string()))?;
        let handler = self
            .handler
            .ok_or_else(|| InternalError::with_message("Handler must be specified".to_string()))?;
        #[cfg(feature = "authorization")]
        let permission = self.permission.ok_or_else(|| {
            InternalError::with_message("Permission must be specified".to_string())
        })?;

        Ok(ResourceMethod {
            method,
            handler,
            #[cfg(feature = "authorization")]
            permission,
        })
    }
}
