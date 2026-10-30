//! Per-launch bearer token, Host-header check, and CORS for the local API.
//!
//! The token is generated once per launch, held only in memory, and handed to the webview
//! through the `backend_info` Tauri command. It is never logged.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use subtle::ConstantTimeEq;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// What the middleware checks on every request.
#[derive(Clone)]
pub struct AuthState {
    token: Arc<str>,
    host: Arc<str>,
}

// Deliberately not `Debug`-derived, so the token can never end up in a log line.
impl std::fmt::Debug for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthState").field("host", &self.host).finish_non_exhaustive()
    }
}

impl AuthState {
    pub fn new(token: String, port: u16) -> Self {
        Self { token: token.into(), host: format!("127.0.0.1:{port}").into() }
    }

    fn token_matches(&self, presented: &str) -> bool {
        self.token.as_bytes().ct_eq(presented.as_bytes()).into()
    }
}

/// 32 random bytes, hex encoded.
pub fn generate_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(hex::encode(bytes))
}

fn unauthorized() -> Response {
    StatusCode::UNAUTHORIZED.into_response()
}

/// Rejects (401, empty body) any request without the right `Host` and bearer token.
pub async fn require_auth(State(auth): State<AuthState>, request: Request, next: Next) -> Response {
    let headers = request.headers();

    let host_ok = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|host| host.eq_ignore_ascii_case(&auth.host));
    if !host_ok {
        return unauthorized();
    }

    let token_ok = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|presented| auth.token_matches(presented));
    if !token_ok {
        return unauthorized();
    }

    next.run(request).await
}

/// Exact origins allowed to call the API from the webview.
pub fn allowed_origins(debug_build: bool) -> Vec<HeaderValue> {
    let mut origins = vec!["http://tauri.localhost", "tauri://localhost"];
    if debug_build {
        origins.extend(["http://localhost:5173", "http://127.0.0.1:5173"]);
    }
    origins.into_iter().map(HeaderValue::from_static).collect()
}

pub fn cors_layer(debug_build: bool) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins(debug_build)))
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
        ])
}

/// Wraps every route in auth, with CORS outside it so preflight requests (which carry no
/// `Authorization` header) are answered before auth runs.
pub fn secure(router: Router, auth: AuthState, debug_build: bool) -> Router {
    router
        .layer(middleware::from_fn_with_state(auth, require_auth))
        .layer(cors_layer(debug_build))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const TOKEN: &str = "0123456789abcdef";
    const PORT: u16 = 40123;

    fn app() -> Router {
        let router = Router::new()
            .route("/api/health", get(|| async { "ok" }))
            .route("/api/keys/status", get(|| async { "keys" }));
        secure(router, AuthState::new(TOKEN.to_string(), PORT), false)
    }

    fn request(host: Option<&str>, auth: Option<&str>, origin: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().uri("/api/health");
        if let Some(host) = host {
            builder = builder.header("host", host);
        }
        if let Some(auth) = auth {
            builder = builder.header("authorization", auth);
        }
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn status_and_body(request: Request<Body>) -> (StatusCode, String) {
        let response = app().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn valid_token_and_host_pass() {
        let (status, body) =
            status_and_body(request(Some("127.0.0.1:40123"), Some("Bearer 0123456789abcdef"), None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "ok");
    }

    #[tokio::test]
    async fn missing_token_is_401_with_empty_body() {
        let (status, body) = status_and_body(request(Some("127.0.0.1:40123"), None, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn wrong_token_wrong_scheme_and_prefix_tokens_are_401() {
        for auth in ["Bearer nope", "Bearer ", "Bearer 0123456789abcde", "Bearer 0123456789abcdefX", "Basic 0123456789abcdef", "0123456789abcdef"] {
            let (status, body) = status_and_body(request(Some("127.0.0.1:40123"), Some(auth), None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{auth}");
            assert!(body.is_empty());
        }
    }

    #[tokio::test]
    async fn rebinding_hosts_are_rejected_even_with_the_right_token() {
        for host in [None, Some("evil.example:40123"), Some("localhost:40123"), Some("127.0.0.1:1"), Some("127.0.0.1")] {
            let (status, body) =
                status_and_body(request(host, Some("Bearer 0123456789abcdef"), None)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{host:?}");
            assert!(body.is_empty());
        }
    }

    #[tokio::test]
    async fn every_route_is_protected() {
        let request = Request::builder()
            .uri("/api/keys/status")
            .header("host", "127.0.0.1:40123")
            .body(Body::empty())
            .unwrap();
        let (status, _) = status_and_body(request).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn cors_allows_only_exact_webview_origins() {
        let allowed = app()
            .oneshot(request(Some("127.0.0.1:40123"), Some("Bearer 0123456789abcdef"), Some("http://tauri.localhost")))
            .await
            .unwrap();
        assert_eq!(
            allowed.headers().get("access-control-allow-origin").unwrap(),
            "http://tauri.localhost"
        );

        for origin in ["https://example.com", "http://tauri.localhost.evil.com", "http://localhost:5173"] {
            let denied = app()
                .oneshot(request(Some("127.0.0.1:40123"), Some("Bearer 0123456789abcdef"), Some(origin)))
                .await
                .unwrap();
            assert!(denied.headers().get("access-control-allow-origin").is_none(), "{origin}");
        }
    }

    #[tokio::test]
    async fn preflight_is_answered_without_a_token_and_only_for_allowed_origins() {
        let preflight = |origin: &str| {
            Request::builder()
                .method("OPTIONS")
                .uri("/api/health")
                .header("host", "127.0.0.1:40123")
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "authorization,content-type")
                .body(Body::empty())
                .unwrap()
        };
        let ok = app().oneshot(preflight("http://tauri.localhost")).await.unwrap();
        assert!(ok.status().is_success());
        assert_eq!(ok.headers().get("access-control-allow-origin").unwrap(), "http://tauri.localhost");

        let bad = app().oneshot(preflight("https://example.com")).await.unwrap();
        assert!(bad.headers().get("access-control-allow-origin").is_none());
    }

    #[test]
    fn debug_origins_only_exist_in_debug_builds() {
        let release: Vec<_> = allowed_origins(false).iter().map(|v| v.to_str().unwrap().to_string()).collect();
        assert_eq!(release, vec!["http://tauri.localhost", "tauri://localhost"]);
        assert!(allowed_origins(true).iter().any(|v| v == "http://localhost:5173"));
    }

    #[test]
    fn tokens_are_random_hex_and_never_debug_printed() {
        let a = generate_token().unwrap();
        let b = generate_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        let debug = format!("{:?}", AuthState::new(a.clone(), 1));
        assert!(!debug.contains(&a));
    }
}
