// Copyright 2018-2021 Cargill Incorporated
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

use crate::actix_web::HttpResponse;
use crate::biome::profile::store::UserProfileStore;
use crate::futures::IntoFuture;
#[cfg(feature = "authorization")]
use crate::rest_api::auth::authorization::Permission;
use crate::rest_api::{
    actix_web_1::{HandlerFunction, Method, ProtocolVersionRangeGuard, Resource},
    auth::identity::Identity,
    ErrorResponse, BIOME_PROTOCOL_VERSION,
};

const BIOME_FETCH_PROFILE_PROTOCOL_MIN: u32 = 1;

pub fn make_profile_route(profile_store: Arc<dyn UserProfileStore>) -> Resource {
    let resource = Resource::build("/biome/profile").add_request_guard(
        ProtocolVersionRangeGuard::new(BIOME_FETCH_PROFILE_PROTOCOL_MIN, BIOME_PROTOCOL_VERSION),
    );
    #[cfg(feature = "authorization")]
    {
        resource.add_method(
            Method::Get,
            Permission::AllowAuthenticated,
            handle_get(profile_store),
        )
    }
    #[cfg(not(feature = "authorization"))]
    {
        resource.add_method(Method::Get, handle_get(profile_store))
    }
}

/// Defines a REST endpoint for retrieving the profile of the authenticated user
fn handle_get(profile_store: Arc<dyn UserProfileStore>) -> HandlerFunction {
    Box::new(move |request, _| {
        let profile_store = profile_store.clone();

        let user = match request.extensions().get::<Identity>() {
            Some(Identity::User(user)) => user.clone(),
            _ => {
                return Box::new(
                    HttpResponse::Unauthorized()
                        .json(ErrorResponse::unauthorized())
                        .into_future(),
                )
            }
        };

        match profile_store.get_profile(&user) {
            Ok(profile) => Box::new(HttpResponse::Ok().json(profile).into_future()),
            Err(err) => {
                debug!("Failed to fetch profile {}", err);
                Box::new(
                    HttpResponse::InternalServerError()
                        .json(ErrorResponse::internal_error())
                        .into_future(),
                )
            }
        }
    })
}
