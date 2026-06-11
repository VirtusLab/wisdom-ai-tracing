mod common;

use sqlx::PgPool;
use tracevault_core::streaming::{StreamEventRequest, StreamEventType};
use tracevault_server::service::stream::StreamService;
use tracevault_server::AppState;

// Build an AppState on the COMMUNITY registry — its pricing provider reports
// cost disabled, so ingested costs must be $0 (Cost Analytics is enterprise).
fn community_state(pool: PgPool) -> AppState {
    AppState {
        pool,
        repo_manager: tracevault_server::repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: None,
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: "http://localhost".to_string(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
    }
}

/// On the community edition, hook ingestion must record session cost as $0 even
/// for a model that has (fallback) pricing — cost analytics is enterprise-only.
/// Without the gate, `fetch_pricing_for_model` falls back to non-zero Sonnet
/// rates and this session would carry a real cost.
#[sqlx::test(migrations = "./migrations")]
async fn community_ingest_records_zero_cost(pool: PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg_cost_gate",
            "model": "claude-sonnet-4-6",
            "usage": { "input_tokens": 10000, "output_tokens": 5000 }
        }
    });
    let req = StreamEventRequest {
        protocol_version: 2,
        tool: Some("claude-code".to_string()),
        event_type: StreamEventType::Transcript,
        session_id: "sess-cost-gate".to_string(),
        timestamp: chrono::Utc::now(),
        hook_event_name: None,
        tool_name: None,
        tool_use_id: None,
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        event_index: None,
        transcript_lines: Some(vec![line]),
        transcript_offset: Some(0),
        model: Some("claude-sonnet-4-6".to_string()),
        cwd: Some("/project".to_string()),
        final_stats: None,
    };

    let state = community_state(pool.clone());
    StreamService::process(&state, org_id, repo_id, user_id, req)
        .await
        .unwrap();

    // Tokens are recorded, but cost is gated to $0 under the community edition.
    let (tokens, cost): (Option<i64>, Option<f64>) = sqlx::query_as(
        "SELECT total_tokens, estimated_cost_usd FROM sessions WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tokens, Some(15000), "tokens are still recorded");
    assert_eq!(
        cost.unwrap_or(-1.0),
        0.0,
        "community edition must record $0 cost (cost analytics is enterprise)"
    );
}
