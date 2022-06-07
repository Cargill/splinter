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

//! Authorization middleware for the Actix REST API

mod middleware;
mod transform;

pub use middleware::AuthorizationMiddleware;
pub use transform::Authorization;

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::dev::*;
    use actix_web::{http::StatusCode, test, web, App, HttpRequest};
    use actix_web::{
        http::{header::HeaderValue, Method as ActixMethod},
        HttpResponse,
    };

    use crate::error::InternalError;
    #[cfg(feature = "authorization")]
    use crate::rest_api::auth::authorization::Permission;
    #[cfg(feature = "authorization")]
    use crate::rest_api::auth::authorization::PermissionMap;
    use crate::rest_api::auth::identity::IdentityProvider;
    use crate::rest_api::auth::{identity::Identity, AuthorizationHeader};
    #[cfg(feature = "authorization")]
    use crate::rest_api::Method;

    /// Verifies that the authorization middleware sets the `Access-Control-Allow-Credentials: true`
    /// header for `OPTIONS` requests.
    #[test]
    fn auth_middleware_options_request_header() {
        let mut app = test::init_service(
            App::new()
                .wrap(Authorization::new(
                    vec![],
                    #[cfg(feature = "authorization")]
                    vec![],
                ))
                .route(
                    "/",
                    web::route()
                        .method(ActixMethod::OPTIONS)
                        .to(|| HttpResponse::Ok()),
                ),
        );

        let req = test::TestRequest::with_uri("/")
            .method(ActixMethod::OPTIONS)
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("Access-Control-Allow-Credentials"),
            Some(&HeaderValue::from_static("true"))
        );
    }

    /// Verifies that the authorization middleware returns a `403 Unauthorized` response when the
    /// `authorize` function returns an "unauthorized" result. This is simulated by not configuring
    /// any identity providers.
    #[test]
    fn auth_middleware_unauthorized() {
        let app = App::new()
            .wrap(Authorization::new(
                vec![],
                #[cfg(feature = "authorization")]
                vec![],
            ))
            .route("/", web::get().to(|| HttpResponse::Ok()));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::<Method>::new();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        let req = test::TestRequest::with_uri("/").to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Verifies that the authorization middleware returns a `404 Not Found` response when the
    /// `authorize` function returns an "unkonwn endpoint" result.
    #[test]
    fn auth_middleware_not_found() {
        let app = App::new()
            .wrap(Authorization::new(
                vec![Box::new(AlwaysAcceptIdentityProvider)],
                #[cfg(feature = "authorization")]
                vec![],
            ))
            .route("/", web::get().to(|| HttpResponse::Ok()));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::<Method>::new();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        let req = test::TestRequest::with_uri("/test")
            .header("Authorization", "test")
            .to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// Verifies that the authorization middleware allows requests that are properly authorized (the
    /// `authorize` function returns an "authorized" result), and that the client's identity is
    /// added to the request's extensions.
    #[test]
    fn auth_middleware_authorized() {
        let auth_middleware = Authorization::new(
            vec![Box::new(AlwaysAcceptIdentityProvider)],
            #[cfg(feature = "authorization")]
            vec![],
        );

        let app = App::new().wrap(auth_middleware).route(
            "/",
            web::get().to(|req: HttpRequest| {
                // Verify that the client's identity was added to the request extensions
                if req.extensions().get() == Some(&Identity::Custom("identity".into())) {
                    HttpResponse::Ok()
                } else {
                    HttpResponse::InternalServerError()
                }
            }),
        );

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::<Method>::new();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        // Need to provide some value for the `Authorization` header
        let req = test::TestRequest::with_uri("/")
            .header("Authorization", "test")
            .to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    /// An identity provider that always returns `Ok(Some(_))`
    #[derive(Clone)]
    struct AlwaysAcceptIdentityProvider;

    impl IdentityProvider for AlwaysAcceptIdentityProvider {
        fn get_identity(
            &self,
            _authorization: &AuthorizationHeader,
        ) -> Result<Option<Identity>, InternalError> {
            Ok(Some(Identity::Custom("identity".into())))
        }

        fn clone_box(&self) -> Box<dyn IdentityProvider> {
            Box::new(self.clone())
        }
    }
}
