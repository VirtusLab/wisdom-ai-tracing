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

    let id = LlmCallRepo::insert(&pool, &rec).await.unwrap();

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
