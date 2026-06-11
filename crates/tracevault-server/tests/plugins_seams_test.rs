mod common;

use axum::http::StatusCode;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt; // for oneshot
use tracevault_server::plugins::{Plugins, RoutePlugin};
use tracevault_server::AppState;

struct PingPlugin;
impl RoutePlugin for PingPlugin {
    fn id(&self) -> &'static str {
        "ping"
    }
    fn router(&self) -> axum::Router<AppState> {
        axum::Router::new().route("/api/v1/ext/ping", axum::routing::get(|| async { "pong" }))
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn route_plugin_is_mounted(pool: PgPool) {
    let mut plugins = Plugins::default();
    plugins.routes.push(Arc::new(PingPlugin));
    plugins.capabilities.insert("ping".to_string());

    let state = common::test_state_with_plugins(pool, Arc::new(plugins));
    let app = tracevault_server::build_router(state);

    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/ext/ping")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cap = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/capabilities")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cap.status(), StatusCode::OK);

    // The registered capability is actually advertised.
    let body = axum::body::to_bytes(cap.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let caps = json["capabilities"].as_array().expect("capabilities array");
    assert!(
        caps.iter().any(|c| c == "ping"),
        "expected 'ping' capability to be advertised, got {json}"
    );
}
