mod common;

use chrono::Utc;
use tracevault_server::repo::sessions::{SessionRepo, TokenBatch, UpsertSession};

#[sqlx::test(migrations = "./migrations")]
async fn upsert_creates_session(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "test-session-1".into(),
            model: Some("claude-sonnet".into()),
            cwd: Some("/home/user/project".into()),
            tool: Some("claude-code".into()),
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    assert_ne!(id, uuid::Uuid::nil());
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_returns_same_id_on_conflict(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let make = || UpsertSession {
        org_id,
        repo_id,
        user_id,
        session_id: "s-dup".into(),
        model: None,
        cwd: None,
        tool: Some("claude-code".into()),
        timestamp: Some(Utc::now()),
    };

    let id1 = SessionRepo::upsert(&pool, &make()).await.unwrap();
    let id2 = SessionRepo::upsert(&pool, &make()).await.unwrap();
    assert_eq!(id1, id2);
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_reactivates_completed_session(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;
    let now = Utc::now();

    let id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "s-reactivate".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(now),
        },
    )
    .await
    .unwrap();

    SessionRepo::complete_minimal(&pool, id, Some(now))
        .await
        .unwrap();

    // Verify it is completed
    let (status,): (String,) = sqlx::query_as("SELECT status FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "completed");

    // Upsert again — should reactivate
    let id2 = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "s-reactivate".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(now),
        },
    )
    .await
    .unwrap();

    assert_eq!(id, id2);
    let (status,): (String,) = sqlx::query_as("SELECT status FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "active");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_tokens_accumulates(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "s-tokens".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    SessionRepo::update_tokens(
        &pool,
        id,
        &TokenBatch {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
            estimated_cost_usd: 0.01,
            model: None,
        },
    )
    .await
    .unwrap();

    SessionRepo::update_tokens(
        &pool,
        id,
        &TokenBatch {
            input_tokens: 200,
            output_tokens: 100,
            cache_read_tokens: 20,
            cache_write_tokens: 10,
            estimated_cost_usd: 0.02,
            model: None,
        },
    )
    .await
    .unwrap();

    let (input, output, cache_read, cache_write, total): (i64, i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, total_tokens \
             FROM sessions WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(input, 300);
    assert_eq!(output, 150);
    assert_eq!(cache_read, 30);
    assert_eq!(cache_write, 15);
    assert_eq!(total, 300 + 150 + 30 + 15);
}

#[sqlx::test(migrations = "./migrations")]
async fn increment_tool_calls(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "s-tools".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    SessionRepo::increment_tool_calls(&pool, id).await.unwrap();
    SessionRepo::increment_tool_calls(&pool, id).await.unwrap();
    SessionRepo::increment_tool_calls(&pool, id).await.unwrap();

    let (count,): (i32,) = sqlx::query_as("SELECT total_tool_calls FROM sessions WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn complete_with_stats(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;
    let session_id = common::seed_session(&pool, org_id, repo_id, user_id).await;

    let stats = tracevault_core::streaming::SessionFinalStats {
        duration_ms: Some(5000),
        total_tokens: Some(200),
        input_tokens: Some(150),
        output_tokens: Some(50),
        cache_read_tokens: None,
        cache_write_tokens: None,
        user_messages: Some(3),
        assistant_messages: Some(3),
        total_tool_calls: Some(10),
    };

    SessionRepo::complete_with_stats(&pool, session_id, Some(Utc::now()), &stats)
        .await
        .unwrap();

    let row: (Option<String>, Option<i64>, Option<i32>) =
        sqlx::query_as("SELECT status, duration_ms, user_messages FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.0.as_deref(), Some("completed"));
    assert_eq!(row.1, Some(5000));
    assert_eq!(row.2, Some(3));
}

#[sqlx::test(migrations = "./migrations")]
async fn complete_minimal(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;
    let session_id = common::seed_session(&pool, org_id, repo_id, user_id).await;

    SessionRepo::complete_minimal(&pool, session_id, Some(Utc::now()))
        .await
        .unwrap();

    let (status,): (Option<String>,) = sqlx::query_as("SELECT status FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status.as_deref(), Some("completed"));
}
