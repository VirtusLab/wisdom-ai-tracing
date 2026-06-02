//! Real-network integration tests against `api.anthropic.com`. Both tests
//! are `#[ignore]` so they do not run in CI or on every `cargo test`. Run
//! locally before merging the proxy slice with:
//!
//! ```sh
//! ANTHROPIC_API_KEY=sk-ant-... \
//!   cargo test -p tracevault-server --test proxy_real_anthropic \
//!   -- --ignored --nocapture
//! ```
//!
//! These verify byte-fidelity against the real upstream that wiremock can
//! only approximate: TLS, HTTP/2, real anthropic-version negotiation, and
//! the actual SSE event vocabulary Anthropic emits today.

mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use tracevault_server::{api, repo_manager, AppState};

const MODEL: &str = "claude-haiku-4-5";

fn require_anthropic_key() -> String {
    // Walk up from the test binary's CWD looking for a .env file (workspace
    // root in most layouts). Silently no-op if no .env is present — the
    // env var may still be set externally.
    let _ = dotenvy::dotenv();
    std::env::var("ANTHROPIC_API_KEY").expect(
        "ANTHROPIC_API_KEY env var must be set to run #[ignore]-d real-Anthropic tests \
         (export it, or add it to .env at the workspace root)",
    )
}

async fn build_real_state(pool: &sqlx::PgPool, upstream_key: &str) -> (AppState, String) {
    let user = common::seed_user(pool).await;
    let session_token = common::seed_auth_session(pool, user).await;
    let encryption_key = common::fixture_encryption_key();

    // Forward to the real api.anthropic.com — exactly what we want here.
    tracevault_server::repo::credentials::CredentialRepo::upsert(
        pool,
        &encryption_key,
        user,
        "default",
        api::proxy::DEFAULT_ANTHROPIC_UPSTREAM_BASE,
        upstream_key,
        None,
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::ensure_default(pool, user, "default")
        .await
        .unwrap();

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: api::proxy::DEFAULT_ANTHROPIC_UPSTREAM_BASE.to_string(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    };
    (state, session_token)
}

fn build_proxy_app(state: AppState) -> Router {
    Router::new()
        .route(
            "/proxy/anthropic/{*path}",
            get(api::proxy::anthropic_proxy)
                .post(api::proxy::anthropic_proxy)
                .put(api::proxy::anthropic_proxy)
                .delete(api::proxy::anthropic_proxy),
        )
        .with_state(state)
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "hits api.anthropic.com — requires ANTHROPIC_API_KEY"]
async fn real_anthropic_non_streaming_messages(pool: sqlx::PgPool) {
    let upstream_key = require_anthropic_key();
    let (state, session) = build_real_state(&pool, &upstream_key).await;
    let app = build_proxy_app(state);

    let req_body = json!({
        "model": MODEL,
        "max_tokens": 16,
        "messages": [
            { "role": "user", "content": "Say hi in one word." }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &session)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = to_bytes(resp.into_body(), 16 * 1024 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap_or_else(|e| {
        panic!(
            "non-JSON body from upstream (status {status}): {:?}\n{}",
            e,
            String::from_utf8_lossy(&body_bytes)
        )
    });

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["type"], "message");
    assert!(
        body["content"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|c| c["text"].as_str())
            .is_some_and(|t| !t.is_empty()),
        "expected non-empty content[0].text; got {body}"
    );
    assert!(
        body["stop_reason"].is_string(),
        "expected stop_reason; got {body}"
    );
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "hits api.anthropic.com — requires ANTHROPIC_API_KEY"]
async fn real_anthropic_streaming_messages(pool: sqlx::PgPool) {
    let upstream_key = require_anthropic_key();
    let (state, session) = build_real_state(&pool, &upstream_key).await;
    let app = build_proxy_app(state);

    let req_body = json!({
        "model": MODEL,
        "max_tokens": 16,
        "stream": true,
        "messages": [
            { "role": "user", "content": "Count to 3." }
        ]
    });

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &session)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = to_bytes(resp.into_body(), 16 * 1024 * 1024).await.unwrap();
    let body_text = String::from_utf8_lossy(&body_bytes);

    assert_eq!(status, StatusCode::OK, "non-200 from upstream: {body_text}");
    assert!(
        body_text.contains("event: message_start"),
        "expected message_start SSE event in stream; got:\n{body_text}"
    );
    assert!(
        body_text.contains("event: content_block_delta"),
        "expected content_block_delta SSE event in stream; got:\n{body_text}"
    );
}
