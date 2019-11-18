// Copyright 2019 Cargill Incorporated
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
use crate::futures::{Future, IntoFuture};
use crate::rest_api::{into_bytes, Method, Resource};

use super::super::notifications::{BiomeNewNotification, NotificationManager};
use super::notification_manager::BiomeNotificationManager;

pub fn make_add_route(notification_manager: Arc<BiomeNotificationManager>) -> Resource {
    Resource::new(Method::Post, "/biome/notifications", move |_, payload| {
        let notification_manager = notification_manager.clone();
        Box::new(into_bytes(payload).and_then(move |payload_bytes| {
            let notification = match serde_json::from_slice::<BiomeNewNotification>(&payload_bytes)
            {
                Ok(val) => val,
                Err(_err) => {
                    return HttpResponse::InternalServerError().finish().into_future();
                }
            };
            match notification_manager.add_notification(notification) {
                Ok(()) => (),
                Err(_err) => return HttpResponse::InternalServerError().finish().into_future(),
            };
            HttpResponse::Ok().finish().into_future()
        }))
    })
}
