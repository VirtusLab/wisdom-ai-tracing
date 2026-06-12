mod common;

use serde_json::json;
use tracevault_server::repo::policies::{PolicyEvaluationFilter, PolicyRepo};
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn repo_belongs_to_org_true(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    assert!(PolicyRepo::repo_belongs_to_org(&pool, repo_id, org_id)
        .await
        .unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn repo_belongs_to_org_false(pool: sqlx::PgPool) {
    let org1 = common::seed_org(&pool).await;
    let org2 = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org1).await;
    assert!(!PolicyRepo::repo_belongs_to_org(&pool, repo_id, org2)
        .await
        .unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn create_and_list_for_repo(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "repo-policy",
        "desc",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        "session",
        true,
    )
    .await
    .unwrap();

    let policies = PolicyRepo::list_for_repo(&pool, org_id, repo_id)
        .await
        .unwrap();
    assert!(!policies.is_empty());
    assert!(policies.iter().any(|p| p.name == "repo-policy"));
}

#[sqlx::test(migrations = "./migrations")]
async fn update_partial_coalesces(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "original",
        "desc",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        "session",
        true,
    )
    .await
    .unwrap();

    let updated = PolicyRepo::update(
        &pool,
        id,
        org_id,
        &Some("renamed".into()),
        &None,
        &None,
        &None,
        &None,
        &None,
        None,
    )
    .await
    .unwrap();

    assert!(updated.is_some());
    let p = updated.unwrap();
    assert_eq!(p.name, "renamed");
    assert_eq!(p.description, "desc");
}

#[sqlx::test(migrations = "./migrations")]
async fn update_nonexistent_returns_none(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let result = PolicyRepo::update(
        &pool,
        Uuid::new_v4(),
        org_id,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        None,
    )
    .await
    .unwrap();
    assert!(result.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_returns_rows_affected(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "to-delete",
        "d",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "low",
        "session",
        true,
    )
    .await
    .unwrap();

    assert_eq!(PolicyRepo::delete(&pool, id, org_id).await.unwrap(), 1);
    assert_eq!(PolicyRepo::delete(&pool, id, org_id).await.unwrap(), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn list_enabled_for_check_filters_disabled(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "enabled-policy",
        "d",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        "session",
        true,
    )
    .await
    .unwrap();

    PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "disabled-policy",
        "d",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        "session",
        false,
    )
    .await
    .unwrap();

    let enabled = PolicyRepo::list_enabled_for_check(&pool, org_id, repo_id)
        .await
        .unwrap();
    assert!(enabled
        .iter()
        .all(|(_, name, _, _, _, _)| name != "disabled-policy"));
    assert!(enabled
        .iter()
        .any(|(_, name, _, _, _, _)| name == "enabled-policy"));
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_and_list_evaluations(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (policy_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "trace-complete",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        "session",
        true,
    )
    .await
    .unwrap();

    // Two pass evaluations, one fail. The fail is newer; list should return
    // them DESC by evaluated_at.
    for (result, details) in [("pass", "ok 1"), ("pass", "ok 2"), ("fail", "missing tool")] {
        PolicyRepo::insert_evaluation(
            &pool,
            org_id,
            repo_id,
            Some(policy_id),
            "trace-complete",
            Some("session-xyz"),
            Some("abcdef1234"),
            result,
            "warn",
            details,
            "cli_check",
            None,
            false,
        )
        .await
        .unwrap();
    }

    let all = PolicyRepo::list_evaluations(
        &pool,
        org_id,
        repo_id,
        &PolicyEvaluationFilter {
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].result, "fail"); // most recent

    // Filter by result = pass
    let passes = PolicyRepo::list_evaluations(
        &pool,
        org_id,
        repo_id,
        &PolicyEvaluationFilter {
            result: Some("pass".into()),
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(passes.len(), 2);
    assert!(passes.iter().all(|e| e.result == "pass"));

    // Filter by policy_id
    let for_policy = PolicyRepo::list_evaluations(
        &pool,
        org_id,
        repo_id,
        &PolicyEvaluationFilter {
            policy_id: Some(policy_id),
            limit: 100,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(for_policy.len(), 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn evaluation_row_survives_policy_delete(pool: sqlx::PgPool) {
    // Deleting a rule must not cascade away its evaluation history —
    // policy_id goes null, but policy_name is the snapshot that keeps the
    // activity view meaningful.
    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (policy_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "will-be-deleted",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "low",
        "session",
        true,
    )
    .await
    .unwrap();

    PolicyRepo::insert_evaluation(
        &pool,
        org_id,
        repo_id,
        Some(policy_id),
        "will-be-deleted",
        None,
        None,
        "pass",
        "warn",
        "",
        "cli_check",
        None,
        false,
    )
    .await
    .unwrap();

    PolicyRepo::delete(&pool, policy_id, org_id).await.unwrap();

    let rows = PolicyRepo::list_evaluations(
        &pool,
        org_id,
        repo_id,
        &PolicyEvaluationFilter {
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert!(rows[0].policy_id.is_none());
    assert_eq!(rows[0].policy_name, "will-be-deleted");
}

// ── Verification phase scope tests ─────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn verification_phase_policy_skipped_when_no_phase(pool: sqlx::PgPool) {
    use tracevault_server::repo::policies::PolicyRepo;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (policy_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "require-fmt-in-window",
        "",
        &json!({"type": "RequiredToolCall", "tool_names": ["cargo_fmt"]}),
        "block_push",
        "medium",
        "verification_phase",
        true,
    )
    .await
    .unwrap();

    let policies = PolicyRepo::list_enabled_for_check(&pool, org_id, repo_id)
        .await
        .unwrap();

    // A verification_phase-scoped policy with no tool calls and no window
    // should be skipped (not fail) — the check_policies handler handles this,
    // but here we verify the DB layer returns scope correctly.
    assert!(policies
        .iter()
        .any(|(id, _, _, _, _, scope)| *id == policy_id && scope == "verification_phase"));
}

#[sqlx::test(migrations = "./migrations")]
async fn allow_scope_policy_stored_correctly(pool: sqlx::PgPool) {
    use tracevault_server::repo::policies::PolicyRepo;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (policy_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "allow-read-in-window",
        "",
        &json!({"type": "RequiredToolCall", "tool_names": ["Read"]}),
        "allow",
        "low",
        "verification_phase",
        true,
    )
    .await
    .unwrap();

    let policies = PolicyRepo::list_enabled_for_check(&pool, org_id, repo_id)
        .await
        .unwrap();

    let row = policies.iter().find(|(id, _, _, _, _, _)| *id == policy_id);
    assert!(row.is_some());
    let (_, _, _, action, _, scope) = row.unwrap();
    assert_eq!(action, "allow");
    assert_eq!(scope, "verification_phase");
}

#[sqlx::test(migrations = "./migrations")]
async fn both_scope_policy_stored_correctly(pool: sqlx::PgPool) {
    use tracevault_server::repo::policies::PolicyRepo;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (policy_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "check-in-both",
        "",
        &json!({"type": "RequiredToolCall", "tool_names": ["cargo_check"]}),
        "warn",
        "medium",
        "both",
        true,
    )
    .await
    .unwrap();

    let policies = PolicyRepo::list_enabled_for_check(&pool, org_id, repo_id)
        .await
        .unwrap();

    let row = policies.iter().find(|(id, _, _, _, _, _)| *id == policy_id);
    assert!(row.is_some());
    let (_, _, _, _, _, scope) = row.unwrap();
    assert_eq!(scope, "both");
}

#[sqlx::test(migrations = "./migrations")]
async fn get_verification_phase_mode_default_is_disabled(pool: sqlx::PgPool) {
    use tracevault_server::repo::policies::PolicyRepo;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let mode = PolicyRepo::get_verification_phase_mode(&pool, repo_id)
        .await
        .unwrap();
    assert_eq!(mode, "disabled");
}

#[sqlx::test(migrations = "./migrations")]
async fn window_tool_call_stats_after_timestamp(pool: sqlx::PgPool) {
    use tracevault_server::repo::events::{EventRepo, InsertToolEvent};
    use tracevault_server::repo::sessions::{SessionRepo, UpsertSession};

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let session_db_id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "test-window-sess".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(chrono::Utc::now()),
        },
    )
    .await
    .unwrap();

    let before_window = chrono::Utc::now();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Event inside the window — must be PostToolUse to be counted.
    EventRepo::insert_tool_event(
        &pool,
        &InsertToolEvent {
            session_id: session_db_id,
            event_index: Some(1),
            event_uuid: None,
            tool_name: Some("cargo_fmt".into()),
            tool_input: None,
            tool_response: None,
            tool_is_error: Some(false),
            timestamp: Some(chrono::Utc::now()),
            hook_event_name: Some("PostToolUse".into()),
            tool_use_id: None,
        },
    )
    .await
    .unwrap();

    let stats =
        EventRepo::get_verification_phase_tool_call_stats(&pool, session_db_id, before_window)
            .await
            .unwrap();

    assert_eq!(stats.get("cargo_fmt").map(|s| s.total), Some(1));
    assert_eq!(stats.get("cargo_fmt").map(|s| s.successful), Some(1));
}

#[sqlx::test(migrations = "./migrations")]
async fn window_tool_call_stats_excludes_pre_window_events(pool: sqlx::PgPool) {
    use tracevault_server::repo::events::{EventRepo, InsertToolEvent};
    use tracevault_server::repo::sessions::{SessionRepo, UpsertSession};

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;
    let user_id = common::seed_user(&pool).await;

    let session_db_id = SessionRepo::upsert(
        &pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: "test-pre-window-sess".into(),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(chrono::Utc::now()),
        },
    )
    .await
    .unwrap();

    // Event BEFORE the window
    EventRepo::insert_tool_event(
        &pool,
        &InsertToolEvent {
            session_id: session_db_id,
            event_index: Some(1),
            event_uuid: None,
            tool_name: Some("cargo_fmt".into()),
            tool_input: None,
            tool_response: None,
            tool_is_error: Some(false),
            timestamp: Some(chrono::Utc::now()),
            hook_event_name: None,
            tool_use_id: None,
        },
    )
    .await
    .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let window_start = chrono::Utc::now();

    // Stats should be empty — event happened before the window
    let stats =
        EventRepo::get_verification_phase_tool_call_stats(&pool, session_db_id, window_start)
            .await
            .unwrap();

    assert!(stats.is_empty());
}
