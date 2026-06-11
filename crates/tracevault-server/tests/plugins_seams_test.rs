mod common;

use axum::http::StatusCode;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt; // for oneshot
use tracevault_server::plugins::{BackgroundTask, Plugins, RoutePlugin, Schedule};
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

struct FlagTask(Arc<std::sync::atomic::AtomicBool>);
#[async_trait::async_trait]
impl BackgroundTask for FlagTask {
    fn name(&self) -> &'static str {
        "flag"
    }
    fn schedule(&self) -> Schedule {
        Schedule::Startup
    }
    async fn run(&self, _state: AppState) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn startup_task_runs(pool: PgPool) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let flag = Arc::new(AtomicBool::new(false));
    let mut plugins = Plugins::default();
    plugins.tasks.push(Arc::new(FlagTask(flag.clone())));

    let state = common::test_state_with_plugins(pool, Arc::new(plugins));
    tracevault_server::spawn_plugin_tasks(&state);

    for _ in 0..100 {
        if flag.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        flag.load(Ordering::SeqCst),
        "Startup background task should have run"
    );
}
