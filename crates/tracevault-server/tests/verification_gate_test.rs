mod common;

use chrono::Utc;
use tracevault_server::repo::events::EventRepo;

#[sqlx::test(migrations = "./migrations")]
async fn verification_gate_counts_only_posttooluse_and_excludes_verify_start(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;
    let session_db_id = common::seed_session(&pool, org_id, repo_id, user_id).await;

    let window_start = Utc::now();

    // Row 1: Bash / PostToolUse / command=`tracevault verify-start`
    // → should be EXCLUDED by the verify-start classifier
    sqlx::query(
        "INSERT INTO events \
         (session_id, event_index, event_type, tool_name, tool_input, is_error, timestamp, hook_event_name) \
         VALUES ($1, $2, 'tool_use', $3, $4, false, $5, $6)",
    )
    .bind(session_db_id)
    .bind(1i32)
    .bind("Bash")
    .bind(Some(serde_json::json!({"command": "tracevault verify-start"})))
    .bind(window_start + chrono::Duration::seconds(1))
    .bind("PostToolUse")
    .execute(&pool)
    .await
    .unwrap();

    // Row 2: Bash / PreToolUse / command=`git push origin main`
    // → PreToolUse, not a Post → should NOT be counted
    sqlx::query(
        "INSERT INTO events \
         (session_id, event_index, event_type, tool_name, tool_input, is_error, timestamp, hook_event_name) \
         VALUES ($1, $2, 'tool_use', $3, $4, false, $5, $6)",
    )
    .bind(session_db_id)
    .bind(2i32)
    .bind("Bash")
    .bind(Some(serde_json::json!({"command": "git push origin main"})))
    .bind(window_start + chrono::Duration::seconds(2))
    .bind("PreToolUse")
    .execute(&pool)
    .await
    .unwrap();

    // Row 3: Edit / PostToolUse → should be counted (total=1, successful=1)
    sqlx::query(
        "INSERT INTO events \
         (session_id, event_index, event_type, tool_name, tool_input, is_error, timestamp, hook_event_name) \
         VALUES ($1, $2, 'tool_use', $3, $4, false, $5, $6)",
    )
    .bind(session_db_id)
    .bind(3i32)
    .bind("Edit")
    .bind(None::<serde_json::Value>)
    .bind(window_start + chrono::Duration::seconds(3))
    .bind("PostToolUse")
    .execute(&pool)
    .await
    .unwrap();

    // Row 4: Read / PostToolUse → should be counted (total=1, successful=1)
    sqlx::query(
        "INSERT INTO events \
         (session_id, event_index, event_type, tool_name, tool_input, is_error, timestamp, hook_event_name) \
         VALUES ($1, $2, 'tool_use', $3, $4, false, $5, $6)",
    )
    .bind(session_db_id)
    .bind(4i32)
    .bind("Read")
    .bind(None::<serde_json::Value>)
    .bind(window_start + chrono::Duration::seconds(4))
    .bind("PostToolUse")
    .execute(&pool)
    .await
    .unwrap();

    let stats =
        EventRepo::get_verification_phase_tool_call_stats(&pool, session_db_id, window_start)
            .await
            .unwrap();

    // Bash must be absent: the verify-start PostToolUse is excluded by the
    // classifier, and the git-push PreToolUse is filtered out by the SQL.
    assert!(
        !stats.contains_key("Bash"),
        "Bash should be absent from stats; got: {:?}",
        stats.keys().collect::<Vec<_>>()
    );

    // Edit and Read must each have total == 1.
    let edit = stats.get("Edit").expect("Edit should be present in stats");
    assert_eq!(edit.total, 1, "Edit total should be 1");

    let read = stats.get("Read").expect("Read should be present in stats");
    assert_eq!(read.total, 1, "Read total should be 1");

    // No other tools should be present.
    assert_eq!(
        stats.len(),
        2,
        "Only Edit and Read should be in stats; got: {:?}",
        stats.keys().collect::<Vec<_>>()
    );
}
