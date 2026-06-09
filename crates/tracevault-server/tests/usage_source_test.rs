mod common;

use tracevault_server::api::analytics::UsageSource;

#[sqlx::test(migrations = "./migrations")]
async fn usage_source_defaults_to_both(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(org_id)
        .execute(&pool)
        .await
        .unwrap();

    let src = tracevault_server::api::analytics::fetch_usage_source_for_test(&pool, org_id).await;
    assert_eq!(src, UsageSource::Both);
}

async fn seed_one_session_and_one_ledger(pool: &sqlx::PgPool) -> (uuid::Uuid, String) {
    use tracevault_server::repo::llm_calls::{LlmCallRecord, LlmCallRepo};
    let user_id = common::seed_user(pool).await;
    let org_id = common::seed_org_with_member(pool, user_id).await;
    sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(org_id)
        .execute(pool)
        .await
        .unwrap();
    let repo_id = common::seed_repo(pool, org_id).await;
    let sess = common::seed_session(pool, org_id, repo_id, user_id).await;
    sqlx::query("UPDATE sessions SET input_tokens=100, output_tokens=10, total_tokens=110, estimated_cost_usd=0.10, model='m' WHERE id=$1")
        .bind(sess)
        .execute(pool)
        .await
        .unwrap();
    let rec = LlmCallRecord {
        org_id,
        user_id,
        credential_id: None,
        auth_session_id: None,
        client_session_id: None,
        repo_id: Some(repo_id),
        branch: None,
        requested_model: Some("m".into()),
        provider_model: None,
        response_model: Some("m".into()),
        input_tokens: Some(200),
        output_tokens: Some(20),
        cache_read_tokens: Some(0),
        cache_write_tokens: Some(0),
        total_tokens: Some(220),
        estimated_cost_usd: Some(0.20),
        stop_reason: Some("end_turn".into()),
        http_status: 200,
        outcome: "success".into(),
        duration_ms: 1,
        anthropic_request_id: None,
        path: "v1/messages".into(),
    };
    LlmCallRepo::insert(pool, &rec).await.unwrap();
    let email = sqlx::query_scalar::<_, String>("SELECT email FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (org_id, email)
}

async fn set_source(pool: &sqlx::PgPool, org_id: uuid::Uuid, src: &str) {
    sqlx::query("UPDATE org_compliance_settings SET usage_source=$2 WHERE org_id=$1")
        .bind(org_id)
        .bind(src)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn overview_total_tokens_respects_source(pool: sqlx::PgPool) {
    let (org_id, _email) = seed_one_session_and_one_ledger(&pool).await;
    set_source(&pool, org_id, "both").await;
    assert_eq!(
        tracevault_server::api::analytics::overview_total_tokens_for_test(&pool, org_id).await,
        330
    );
    set_source(&pool, org_id, "hook").await;
    assert_eq!(
        tracevault_server::api::analytics::overview_total_tokens_for_test(&pool, org_id).await,
        110
    );
    set_source(&pool, org_id, "proxy").await;
    assert_eq!(
        tracevault_server::api::analytics::overview_total_tokens_for_test(&pool, org_id).await,
        220
    );
}

// Foundation coverage gap follow-up: verify the Rust absent-row default path.
#[sqlx::test(migrations = "./migrations")]
async fn usage_source_absent_row_defaults_to_both(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    // deliberately NO org_compliance_settings row
    let src = tracevault_server::api::analytics::fetch_usage_source_for_test(&pool, org_id).await;
    assert_eq!(src, tracevault_server::api::analytics::UsageSource::Both);
}

#[sqlx::test(migrations = "./migrations")]
async fn tokens_by_author_respects_source(pool: sqlx::PgPool) {
    let (org_id, email) = seed_one_session_and_one_ledger(&pool).await;
    set_source(&pool, org_id, "both").await;
    assert_eq!(
        tracevault_server::api::analytics::tokens_by_author_total_for_test(&pool, org_id, &email)
            .await,
        330
    );
    set_source(&pool, org_id, "hook").await;
    assert_eq!(
        tracevault_server::api::analytics::tokens_by_author_total_for_test(&pool, org_id, &email)
            .await,
        110
    );
    set_source(&pool, org_id, "proxy").await;
    assert_eq!(
        tracevault_server::api::analytics::tokens_by_author_total_for_test(&pool, org_id, &email)
            .await,
        220
    );
}
