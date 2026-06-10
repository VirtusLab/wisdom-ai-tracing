mod common;

use tracevault_core::streaming::{StreamEventRequest, StreamEventType};
use tracevault_server::service::stream::StreamService;
use tracevault_server::AppState;

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
    assert_eq!(
        count, 1,
        "expected one session_message_ids row for msg_test_X"
    );
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
    StreamService::process(
        &state,
        org_id,
        repo_id,
        user_id,
        transcript_request("sess-dedup-2", vec![line.clone()]),
    )
    .await
    .unwrap();
    StreamService::process(
        &state,
        org_id,
        repo_id,
        user_id,
        transcript_request("sess-dedup-2", vec![line]),
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM session_message_ids WHERE anthropic_message_id = $1",
    )
    .bind("msg_test_Y")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "duplicate ingestion must not create a second row");
}

#[sqlx::test(migrations = "./migrations")]
async fn fetch_ledger_kpis_dedups_under_both(pool: sqlx::PgPool) {
    use tracevault_server::repo::llm_calls::{LlmCallRecord, LlmCallRepo};
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let session_id = common::seed_session(&pool, org_id, repo_id, user_id).await;

    sqlx::query(
        "INSERT INTO session_message_ids (anthropic_message_id, org_id, session_id) VALUES ($1,$2,$3)",
    )
    .bind("msg_X").bind(org_id).bind(session_id)
    .execute(&pool).await.unwrap();

    let rec = LlmCallRecord {
        org_id,
        user_id,
        credential_id: None,
        auth_session_id: None,
        client_session_id: None,
        repo_id: Some(repo_id),
        branch: None,
        requested_model: None,
        provider_model: None,
        response_model: Some("m".into()),
        input_tokens: Some(180),
        output_tokens: Some(20),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        total_tokens: Some(200),
        estimated_cost_usd: Some(0.20),
        stop_reason: None,
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 1,
        anthropic_request_id: None,
        path: "v1/messages".into(),
        anthropic_message_id: Some("msg_X".into()),
    };
    LlmCallRepo::insert(&pool, &rec).await.unwrap();

    // dedup on (the `both` case): matched ledger row excluded.
    let deduped = LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, None, None, None, true)
        .await
        .unwrap();
    assert_eq!(deduped.total_tokens, 0, "msg_X ledger row must be deduped");

    // dedup off (the `proxy` case): row counts.
    let raw = LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, None, None, None, false)
        .await
        .unwrap();
    assert_eq!(raw.total_tokens, 200, "proxy mode ignores dedup");
}

// ── End-to-end dedup across analytics breakdowns ────────────────────────────
//
// These exercise the UNION-arm guards (Task 7) and the KPI fold (Task 5/6)
// through the public `*_for_test` analytics seams, with an OVERLAPPING message
// id seeded on both the session (hook) and the ledger (proxy) side.

async fn set_source(pool: &sqlx::PgPool, org_id: uuid::Uuid, src: &str) {
    sqlx::query("UPDATE org_compliance_settings SET usage_source=$2 WHERE org_id=$1")
        .bind(org_id)
        .bind(src)
        .execute(pool)
        .await
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn insert_ledger(
    pool: &sqlx::PgPool,
    org_id: uuid::Uuid,
    user_id: uuid::Uuid,
    repo_id: uuid::Uuid,
    model: &str,
    msg_id: Option<&str>,
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    cost: f64,
) {
    use tracevault_server::repo::llm_calls::{LlmCallRecord, LlmCallRepo};
    let rec = LlmCallRecord {
        org_id,
        user_id,
        credential_id: None,
        auth_session_id: None,
        client_session_id: None,
        repo_id: Some(repo_id),
        branch: None,
        requested_model: None,
        provider_model: None,
        response_model: Some(model.into()),
        input_tokens: Some(input),
        output_tokens: Some(output),
        cache_read_tokens: Some(cache_read),
        cache_write_tokens: Some(cache_write),
        // total_tokens is stored verbatim and excludes cache (matches the
        // sessions/ledger seeding used elsewhere: total == input + output).
        total_tokens: Some(input + output),
        estimated_cost_usd: Some(cost),
        stop_reason: None,
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 1,
        anthropic_request_id: None,
        path: "v1/messages".into(),
        anthropic_message_id: msg_id.map(String::from),
    };
    LlmCallRepo::insert(pool, &rec).await.unwrap();
}

/// Seed an org whose hook session and proxy ledger row are the SAME Anthropic
/// call (message id `msg_X`). Session: 110 tokens, model 'm', cache_read 50,
/// cost 0.10. Ledger msg_X: 220 tokens, model 'm', cache_read 70, cost 0.20.
/// Returns (org_id, author email, user_id, repo_id).
async fn seed_overlap(pool: &sqlx::PgPool) -> (uuid::Uuid, String, uuid::Uuid, uuid::Uuid) {
    let user_id = common::seed_user(pool).await;
    let org_id = common::seed_org_with_member(pool, user_id).await;
    sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();
    let repo_id = common::seed_repo(pool, org_id).await;
    let sess = common::seed_session(pool, org_id, repo_id, user_id).await;
    sqlx::query(
        "UPDATE sessions SET input_tokens=100, output_tokens=10, total_tokens=110, \
         estimated_cost_usd=0.10, cache_read_tokens=50, cache_write_tokens=5, model='m' WHERE id=$1",
    )
    .bind(sess)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_message_ids (anthropic_message_id, org_id, session_id) VALUES ($1,$2,$3)",
    )
    .bind("msg_X")
    .bind(org_id)
    .bind(sess)
    .execute(pool)
    .await
    .unwrap();
    // Proxy ledger row for the SAME call (msg_X): 200in/20out → 220 tokens,
    // cache_read 70 (mirrors the proven seeding in usage_source_test).
    insert_ledger(
        pool,
        org_id,
        user_id,
        repo_id,
        "m",
        Some("msg_X"),
        200,
        20,
        70,
        7,
        0.20,
    )
    .await;
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (org_id, email, user_id, repo_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn dedup_under_both_drops_overlapping_ledger_across_breakdowns(pool: sqlx::PgPool) {
    use tracevault_server::api::analytics as a;
    let (org_id, email, _user_id, _repo_id) = seed_overlap(&pool).await;

    // under `both`: the msg_X ledger row is the same call as the session, so it
    // is deduped everywhere → every total reflects the session (hook) value only.
    set_source(&pool, org_id, "both").await;
    assert_eq!(
        a::overview_total_tokens_for_test(&pool, org_id).await,
        110,
        "overview"
    );
    assert_eq!(
        a::tokens_by_author_total_for_test(&pool, org_id, &email).await,
        110,
        "tokens_by_author"
    );
    assert_eq!(
        a::author_tokens_for_test(&pool, org_id, &email).await,
        110,
        "authors_leaderboard"
    );
    assert_eq!(
        a::model_m_for_test(&pool, org_id).await,
        (110, 1),
        "models_distribution"
    );
    assert!(
        (a::cost_total_for_test(&pool, org_id).await - 0.10).abs() < 1e-9,
        "cost_total"
    );
    assert_eq!(
        a::cost_cache_read_total_for_test(&pool, org_id).await,
        50,
        "cost_cache_read"
    );

    // under `proxy`: session excluded, dedup OFF → the ledger row counts in full.
    set_source(&pool, org_id, "proxy").await;
    assert_eq!(
        a::overview_total_tokens_for_test(&pool, org_id).await,
        220,
        "overview proxy"
    );
    assert_eq!(
        a::model_m_for_test(&pool, org_id).await,
        (220, 0),
        "models proxy"
    );

    // under `hook`: ledger excluded entirely → session value.
    set_source(&pool, org_id, "hook").await;
    assert_eq!(
        a::overview_total_tokens_for_test(&pool, org_id).await,
        110,
        "overview hook"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn dedup_keeps_non_overlapping_and_null_id_ledger(pool: sqlx::PgPool) {
    use tracevault_server::api::analytics as a;
    let (org_id, _email, user_id, repo_id) = seed_overlap(&pool).await;

    // A genuine proxy-only call (msg_Y, no matching session) and an
    // un-correlatable row (NULL message id) — both model 'n' so they don't
    // perturb the model-'m' assertions. Both must survive dedup under `both`.
    insert_ledger(
        &pool,
        org_id,
        user_id,
        repo_id,
        "n",
        Some("msg_Y"),
        40,
        10,
        0,
        0,
        0.05,
    )
    .await;
    insert_ledger(&pool, org_id, user_id, repo_id, "n", None, 5, 2, 0, 0, 0.01).await;

    set_source(&pool, org_id, "both").await;
    // 110 (session) + 50 (msg_Y) + 7 (null-id) ; msg_X ledger 220 deduped.
    assert_eq!(
        a::overview_total_tokens_for_test(&pool, org_id).await,
        167,
        "both keeps non-overlap + null"
    );

    set_source(&pool, org_id, "proxy").await;
    // proxy: all ledger rows (220 + 50 + 7), session excluded, dedup off.
    assert_eq!(
        a::overview_total_tokens_for_test(&pool, org_id).await,
        277,
        "proxy counts all ledger"
    );
}
