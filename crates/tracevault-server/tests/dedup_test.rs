mod common;

use tracevault_server::service::stream::StreamService;
use tracevault_server::AppState;
use tracevault_core::streaming::{StreamEventRequest, StreamEventType};

fn build_state(pool: sqlx::PgPool) -> AppState {
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

fn transcript_request(session_id: &str, lines: Vec<serde_json::Value>) -> StreamEventRequest {
    StreamEventRequest {
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
        transcript_lines: Some(lines),
        transcript_offset: Some(0),
        model: Some("claude-sonnet-4-6".to_string()),
        cwd: Some("/project".to_string()),
        final_stats: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn ingestion_records_assistant_message_id(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg_test_X",
            "model": "claude-sonnet-4-6",
            "usage": { "input_tokens": 100, "output_tokens": 10 }
        }
    });
    let req = transcript_request("sess-dedup-1", vec![line]);

    let state = build_state(pool.clone());
    StreamService::process(&state, org_id, repo_id, user_id, req)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM session_message_ids WHERE anthropic_message_id = $1 AND org_id = $2",
    )
    .bind("msg_test_X")
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "expected one session_message_ids row for msg_test_X");
}

#[sqlx::test(migrations = "./migrations")]
async fn ingestion_message_id_is_idempotent(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let line = serde_json::json!({
        "type": "assistant",
        "message": { "id": "msg_test_Y", "model": "m", "usage": { "input_tokens": 1, "output_tokens": 1 } }
    });

    let state = build_state(pool.clone());
    // Same session, same transcript line, sent twice (overlapping batch).
    StreamService::process(&state, org_id, repo_id, user_id, transcript_request("sess-dedup-2", vec![line.clone()]))
        .await.unwrap();
    StreamService::process(&state, org_id, repo_id, user_id, transcript_request("sess-dedup-2", vec![line]))
        .await.unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM session_message_ids WHERE anthropic_message_id = $1",
    )
    .bind("msg_test_Y")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "duplicate ingestion must not create a second row");
}
