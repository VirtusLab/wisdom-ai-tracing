//! End-to-end integration tests for the transparent Anthropic proxy
//! (issue softwaremill/tracevault#207, parent #181).
//!
//! Spins up:
//!   * a real Postgres pool via `sqlx::test` with all migrations applied
//!   * a `wiremock::MockServer` standing in for `api.anthropic.com`
//!   * an in-process `axum::Router` carrying the proxy handler and the
//!     `/api/v1/me/anthropic-key` endpoints, with `AppState` pointing at the
//!     two above
//!
//! Verifies the full request/response lifecycle including auth failures,
//! header forwarding, byte-level streaming, error envelope shape, and the
//! deferred-from-T02 `/me/anthropic-key` HTTP lifecycle.

mod common;

use axum::{
    body::{to_bytes, Body, Bytes},
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use tracevault_server::{api, repo_manager, AppState};
use uuid::Uuid;
use wiremock::{
    matchers::{header, method, path as wm_path},
    Mock, MockServer, ResponseTemplate,
};

// --- Test harness ---------------------------------------------------------

struct Harness {
    app: Router,
    upstream: MockServer,
    /// The same PgPool wired into the AppState — useful for tests that
    /// seed extra users or keys beyond what the harness creates.
    pool: sqlx::PgPool,
    /// Raw TV session token to send in x-api-key. Test user has a stored
    /// Anthropic key of `sk-ant-test-upstream-key`.
    user_session_token: String,
    /// Raw TV session token belonging to a user with NO Anthropic key stored.
    user_no_key_session_token: String,
    /// Raw org API key hash — sent in x-api-key should be rejected with the
    /// "use a user session token" error.
    org_api_key_token: String,
}

async fn build_harness(pool: sqlx::PgPool) -> Harness {
    let upstream = MockServer::start().await;

    // Seed: org, two users, two sessions, one credential + default routing
    // rule, one org api_key.
    let org_id = common::seed_org(&pool).await;
    let user_with_key = common::seed_user(&pool).await;
    let user_without_key = common::seed_user(&pool).await;
    let user_session_token = common::seed_auth_session(&pool, user_with_key).await;
    let user_no_key_session_token = common::seed_auth_session(&pool, user_without_key).await;

    // Org api_key. `seed_api_key` returns (id, hash) — we want the raw token
    // form, but the codebase stores only the hash. For test purposes we can
    // insert a known raw+hash pair directly.
    let raw_org_token = format!("tv_ak_{}", Uuid::new_v4());
    let org_token_hash = tracevault_server::auth::sha256_hex(&raw_org_token);
    sqlx::query("INSERT INTO api_keys (org_id, key_hash, name) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(&org_token_hash)
        .bind("test-org-key")
        .execute(&pool)
        .await
        .unwrap();

    let encryption_key = common::fixture_encryption_key();

    tracevault_server::repo::credentials::CredentialRepo::upsert(
        &pool,
        &encryption_key,
        user_with_key,
        "default",
        &upstream.uri(),
        "sk-ant-test-upstream-key",
        None,
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::ensure_default(&pool, user_with_key, "default")
        .await
        .unwrap();

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: upstream.uri(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    };

    let app = Router::new()
        .route(
            "/proxy/anthropic/{*path}",
            get(api::proxy::anthropic_proxy)
                .post(api::proxy::anthropic_proxy)
                .put(api::proxy::anthropic_proxy)
                .delete(api::proxy::anthropic_proxy),
        )
        // Mirror the production body limit so integration tests exercise the
        // same envelope as live traffic.
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .route(
            "/api/v1/me/anthropic-key",
            get(api::me::get_anthropic_key_status)
                .put(api::me::put_anthropic_key)
                .delete(api::me::delete_anthropic_key),
        )
        .with_state(state);

    Harness {
        app,
        upstream,
        pool,
        user_session_token,
        user_no_key_session_token,
        org_api_key_token: raw_org_token,
    }
}

/// Build a harness with explicit concurrency caps. The default `build_harness`
/// uses `max_concurrent = 8` (DB default) and no global cap, which works for
/// every test that does not exercise the cap. The cap-specific tests need
/// tighter knobs:
///   * `per_credential_cap`: overrides the seeded user's `max_concurrent`.
///   * `global_cap`: when `Some(n)`, the AppState carries a global
///     `Semaphore::new(n)`; when `None`, the global cap is disabled.
async fn build_harness_with_caps(
    pool: sqlx::PgPool,
    per_credential_cap: i32,
    global_cap: Option<usize>,
) -> Harness {
    let upstream = MockServer::start().await;

    let org_id = common::seed_org(&pool).await;
    let user_with_key = common::seed_user(&pool).await;
    let user_without_key = common::seed_user(&pool).await;
    let user_session_token = common::seed_auth_session(&pool, user_with_key).await;
    let user_no_key_session_token = common::seed_auth_session(&pool, user_without_key).await;

    let raw_org_token = format!("tv_ak_{}", Uuid::new_v4());
    let org_token_hash = tracevault_server::auth::sha256_hex(&raw_org_token);
    sqlx::query("INSERT INTO api_keys (org_id, key_hash, name) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(&org_token_hash)
        .bind("test-org-key")
        .execute(&pool)
        .await
        .unwrap();

    let encryption_key = common::fixture_encryption_key();
    tracevault_server::repo::credentials::CredentialRepo::upsert(
        &pool,
        &encryption_key,
        user_with_key,
        "default",
        &upstream.uri(),
        "sk-ant-test-upstream-key",
        Some(per_credential_cap),
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::ensure_default(&pool, user_with_key, "default")
        .await
        .unwrap();

    let proxy_global_semaphore =
        global_cap.map(|n| std::sync::Arc::new(tokio::sync::Semaphore::new(n)));

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: upstream.uri(),
        proxy_global_semaphore,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    };

    let app = Router::new()
        .route(
            "/proxy/anthropic/{*path}",
            get(api::proxy::anthropic_proxy)
                .post(api::proxy::anthropic_proxy)
                .put(api::proxy::anthropic_proxy)
                .delete(api::proxy::anthropic_proxy),
        )
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .route(
            "/api/v1/me/anthropic-key",
            get(api::me::get_anthropic_key_status)
                .put(api::me::put_anthropic_key)
                .delete(api::me::delete_anthropic_key),
        )
        .with_state(state);

    Harness {
        app,
        upstream,
        pool,
        user_session_token,
        user_no_key_session_token,
        org_api_key_token: raw_org_token,
    }
}

async fn read_body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, 16 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn read_body_to_bytes(body: Body) -> Bytes {
    to_bytes(body, 16 * 1024 * 1024).await.unwrap()
}

// --- Proxy: success / passthrough -----------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn proxy_forwards_non_streaming_request_and_returns_upstream_json(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let upstream_payload = json!({
        "id": "msg_01abc",
        "type": "message",
        "role": "assistant",
        "content": [{"type": "text", "text": "hi"}],
        "stop_reason": "end_turn",
    });

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        // Critical: upstream must see the upstream-Anthropic key, not the
        // TV session token. This is the central security property of the
        // proxy's auth-swap.
        .and(header("x-api-key", "sk-ant-test-upstream-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("anthropic-request-id", "req_abc123")
                .set_body_json(&upstream_payload),
        )
        .expect(1)
        .mount(&h.upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "model": "claude-haiku", "max_tokens": 1 })).unwrap(),
        ))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("anthropic-request-id")
            .and_then(|v| v.to_str().ok()),
        Some("req_abc123"),
        "anthropic-request-id must be forwarded for client correlation"
    );

    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body, upstream_payload);
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_streams_sse_response_byte_for_byte(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    // Three SSE events in the format Anthropic emits. We assert the
    // downstream client sees the exact same bytes back, verifying that
    // bytes_stream() passthrough does not parse or re-frame the SSE body.
    let sse_payload = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                // set_body_raw lets us set both the bytes and the
                // content-type explicitly (set_body_string defaults to
                // text/plain and ignores insert_header overrides).
                .set_body_raw(sse_payload.as_bytes().to_vec(), "text/event-stream"),
        )
        .expect(1)
        .mount(&h.upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"stream":true}"#))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream"),
    );

    let downstream_bytes = read_body_to_bytes(resp.into_body()).await;
    assert_eq!(
        downstream_bytes.as_ref(),
        sse_payload.as_bytes(),
        "SSE payload must be forwarded byte-for-byte"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_passes_upstream_4xx_through_verbatim(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let upstream_error = json!({
        "type": "error",
        "error": {
            "type": "invalid_request_error",
            "message": "max_tokens is required"
        }
    });

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_json(&upstream_error))
        .expect(1)
        .mount(&h.upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body, upstream_error);
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_passes_upstream_5xx_through_verbatim(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let upstream_error = json!({
        "type": "error",
        "error": {
            "type": "overloaded_error",
            "message": "Overloaded"
        }
    });

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(ResponseTemplate::new(529).set_body_json(&upstream_error))
        .expect(1)
        .mount(&h.upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 529);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body, upstream_error);
}

// --- Proxy: auth + missing-key error envelope -----------------------------

async fn assert_anthropic_error_envelope(
    resp: axum::response::Response,
    expected_status: StatusCode,
    expected_kind: &str,
    expected_message_contains: &str,
) {
    assert_eq!(resp.status(), expected_status);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["type"], "error", "envelope must use Anthropic shape");
    assert_eq!(
        body["error"]["type"], expected_kind,
        "error.type must use Anthropic vocabulary"
    );
    let msg = body["error"]["message"]
        .as_str()
        .expect("error.message must be a string");
    assert!(
        msg.contains(expected_message_contains),
        "error.message {msg:?} should contain {expected_message_contains:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_missing_x_api_key(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .body(Body::empty())
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_anthropic_error_envelope(
        resp,
        StatusCode::UNAUTHORIZED,
        "authentication_error",
        "Missing x-api-key",
    )
    .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_invalid_token(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", "not-a-real-token")
        .body(Body::empty())
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_anthropic_error_envelope(
        resp,
        StatusCode::UNAUTHORIZED,
        "authentication_error",
        "Invalid or expired",
    )
    .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_org_api_key_with_specific_message(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.org_api_key_token)
        .body(Body::empty())
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_anthropic_error_envelope(
        resp,
        StatusCode::UNAUTHORIZED,
        "authentication_error",
        "org API key",
    )
    .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_returns_502_when_upstream_unreachable(pool: sqlx::PgPool) {
    // Build a state pointing at a port nothing is listening on — the
    // outgoing reqwest must fail at the TCP layer and the handler must
    // surface that as a 502 api_error (NOT a 500). Port 1 is reserved
    // and effectively never has a listener on it; reqwest hits ECONNREFUSED
    // immediately.
    let user = common::seed_user(&pool).await;
    let session = common::seed_auth_session(&pool, user).await;
    let encryption_key = common::fixture_encryption_key();
    // The credential's own base_url is the unreachable address — that is what
    // the proxy forwards to now (not the AppState field).
    tracevault_server::repo::credentials::CredentialRepo::upsert(
        &pool,
        &encryption_key,
        user,
        "default",
        "http://127.0.0.1:1",
        "sk-ant-doesnt-matter",
        None,
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::ensure_default(&pool, user, "default")
        .await
        .unwrap();

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::builder()
            // Tight timeout so we don't sit for 30s on the OS default.
            .connect_timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: "http://127.0.0.1:1".to_string(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    };

    let app = Router::new()
        .route(
            "/proxy/anthropic/{*path}",
            get(api::proxy::anthropic_proxy)
                .post(api::proxy::anthropic_proxy)
                .put(api::proxy::anthropic_proxy)
                .delete(api::proxy::anthropic_proxy),
        )
        .with_state(state);

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &session)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_anthropic_error_envelope(
        resp,
        StatusCode::BAD_GATEWAY,
        "api_error",
        "Upstream Anthropic API unreachable",
    )
    .await;
}

#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_user_with_no_key_configured(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_no_key_session_token)
        .body(Body::empty())
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_anthropic_error_envelope(
        resp,
        StatusCode::UNAUTHORIZED,
        "authentication_error",
        "No Anthropic API key configured",
    )
    .await;
}

// --- Proxy: header allow-list assertion -----------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn proxy_strips_forbidden_request_headers(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        // Upstream should see anthropic-version and content-type forwarded.
        .and(header("anthropic-version", "2023-06-01"))
        .and(header("content-type", "application/json"))
        // Upstream must see x-api-key swapped to the upstream key, NOT
        // the TV session token.
        .and(header("x-api-key", "sk-ant-test-upstream-key"))
        // Custom matcher: any header NOT on our allow-list must be absent.
        .and(wiremock::matchers::header_exists("anthropic-version"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&h.upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        // These three headers must NOT reach upstream:
        .header("cookie", "session=must-not-leak")
        .header("authorization", "Bearer must-not-leak")
        .header("x-forwarded-for", "192.0.2.1")
        .header("x-internal-secret", "must-not-leak")
        .body(Body::from("{}"))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify the four forbidden headers really did not make it upstream.
    // wiremock records received requests; we can inspect them after the
    // call to assert absence.
    let recv = h.upstream.received_requests().await.unwrap();
    assert_eq!(recv.len(), 1);
    let upstream_req = &recv[0];
    for forbidden in [
        "cookie",
        "authorization",
        "x-forwarded-for",
        "x-internal-secret",
    ] {
        assert!(
            !upstream_req.headers.contains_key(forbidden),
            "header {forbidden} must not be forwarded to upstream"
        );
    }
}

// --- Proxy: body size + path-traversal hardening --------------------------

/// A request body comfortably larger than Axum's 2 MB `Bytes` default must
/// reach upstream when the proxy router raises `DefaultBodyLimit`. This
/// catches regressions where the body cap is removed or shrunk back to the
/// default and silently breaks vision / long-context Anthropic requests.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_accepts_large_body_within_raised_limit(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .and(header("x-api-key", "sk-ant-test-upstream-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&h.upstream)
        .await;

    // 4 MB body — 2× Axum's default cap, well within our 32 MB limit.
    let payload = vec![b'a'; 4 * 1024 * 1024];

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/octet-stream")
        .body(Body::from(payload))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "4 MB body must pass through with the raised body limit"
    );
}

/// `..` segments in the proxy path must be rejected at the router entry
/// with an Anthropic-shaped error envelope. Belt-and-braces against future
/// reconfiguration of a credential `base_url` to a path-prefixed URL.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_path_traversal_segments(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;

    // No mock mounted — the request must never reach upstream.
    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages/..%2F..%2Fadmin")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "api_error");

    // And confirm upstream really was never called.
    let recv = h.upstream.received_requests().await.unwrap();
    assert!(
        recv.is_empty(),
        "upstream must not receive a `..`-bearing path"
    );
}

// --- Proxy: per-model routing (step 2) ------------------------------------

/// Seed a second credential (`fast` → `fast_upstream.uri()`) and a model rule
/// `claude-haiku → fast (provider_model = claude-3-5-haiku-latest)` for the
/// harness user, returning the second MockServer. The harness already wires
/// the `default` credential at `h.upstream` + a default routing rule, so this
/// gives us a two-target routing setup.
async fn seed_model_routing(h: &Harness, user_token_owner: Uuid) -> MockServer {
    let fast_upstream = MockServer::start().await;
    let encryption_key = common::fixture_encryption_key();
    tracevault_server::repo::credentials::CredentialRepo::upsert(
        &h.pool,
        &encryption_key,
        user_token_owner,
        "fast",
        &fast_upstream.uri(),
        "sk-ant-fast-upstream-key",
        Some(8),
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::upsert_rule(
        &h.pool,
        user_token_owner,
        Some("claude-haiku"),
        "fast",
        Some("claude-3-5-haiku-latest"),
    )
    .await
    .unwrap();
    fast_upstream
}

/// Resolve the harness `user_session_token` back to its user_id (the harness
/// owns the raw token but not the id, so look it up by its hash).
async fn user_id_for_token(pool: &sqlx::PgPool, token: &str) -> Uuid {
    let token_hash = tracevault_server::auth::sha256_hex(token);
    sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM auth_sessions WHERE token_hash = $1")
        .bind(token_hash)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A request whose `model` matches a routing rule must be forwarded to the
/// matched credential's `base_url` (mock B / `fast`) — NOT the default
/// (mock A) — and the body's `model` must be rewritten to the rule's
/// `provider_model` before it reaches upstream.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_routes_matching_model_to_matched_credential_and_rewrites_model(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;
    let user_id = user_id_for_token(&h.pool, &h.user_session_token).await;
    let fast_upstream = seed_model_routing(&h, user_id).await;

    // Default (mock A): must NOT be hit for the routed model.
    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&h.upstream)
        .await;

    // Matched credential (mock B / `fast`): must be hit with the upstream
    // `fast` key and the rewritten model.
    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .and(header("x-api-key", "sk-ant-fast-upstream-key"))
        .and(wiremock::matchers::body_json(json!({
            "model": "claude-3-5-haiku-latest",
            "max_tokens": 1
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&fast_upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "model": "claude-haiku", "max_tokens": 1 })).unwrap(),
        ))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Drain so the streaming permit drops cleanly.
    let _ = read_body_to_bytes(resp.into_body()).await;
}

/// A request whose `model` matches NO rule must fall back to the default
/// credential (mock A) with the body's `model` left unchanged.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_routes_unmatched_model_to_default_credential_unchanged(pool: sqlx::PgPool) {
    let h = build_harness(pool).await;
    let user_id = user_id_for_token(&h.pool, &h.user_session_token).await;
    let fast_upstream = seed_model_routing(&h, user_id).await;

    // Default (mock A): hit with the default key and the model UNCHANGED.
    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .and(header("x-api-key", "sk-ant-test-upstream-key"))
        .and(wiremock::matchers::body_json(json!({
            "model": "claude-opus",
            "max_tokens": 1
        })))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&h.upstream)
        .await;

    // Matched credential (mock B / `fast`): must NOT be hit.
    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&fast_upstream)
        .await;

    let req = Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", &h.user_session_token)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({ "model": "claude-opus", "max_tokens": 1 })).unwrap(),
        ))
        .unwrap();

    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let _ = read_body_to_bytes(resp.into_body()).await;
}

// --- Proxy: per-credential and global concurrency caps (#210) -------------

use std::time::Duration;

/// Build a request to the proxy with the standard headers + a marker query
/// so we can tell wiremock-served requests apart.
fn proxy_request(token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/proxy/anthropic/v1/messages")
        .header("x-api-key", token)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"model":"claude-haiku","max_tokens":1}"#))
        .unwrap()
}

/// Per-credential cap exceeded: with `max_concurrent = 2`, two in-flight
/// requests succeed (eventually), but the third in-flight request returns
/// 429 / `overloaded_error` with `reason = per_credential_cap`.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_when_per_credential_cap_exceeded(pool: sqlx::PgPool) {
    let h = build_harness_with_caps(pool, 2, None).await;

    // Upstream sits on each request for 2s so the in-flight permits are
    // really held when we issue the rejecting request.
    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{}")
                .set_delay(Duration::from_secs(2)),
        )
        .mount(&h.upstream)
        .await;

    let app = h.app.clone();
    let token = h.user_session_token.clone();

    // Spawn two slow-but-eventually-OK requests so the per-credential
    // semaphore is at full capacity. We deliberately do not await these;
    // they keep the permits held until the wiremock delay elapses or the
    // task is dropped at end-of-test.
    let _h1 = tokio::spawn({
        let app = app.clone();
        let token = token.clone();
        async move { app.oneshot(proxy_request(&token)).await }
    });
    let _h2 = tokio::spawn({
        let app = app.clone();
        let token = token.clone();
        async move { app.oneshot(proxy_request(&token)).await }
    });

    // Brief yield so both spawned tasks reach the acquire/upstream-send
    // boundary before we issue the rejecting request.
    tokio::time::sleep(Duration::from_millis(150)).await;

    let resp = app
        .clone()
        .oneshot(proxy_request(&token))
        .await
        .expect("third request should respond, not panic");
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "third in-flight request must hit the per-credential cap"
    );
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["type"], "error");
    assert_eq!(body["error"]["type"], "overloaded_error");
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("cap: 2"),
        "error message should name the configured cap; got: {msg}"
    );
}

/// After the in-flight requests complete and release their permits, a
/// new request must succeed. Guards against the bug where permits leak.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_frees_permit_when_request_completes(pool: sqlx::PgPool) {
    let h = build_harness_with_caps(pool, 1, None).await;

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{}")
                .set_delay(Duration::from_millis(100)),
        )
        .mount(&h.upstream)
        .await;

    // Cap is 1. First request must succeed.
    let r1 = h
        .app
        .clone()
        .oneshot(proxy_request(&h.user_session_token))
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);
    // Drain the body so the streaming permit is dropped — otherwise the
    // permit stays held until r1.into_body() is consumed.
    let _ = read_body_to_bytes(r1.into_body()).await;

    // Second request, sequential, after the first completes: must succeed.
    // If the permit leaked we'd get 429 here instead.
    let r2 = h
        .app
        .clone()
        .oneshot(proxy_request(&h.user_session_token))
        .await
        .unwrap();
    assert_eq!(
        r2.status(),
        StatusCode::OK,
        "second sequential request must succeed once the first releases its permit"
    );
}

/// Global cap exceeded: with `Semaphore::new(1)`, one in-flight request
/// from any user holds the only global slot; a request from a *different*
/// user must be rejected with `reason = global_cap`.
#[sqlx::test(migrations = "./migrations")]
async fn proxy_rejects_when_global_cap_exceeded(pool: sqlx::PgPool) {
    let h = build_harness_with_caps(pool, 8, Some(1)).await;

    // Seed a second user + session with their own Anthropic key so we can
    // prove the cap is global (cross-user), not per-credential.
    let second_user = common::seed_user(&h.pool).await;
    let second_token = common::seed_auth_session(&h.pool, second_user).await;
    let encryption_key = common::fixture_encryption_key();
    tracevault_server::repo::credentials::CredentialRepo::upsert(
        &h.pool,
        &encryption_key,
        second_user,
        "default",
        &h.upstream.uri(),
        "sk-ant-second-upstream-key",
        Some(8),
    )
    .await
    .unwrap();
    tracevault_server::repo::routing::RoutingRepo::ensure_default(&h.pool, second_user, "default")
        .await
        .unwrap();

    Mock::given(method("POST"))
        .and(wm_path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{}")
                .set_delay(Duration::from_secs(2)),
        )
        .mount(&h.upstream)
        .await;

    let app = h.app.clone();

    // User 1 holds the only global slot.
    let token1 = h.user_session_token.clone();
    let _holder = tokio::spawn({
        let app = app.clone();
        async move { app.oneshot(proxy_request(&token1)).await }
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    // User 2 tries to use the proxy — they have their own per-credential
    // budget but the global cap is exhausted, so this must 429.
    let resp = app
        .clone()
        .oneshot(proxy_request(&second_token))
        .await
        .expect("request should respond, not panic");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["error"]["type"], "overloaded_error");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Server is at capacity"),
        "global cap rejection should use the server-wide message: {body}"
    );
}

// --- /api/v1/me/anthropic-key HTTP lifecycle (deferred from T02) ---------

#[sqlx::test(migrations = "./migrations")]
async fn me_anthropic_key_lifecycle(pool: sqlx::PgPool) {
    // Use a clean pool for this test (no key pre-seeded) — `build_harness`
    // seeds one, so we make a parallel handcrafted state instead.
    let upstream = MockServer::start().await;
    let user = common::seed_user(&pool).await;
    let session = common::seed_auth_session(&pool, user).await;
    let encryption_key = common::fixture_encryption_key();

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: upstream.uri(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    };

    let app = Router::new()
        .route(
            "/api/v1/me/anthropic-key",
            get(api::me::get_anthropic_key_status)
                .put(api::me::put_anthropic_key)
                .delete(api::me::delete_anthropic_key),
        )
        .with_state(state);

    let bearer = format!("Bearer {session}");

    // GET before PUT -> configured=false
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["configured"], false);
    assert!(body["configured_at"].is_null());

    // PUT empty key -> 400
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"key":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // PUT key with wrong prefix -> 400
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"key":"not-an-anthropic-key"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // PUT valid key -> 204
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"key":"sk-ant-test-12345"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET after PUT -> configured=true with timestamp
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["configured"], true);
    assert!(body["configured_at"].is_string());

    // DELETE -> 204
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // GET after DELETE -> configured=false again
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body_to_value(resp.into_body()).await;
    assert_eq!(body["configured"], false);
}

/// Build a minimal app + state with just the /me/anthropic-key endpoints,
/// for tests that exercise the settings-only PUT path. Returns
/// (app, bearer-header-value, user_id, shared-semaphore-map).
async fn build_me_endpoints_only(
    pool: sqlx::PgPool,
) -> (
    Router,
    String,
    uuid::Uuid,
    std::sync::Arc<dashmap::DashMap<uuid::Uuid, std::sync::Arc<tokio::sync::Semaphore>>>,
) {
    let upstream = MockServer::start().await;
    let user = common::seed_user(&pool).await;
    let session = common::seed_auth_session(&pool, user).await;
    let encryption_key = common::fixture_encryption_key();
    let sems: std::sync::Arc<dashmap::DashMap<uuid::Uuid, std::sync::Arc<tokio::sync::Semaphore>>> =
        std::sync::Arc::new(dashmap::DashMap::new());

    let state = AppState {
        pool: pool.clone(),
        repo_manager: repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: Some(encryption_key),
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: upstream.uri(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: sems.clone(),
    };

    let app = Router::new()
        .route(
            "/api/v1/me/anthropic-key",
            get(api::me::get_anthropic_key_status)
                .put(api::me::put_anthropic_key)
                .delete(api::me::delete_anthropic_key),
        )
        .with_state(state);

    (app, format!("Bearer {session}"), user, sems)
}

fn put_request(bearer: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("PUT")
        .uri("/api/v1/me/anthropic-key")
        .header("authorization", bearer)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Setting cap-only on an existing row must update only max_concurrent and
/// preserve the ciphertext, *and* must drop the in-memory semaphore so the
/// next proxy request rebuilds it with the new cap.
#[sqlx::test(migrations = "./migrations")]
async fn me_anthropic_key_put_updates_cap_only(pool: sqlx::PgPool) {
    let (app, bearer, user_id, sems) = build_me_endpoints_only(pool.clone()).await;

    // Seed initial key with cap=4.
    let r = app
        .clone()
        .oneshot(put_request(
            &bearer,
            serde_json::json!({ "key": "sk-ant-initial-fixture", "max_concurrent": 4 }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Prime the in-memory semaphore so we can prove it gets dropped.
    sems.entry(user_id)
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)));
    assert!(sems.contains_key(&user_id));

    // Cap-only PUT.
    let r = app
        .clone()
        .oneshot(put_request(
            &bearer,
            serde_json::json!({ "max_concurrent": 16 }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Verify ciphertext unchanged via the GET endpoint (status returns
    // configured=true, max_concurrent=16) AND that the semaphore entry
    // is gone.
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/me/anthropic-key")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = read_body_to_value(r.into_body()).await;
    assert_eq!(body["configured"], true);
    assert_eq!(body["max_concurrent"], 16);
    assert!(
        !sems.contains_key(&user_id),
        "settings-only PUT must drop the in-memory semaphore so the new cap takes effect"
    );

    // And the encrypted key in the DB really is still the initial one.
    let cred =
        tracevault_server::repo::credentials::CredentialRepo::resolve_default(&pool, user_id)
            .await
            .unwrap()
            .unwrap();
    let plaintext = tracevault_server::encryption::decrypt(
        &cred.encrypted,
        &cred.nonce,
        &common::fixture_encryption_key(),
    )
    .unwrap();
    assert_eq!(plaintext, "sk-ant-initial-fixture");
    assert_eq!(cred.max_concurrent, 16);
}

/// Cap-only PUT before any key has been configured must return 400 — we
/// don't want a half-row containing only a cap and no key material.
#[sqlx::test(migrations = "./migrations")]
async fn me_anthropic_key_put_rejects_cap_only_when_unconfigured(pool: sqlx::PgPool) {
    let (app, bearer, _user_id, _sems) = build_me_endpoints_only(pool).await;

    let r = app
        .clone()
        .oneshot(put_request(
            &bearer,
            serde_json::json!({ "max_concurrent": 16 }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

/// Empty body (neither key nor cap) must return 400 rather than silently
/// noop.
#[sqlx::test(migrations = "./migrations")]
async fn me_anthropic_key_put_rejects_empty_body(pool: sqlx::PgPool) {
    let (app, bearer, _user_id, _sems) = build_me_endpoints_only(pool).await;

    let r = app
        .clone()
        .oneshot(put_request(&bearer, serde_json::json!({})))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}

/// A no-key PUT carrying both `base_url` and `max_concurrent` must be
/// rejected with 400 (base_url can only ride along with a key) — and it must
/// be rejected *whole*, never partially applying the cap or silently
/// dropping the base_url change.
#[sqlx::test(migrations = "./migrations")]
async fn me_anthropic_key_put_rejects_base_url_without_key(pool: sqlx::PgPool) {
    let (app, bearer, user_id, _sems) = build_me_endpoints_only(pool.clone()).await;

    // Seed an initial key with cap=4 so there is a row to (not) mutate.
    let r = app
        .clone()
        .oneshot(put_request(
            &bearer,
            serde_json::json!({ "key": "sk-ant-initial-fixture", "max_concurrent": 4 }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    let original =
        tracevault_server::repo::credentials::CredentialRepo::resolve_default(&pool, user_id)
            .await
            .unwrap()
            .unwrap();

    // No-key PUT with both base_url and max_concurrent must 400.
    let r = app
        .clone()
        .oneshot(put_request(
            &bearer,
            serde_json::json!({ "max_concurrent": 16, "base_url": "https://example.com" }),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // The request was rejected whole: neither base_url nor the cap changed.
    let after =
        tracevault_server::repo::credentials::CredentialRepo::resolve_default(&pool, user_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        after.base_url, original.base_url,
        "rejected PUT must not change the stored base_url"
    );
    assert_eq!(
        after.max_concurrent, original.max_concurrent,
        "rejected PUT must not partially apply the cap"
    );
}
