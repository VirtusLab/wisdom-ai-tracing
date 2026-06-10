mod common;

use tracevault_server::repo::llm_calls::{LlmCallRecord, LlmCallRepo};
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn kpis_sum_ledger_rows(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    for (inp, cost) in [(50i64, 0.01f64), (70i64, 0.02f64)] {
        let rec = LlmCallRecord {
            org_id,
            user_id,
            credential_id: None,
            auth_session_id: None,
            client_session_id: None,
            repo_id: None,
            branch: None,
            requested_model: None,
            provider_model: None,
            response_model: Some("m".into()),
            input_tokens: Some(inp),
            output_tokens: Some(5),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
            total_tokens: Some(inp + 5),
            estimated_cost_usd: Some(cost),
            stop_reason: None,
            http_status: 200,
            outcome: "success".into(),
            duration_ms: 1,
            anthropic_request_id: None,
            path: "v1/messages".into(),
            anthropic_message_id: None,
        };
        LlmCallRepo::insert(&pool, &rec).await.unwrap();
    }
    let k = LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, None, None, None, false)
        .await
        .unwrap();
    assert_eq!(k.input_tokens, 120);
    assert!((k.cost_usd - 0.03).abs() < 1e-9);
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_ledger_row(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;

    let rec = LlmCallRecord {
        org_id,
        user_id,
        credential_id: None,
        auth_session_id: None,
        client_session_id: None,
        repo_id: None,
        branch: None,
        requested_model: Some("claude-opus-4-6".into()),
        provider_model: None,
        response_model: Some("claude-opus-4-6".into()),
        input_tokens: Some(50),
        output_tokens: Some(12),
        cache_read_tokens: Some(1000),
        cache_write_tokens: Some(500),
        total_tokens: Some(50 + 12 + 1000 + 500),
        estimated_cost_usd: Some(0.0123),
        stop_reason: Some("end_turn".into()),
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 842,
        anthropic_request_id: Some(format!("req_{}", Uuid::new_v4())),
        path: "v1/messages".into(),
        anthropic_message_id: None,
    };

    let id = LlmCallRepo::insert(&pool, &rec)
        .await
        .unwrap()
        .expect("row inserted");

    let (input, total, outcome): (Option<i64>, Option<i64>, String) =
        sqlx::query_as("SELECT input_tokens, total_tokens, outcome FROM llm_calls WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(input, Some(50));
    assert_eq!(total, Some(1562));
    assert_eq!(outcome, "success");
}

#[sqlx::test(migrations = "./migrations")]
async fn fetch_ledger_kpis_respects_window_and_filters(pool: sqlx::PgPool) {
    use chrono::{TimeZone, Utc};
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let repo_name: String = sqlx::query_scalar("SELECT name FROM repos WHERE id = $1")
        .bind(repo_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let mk = |req: &str, tok: i64| LlmCallRecord {
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
        input_tokens: Some(tok),
        output_tokens: Some(0),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        total_tokens: Some(tok),
        estimated_cost_usd: Some(0.1),
        stop_reason: None,
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 1,
        anthropic_request_id: Some(req.into()),
        path: "v1/messages".into(),
        anthropic_message_id: None,
    };
    LlmCallRepo::insert(&pool, &mk("early", 100)).await.unwrap();
    LlmCallRepo::insert(&pool, &mk("late", 200)).await.unwrap();
    sqlx::query("UPDATE llm_calls SET created_at = '2026-01-01T00:00:00Z' WHERE anthropic_request_id = 'early'")
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE llm_calls SET created_at = '2026-03-01T00:00:00Z' WHERE anthropic_request_id = 'late'")
        .execute(&pool).await.unwrap();

    let all = LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, None, None, None, false)
        .await
        .unwrap();
    assert_eq!(all.total_tokens, 300, "no filters: both rows");

    // Window covering only the January row.
    let from = Utc.with_ymd_and_hms(2025, 12, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
    let win =
        LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, None, Some(from), Some(to), false)
            .await
            .unwrap();
    assert_eq!(win.total_tokens, 100, "time window excludes the March row");

    // Repo filter.
    assert_eq!(
        LlmCallRepo::fetch_ledger_kpis(&pool, org_id, Some(&repo_name), None, None, None, false)
            .await
            .unwrap()
            .total_tokens,
        300,
        "matching repo includes both"
    );
    assert_eq!(
        LlmCallRepo::fetch_ledger_kpis(
            &pool,
            org_id,
            Some("no-such-repo"),
            None,
            None,
            None,
            false
        )
        .await
        .unwrap()
        .total_tokens,
        0,
        "non-matching repo excludes all"
    );

    // Author filter.
    assert_eq!(
        LlmCallRepo::fetch_ledger_kpis(&pool, org_id, None, Some(&email), None, None, false)
            .await
            .unwrap()
            .total_tokens,
        300,
        "matching author includes both"
    );
    assert_eq!(
        LlmCallRepo::fetch_ledger_kpis(
            &pool,
            org_id,
            None,
            Some("nobody@x.test"),
            None,
            None,
            false
        )
        .await
        .unwrap()
        .total_tokens,
        0,
        "non-matching author excludes all"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_is_idempotent_on_duplicate_request_id(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;

    let mk = |req_id: &str| LlmCallRecord {
        org_id,
        user_id,
        credential_id: None,
        auth_session_id: None,
        client_session_id: None,
        repo_id: None,
        branch: None,
        requested_model: None,
        provider_model: None,
        response_model: Some("m".into()),
        input_tokens: Some(10),
        output_tokens: Some(2),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        total_tokens: Some(12),
        estimated_cost_usd: Some(0.01),
        stop_reason: None,
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 1,
        anthropic_request_id: Some(req_id.into()),
        path: "v1/messages".into(),
        anthropic_message_id: None,
    };

    // First insert returns an id; a second with the SAME request_id is a no-op
    // (returns None) rather than a unique-violation error.
    let first = LlmCallRepo::insert(&pool, &mk("req_dup")).await.unwrap();
    let second = LlmCallRepo::insert(&pool, &mk("req_dup")).await.unwrap();
    assert!(first.is_some(), "first insert should create a row");
    assert!(second.is_none(), "duplicate request_id must be a no-op");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM llm_calls WHERE org_id = $1")
        .bind(org_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "only one row despite two inserts");
}
