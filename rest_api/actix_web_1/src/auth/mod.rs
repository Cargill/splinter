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

#[cfg(feature = "authorization")]
mod maintenance;
#[cfg(feature = "authorization")]
mod make_permission_resource;
mod middleware;
#[cfg(feature = "oauth")]
mod oauth;
#[cfg(feature = "rbac")]
mod rbac;
#[cfg(feature = "authorization")]
mod resource_provider;
mod transform;

#[cfg(feature = "authorization")]
pub use maintenance::{
    make_maintenance_resource, AUTHORIZATION_MAINTENANCE_READ_PERMISSION,
    AUTHORIZATION_MAINTENANCE_WRITE_PERMISSION,
};
pub use middleware::AuthorizationMiddleware;
#[cfg(feature = "oauth")]
pub use oauth::{
    callback::make_callback_route, list_users::make_oauth_list_users_resource,
    login::make_login_route, logout::make_logout_route, resource_provider::OAuthResourceProvider,
};
#[cfg(feature = "rbac")]
pub use rbac::RoleBasedAuthorizationResourceProvider;
#[cfg(feature = "authorization")]
pub use resource_provider::{
    AuthorizationResourceProvider, AUTHORIZATION_PERMISSIONS_READ_PERMISSION,
};
pub use transform::Authorization;

#[cfg(test)]
mod tests {
    use super::*;

    use actix_web::http::Method as ActixMethod;
    use actix_web::{
        dev::Service, http::HeaderValue, http::StatusCode, test, web, App, HttpRequest,
        HttpResponse,
    };
    use splinter::error::InternalError;
    use splinter_rest_api_common::auth::{AuthorizationHeader, Identity, IdentityProvider};
    #[cfg(feature = "authorization")]
    use splinter_rest_api_common::auth::{Permission, PermissionMap};

    #[cfg(feature = "authorization")]
    use crate::framework::Method;

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
            .route("/", web::get().to(HttpResponse::Ok));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::default();
            permission_map.add_permission(Method::Get, "/", Permission::AllowAuthenticated);
            app.data(permission_map)
        };

        let mut service = test::init_service(app);

        let req = test::TestRequest::with_uri("/").to_request();
        let resp = test::block_on(service.call(req)).unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Verifies that the authorization middleware returns a `404 Not Found` response when the
    /// `authorize` function returns an "unknown endpoint" result.
    #[test]
    fn auth_middleware_not_found() {
        let app = App::new()
            .wrap(Authorization::new(
                vec![Box::new(AlwaysAcceptIdentityProvider)],
                #[cfg(feature = "authorization")]
                vec![],
            ))
            .route("/", web::get().to(HttpResponse::Ok));

        #[cfg(feature = "authorization")]
        let app = {
            let mut permission_map = PermissionMap::default();
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
            let mut permission_map = PermissionMap::default();
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
