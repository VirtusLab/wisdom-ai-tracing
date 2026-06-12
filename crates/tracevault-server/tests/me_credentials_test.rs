//! HTTP-level tests for the named-credentials and proxy-routing-rule REST
//! endpoints (`/api/v1/me/credentials`, `/api/v1/me/proxy-routing`).
//!
//! Spins an in-process `axum::Router` carrying the `me` handlers over a real
//! Postgres pool (via `sqlx::test`), seeds a user + session token, and drives
//! the endpoints end-to-end. The harness mirrors the `me_anthropic_key_*`
//! tests in `proxy_integration.rs`.

mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{delete, get, put},
    Router,
};
use serde_json::Value;
use tower::ServiceExt;
use tracevault_server::{api, repo_manager, AppState};

/// Build a router carrying just the `me` credential + routing endpoints, plus
/// a freshly-seeded user and the bearer token for it.
async fn setup(pool: sqlx::PgPool) -> (Router, String) {
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
        default_credential_base_url: "https://api.anthropic.com".to_string(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
        agent_registry: std::sync::Arc::new(
            tracevault_core::agent_adapter::AgentAdapterRegistry::new(),
        ),
    };

    let app = Router::new()
        .route("/api/v1/me/credentials", get(api::me::list_credentials))
        .route(
            "/api/v1/me/credentials/{name}",
            put(api::me::put_credential).delete(api::me::delete_credential),
        )
        .route(
            "/api/v1/me/proxy-routing",
            get(api::me::list_routing_rules).put(api::me::put_routing_rule),
        )
        .route(
            "/api/v1/me/proxy-routing/{id}",
            delete(api::me::delete_routing_rule),
        )
        .with_state(state);

    (app, format!("Bearer {session}"))
}

async fn body_to_value(body: Body) -> Value {
    let bytes = to_bytes(body, 16 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn put_json(app: &Router, bearer: &str, uri: &str, body: &str) -> StatusCode {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("authorization", bearer)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    resp.status()
}

async fn get_value(app: &Router, bearer: &str, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let value = body_to_value(resp.into_body()).await;
    (status, value)
}

#[sqlx::test(migrations = "./migrations")]
async fn credentials_and_routing_lifecycle(pool: sqlx::PgPool) {
    let (app, bearer) = setup(pool).await;

    // --- PUT two credentials (default + "fast") ---
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/default",
        r#"{"key":"sk-ant-default-key","base_url":"https://api.anthropic.com","max_concurrent":8}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/fast",
        r#"{"key":"sk-ant-fast-key","base_url":"https://gw.example.com","max_concurrent":16}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // --- GET /me/credentials returns both ---
    let (status, list) = get_value(&app, &bearer, "/api/v1/me/credentials").await;
    assert_eq!(status, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr
        .iter()
        .any(|c| c["name"] == "default" && c["max_concurrent"] == 8));
    let fast = arr.iter().find(|c| c["name"] == "fast").unwrap();
    assert_eq!(fast["base_url"], "https://gw.example.com");
    assert_eq!(fast["max_concurrent"], 16);
    assert_eq!(fast["protocol"], "anthropic");
    assert!(!fast["configured_at"].is_null());

    // --- PUT /me/proxy-routing creates a model rule ---
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/proxy-routing",
        r#"{"match_model":"claude-haiku","credential_name":"fast","provider_model":"claude-3-5-haiku-latest"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // --- PUT /me/proxy-routing with unknown credential -> 400 (NOT 500) ---
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/proxy-routing",
        r#"{"match_model":"claude-opus","credential_name":"ghost"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // --- GET /me/proxy-routing lists default + model rule ---
    let (status, rules) = get_value(&app, &bearer, "/api/v1/me/proxy-routing").await;
    assert_eq!(status, StatusCode::OK);
    let arr = rules.as_array().unwrap();
    assert_eq!(arr.len(), 2, "expected default rule + model rule");
    // Default rule (seeded by the first credential PUT).
    assert!(arr
        .iter()
        .any(|r| r["match_model"].is_null() && r["credential_name"] == "default"));
    // Model rule.
    let model_rule = arr
        .iter()
        .find(|r| r["match_model"] == "claude-haiku")
        .unwrap();
    assert_eq!(model_rule["credential_name"], "fast");
    assert_eq!(model_rule["provider_model"], "claude-3-5-haiku-latest");
    let rule_id = model_rule["id"].as_str().unwrap().to_string();

    // --- DELETE the model rule -> 204 ---
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/me/proxy-routing/{rule_id}"))
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Deleting it again (or a non-existent rule) -> 404.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/me/proxy-routing/{rule_id}"))
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // --- DELETE a credential -> 204 ---
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/me/credentials/fast")
                .header("authorization", &bearer)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Only the default credential remains.
    let (_, list) = get_value(&app, &bearer, "/api/v1/me/credentials").await;
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "default");
}

#[sqlx::test(migrations = "./migrations")]
async fn me_credentials_put_edits_metadata_without_key(pool: sqlx::PgPool) {
    let (app, bearer) = setup(pool).await;

    // Create credential "work" with key, base_url, and max_concurrent.
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/work",
        r#"{"key":"sk-ant-initial","base_url":"https://api.anthropic.com","max_concurrent":8}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Update base_url and max_concurrent WITHOUT re-supplying the key -> 204.
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/work",
        r#"{"base_url":"https://gw.example.com","max_concurrent":20}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // GET /me/credentials and verify the metadata was updated.
    let (status, list) = get_value(&app, &bearer, "/api/v1/me/credentials").await;
    assert_eq!(status, StatusCode::OK);
    let arr = list.as_array().unwrap();
    let work = arr.iter().find(|c| c["name"] == "work").unwrap();
    assert_eq!(
        work["base_url"], "https://gw.example.com",
        "base_url should be updated without re-supplying key"
    );
    assert_eq!(
        work["max_concurrent"], 20,
        "max_concurrent should be updated without re-supplying key"
    );

    // Attempt to update a credential that doesn't exist (no key supplied) -> 400.
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/ghost",
        r#"{"base_url":"https://x.example.com"}"#,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "creating a credential without a key should be rejected"
    );

    // SSRF validator still applies on the no-key path -> 400.
    let status = put_json(
        &app,
        &bearer,
        "/api/v1/me/credentials/work",
        r#"{"base_url":"http://insecure"}"#,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "insecure (http) base_url should be rejected even without a key"
    );
}
