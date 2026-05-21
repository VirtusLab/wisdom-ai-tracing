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
            policy_id,
            "trace-complete",
            Some("session-xyz"),
            Some("abcdef1234"),
            result,
            "warn",
            details,
            "cli_check",
            None,
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
        policy_id,
        "will-be-deleted",
        None,
        None,
        "pass",
        "warn",
        "",
        "cli_check",
        None,
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
