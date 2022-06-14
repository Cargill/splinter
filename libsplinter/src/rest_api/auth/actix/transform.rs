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

use actix_web::dev::*;
use actix_web::Error as ActixError;
use futures::future::{ok, FutureResult};

use crate::rest_api::auth::actix::AuthorizationMiddleware;
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::AuthorizationHandler;
use crate::rest_api::auth::IdentityProvider;

/// Wrapper for the authorization middleware
#[derive(Clone)]
pub struct Authorization {
    identity_providers: Vec<Box<dyn IdentityProvider>>,
    #[cfg(feature = "authorization")]
    authorization_handlers: Vec<Box<dyn AuthorizationHandler>>,
}

impl Authorization {
    pub fn new(
        identity_providers: Vec<Box<dyn IdentityProvider>>,
        #[cfg(feature = "authorization")] authorization_handlers: Vec<
            Box<dyn AuthorizationHandler>,
        >,
    ) -> Self {
        Self {
            identity_providers,
            #[cfg(feature = "authorization")]
            authorization_handlers,
        }
    }
}

impl<S, B> Transform<S> for Authorization
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<B>, Error = ActixError>,
    S::Future: 'static,
    B: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<B>;
    type Error = S::Error;
    type InitError = ();
    type Transform = AuthorizationMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuthorizationMiddleware {
            identity_providers: self.identity_providers.clone(),
            #[cfg(feature = "authorization")]
            authorization_handlers: self.authorization_handlers.clone(),
            service,
        })
    }
}
