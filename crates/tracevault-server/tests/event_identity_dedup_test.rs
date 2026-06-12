mod common;

use sqlx::PgPool;
use tracevault_server::repo::events::{EventRepo, InsertToolEvent};

fn evt(
    session_id: uuid::Uuid,
    event_index: i32,
    tool_use_id: Option<&str>,
    hook_event_name: Option<&str>,
) -> InsertToolEvent {
    InsertToolEvent {
        session_id,
        event_index,
        tool_name: Some("Edit".to_string()),
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        timestamp: Some(chrono::Utc::now()),
        hook_event_name: hook_event_name.map(|s| s.to_string()),
        tool_use_id: tool_use_id.map(|s| s.to_string()),
    }
}

async fn event_count(pool: &PgPool, session_id: uuid::Uuid) -> i64 {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM events WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_session(pool: &PgPool) -> uuid::Uuid {
    let user_id = common::seed_user(pool).await;
    let org_id = common::seed_org_with_member(pool, user_id).await;
    let repo_id = common::seed_repo(pool, org_id).await;
    common::seed_session(pool, org_id, repo_id, user_id).await
}

/// The core fix: two parallel-tool events that raced on the CLI counter and
/// therefore carry the SAME event_index, but are DIFFERENT tools, must BOTH
/// persist. Under the old `ON CONFLICT (session_id, event_index)` the second was
/// silently dropped.
#[sqlx::test(migrations = "./migrations")]
async fn raced_parallel_tools_with_same_index_both_persist(pool: PgPool) {
    let session_id = seed_session(&pool).await;

    let a = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 0, Some("toolu_aaa"), Some("PostToolUse")),
    )
    .await
    .unwrap();
    let b = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 0, Some("toolu_bbb"), Some("PostToolUse")),
    )
    .await
    .unwrap();

    assert!(a.is_some(), "first raced event should insert");
    assert!(
        b.is_some(),
        "second raced event (same index, other tool) must NOT be dropped"
    );
    assert_eq!(event_count(&pool, session_id).await, 2);
}

/// Identity dedup ignores event_index: re-delivering the same tool event with a
/// different (incremented) index collapses to one row.
#[sqlx::test(migrations = "./migrations")]
async fn same_identity_different_index_dedups(pool: PgPool) {
    let session_id = seed_session(&pool).await;

    let first = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 5, Some("toolu_x"), Some("PostToolUse")),
    )
    .await
    .unwrap();
    let again = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 6, Some("toolu_x"), Some("PostToolUse")),
    )
    .await
    .unwrap();

    assert!(first.is_some());
    assert!(
        again.is_none(),
        "same (tool_use_id, hook_event_name) must dedup regardless of index"
    );
    assert_eq!(event_count(&pool, session_id).await, 1);
}

/// Pre and Post of one tool share a tool_use_id but differ on hook_event_name,
/// so they remain two distinct rows.
#[sqlx::test(migrations = "./migrations")]
async fn pre_and_post_are_distinct(pool: PgPool) {
    let session_id = seed_session(&pool).await;

    let pre = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 0, Some("toolu_y"), Some("PreToolUse")),
    )
    .await
    .unwrap();
    let post = EventRepo::insert_tool_event(
        &pool,
        &evt(session_id, 1, Some("toolu_y"), Some("PostToolUse")),
    )
    .await
    .unwrap();

    assert!(pre.is_some() && post.is_some());
    assert_eq!(event_count(&pool, session_id).await, 2);
}

/// Legacy events without a tool_use_id keep the historical event_index dedup.
#[sqlx::test(migrations = "./migrations")]
async fn legacy_null_tool_use_id_dedups_on_index(pool: PgPool) {
    let session_id = seed_session(&pool).await;

    let first = EventRepo::insert_tool_event(&pool, &evt(session_id, 3, None, Some("PostToolUse")))
        .await
        .unwrap();
    let dup = EventRepo::insert_tool_event(&pool, &evt(session_id, 3, None, Some("PostToolUse")))
        .await
        .unwrap();

    assert!(first.is_some());
    assert!(
        dup.is_none(),
        "legacy NULL-tool_use_id rows still dedup on (session_id, event_index)"
    );
    assert_eq!(event_count(&pool, session_id).await, 1);
}
