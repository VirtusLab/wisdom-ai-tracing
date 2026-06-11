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

struct RecordingHook(Arc<std::sync::Mutex<Vec<uuid::Uuid>>>);
#[async_trait::async_trait]
impl tracevault_server::plugins::IngestHook for RecordingHook {
    async fn on_session_finalized(
        &self,
        _state: &AppState,
        ctx: &tracevault_server::plugins::SessionFinalizedContext,
    ) {
        self.0.lock().unwrap().push(ctx.session_db_id);
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn ingest_hook_fires_on_session_end(pool: PgPool) {
    use tracevault_core::streaming::{StreamEventRequest, StreamEventType};
    use tracevault_server::service::stream::StreamService;

    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let seen = Arc::new(std::sync::Mutex::new(Vec::<uuid::Uuid>::new()));
    let mut plugins = Plugins::default();
    plugins
        .ingest_hooks
        .push(Arc::new(RecordingHook(seen.clone())));
    let state = common::test_state_with_plugins(pool.clone(), Arc::new(plugins));

    let session_id = "sess-ingest-hook-1";

    // 1) Create the session via a Transcript event.
    let transcript = StreamEventRequest {
        protocol_version: 2,
        tool: Some("claude-code".to_string()),
        event_type: StreamEventType::Transcript,
        session_id: session_id.to_string(),
        timestamp: chrono::Utc::now(),
        hook_event_name: None,
        tool_name: None,
        tool_use_id: None,
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        event_index: None,
        transcript_lines: Some(vec![serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg_ingest_hook",
                "model": "claude-sonnet-4-6",
                "usage": { "input_tokens": 100, "output_tokens": 10 }
            }
        })]),
        transcript_offset: Some(0),
        model: Some("claude-sonnet-4-6".to_string()),
        cwd: Some("/project".to_string()),
        final_stats: None,
    };
    StreamService::process(&state, org_id, repo_id, user_id, transcript)
        .await
        .unwrap();

    // 2) Finalize it via a SessionEnd event with final_stats.
    let session_end = StreamEventRequest {
        protocol_version: 2,
        tool: Some("claude-code".to_string()),
        event_type: StreamEventType::SessionEnd,
        session_id: session_id.to_string(),
        timestamp: chrono::Utc::now(),
        hook_event_name: None,
        tool_name: None,
        tool_use_id: None,
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        event_index: None,
        transcript_lines: None,
        transcript_offset: None,
        model: Some("claude-sonnet-4-6".to_string()),
        cwd: Some("/project".to_string()),
        final_stats: Some(tracevault_core::streaming::SessionFinalStats {
            duration_ms: Some(5000),
            total_tokens: Some(110),
            input_tokens: Some(100),
            output_tokens: Some(10),
            cache_read_tokens: None,
            cache_write_tokens: None,
            user_messages: Some(1),
            assistant_messages: Some(1),
            total_tool_calls: Some(0),
        }),
    };
    StreamService::process(&state, org_id, repo_id, user_id, session_end)
        .await
        .unwrap();

    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "hook should fire exactly once on finalize"
    );
    assert_ne!(seen.lock().unwrap()[0], uuid::Uuid::nil());
}

struct OneMetric;
#[async_trait::async_trait]
impl tracevault_server::plugins::MetricContributor for OneMetric {
    fn slot(&self) -> &'static str {
        "session.detail"
    }
    async fn contribute(
        &self,
        _state: &AppState,
        _ctx: &tracevault_server::plugins::SessionMetricContext,
    ) -> Vec<tracevault_server::plugins::Metric> {
        vec![tracevault_server::plugins::Metric {
            key: "demo".into(),
            label: "Demo".into(),
            value: serde_json::json!(1),
            format: "count".into(),
        }]
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn session_detail_includes_contributor_metrics(pool: PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let session_id = common::seed_session(&pool, org_id, repo_id, user_id).await;
    let token = common::seed_auth_session(&pool, user_id).await;

    // The `{slug}` path param is matched against orgs.name (case-insensitive);
    // there is no separate slug column.
    let slug = sqlx::query_scalar::<_, String>("SELECT name FROM orgs WHERE id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut plugins = Plugins::default();
    plugins.metrics.push(Arc::new(OneMetric));
    let state = common::test_state_with_plugins(pool.clone(), Arc::new(plugins));
    let app = tracevault_server::build_router(state);

    let uri = format!("/api/v1/orgs/{slug}/analytics/sessions/{session_id}/detail");
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .method("GET")
                .uri(&uri)
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let metrics = json["metrics"].as_array().expect("metrics array");
    assert!(
        metrics.iter().any(|m| m["key"] == "demo"),
        "expected a metric with key 'demo', got {json}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn empty_plugins_change_nothing(pool: PgPool) {
    // With the default (empty) Plugins, /api/v1/capabilities returns an empty list.
    let state = common::test_state_with_plugins(pool, Arc::new(Plugins::default()));
    let app = tracevault_server::build_router(state);

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v1/capabilities")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        json["capabilities"]
            .as_array()
            .expect("capabilities array")
            .len(),
        0,
        "OSS default must advertise zero capabilities"
    );
}
