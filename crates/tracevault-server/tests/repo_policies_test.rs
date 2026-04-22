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
        false,
    )
    .await
    .unwrap();

    let enabled = PolicyRepo::list_enabled_for_check(&pool, org_id, repo_id)
        .await
        .unwrap();
    assert!(enabled
        .iter()
        .all(|(_, name, _, _, _)| name != "disabled-policy"));
    assert!(enabled
        .iter()
        .any(|(_, name, _, _, _)| name == "enabled-policy"));
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

#[sqlx::test(migrations = "./migrations")]
async fn stats_aggregate_and_include_silent_rules(pool: sqlx::PgPool) {
    use chrono::Utc;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    // Rule A: has a mix of results.
    let (rule_a, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "has-evals",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        true,
    )
    .await
    .unwrap();

    // Rule B: zero evaluations — must still appear in stats ("silent rule").
    PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "silent",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "low",
        true,
    )
    .await
    .unwrap();

    for result in ["pass", "pass", "pass", "fail", "skip"] {
        PolicyRepo::insert_evaluation(
            &pool,
            org_id,
            repo_id,
            rule_a,
            "has-evals",
            None,
            None,
            result,
            "warn",
            "",
            "cli_check",
            None,
        )
        .await
        .unwrap();
    }

    let since = Utc::now() - chrono::Duration::days(30);
    let stats = PolicyRepo::policy_stats(&pool, org_id, repo_id, since)
        .await
        .unwrap();

    // Both rules appear, ordered by total DESC then name.
    assert_eq!(stats.len(), 2);
    let a = stats.iter().find(|s| s.policy_name == "has-evals").unwrap();
    assert_eq!(a.total, 5);
    assert_eq!(a.pass_count, 3);
    assert_eq!(a.fail_count, 1);
    assert_eq!(a.skip_count, 1);
    assert!(a.last_evaluated_at.is_some());

    let b = stats.iter().find(|s| s.policy_name == "silent").unwrap();
    assert_eq!(b.total, 0);
    assert!(b.last_evaluated_at.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn stats_respects_since_cutoff(pool: sqlx::PgPool) {
    use chrono::Utc;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (rule_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "r",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        true,
    )
    .await
    .unwrap();

    // One old row (45d ago), one recent.
    sqlx::query(
        "INSERT INTO policy_evaluations
           (org_id, repo_id, policy_id, policy_name, result, action, source, evaluated_at)
         VALUES ($1, $2, $3, 'r', 'pass', 'warn', 'cli_check', NOW() - INTERVAL '45 days')",
    )
    .bind(org_id)
    .bind(repo_id)
    .bind(rule_id)
    .execute(&pool)
    .await
    .unwrap();

    PolicyRepo::insert_evaluation(
        &pool,
        org_id,
        repo_id,
        rule_id,
        "r",
        None,
        None,
        "fail",
        "warn",
        "",
        "cli_check",
        None,
    )
    .await
    .unwrap();

    let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
    let stats = PolicyRepo::policy_stats(&pool, org_id, repo_id, thirty_days_ago)
        .await
        .unwrap();

    // Only the recent fail should count.
    let s = stats.iter().find(|s| s.policy_name == "r").unwrap();
    assert_eq!(s.total, 1);
    assert_eq!(s.fail_count, 1);
    assert_eq!(s.pass_count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn purge_evaluations_drops_old_rows(pool: sqlx::PgPool) {
    use chrono::Utc;

    let org_id = common::seed_org(&pool).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let (rule_id, _, _) = PolicyRepo::create(
        &pool,
        org_id,
        repo_id,
        "r",
        "",
        &json!({"type": "TraceCompleteness"}),
        "warn",
        "medium",
        true,
    )
    .await
    .unwrap();

    // Seed 3 old rows (older than cutoff) + 2 recent.
    for _ in 0..3 {
        sqlx::query(
            "INSERT INTO policy_evaluations
               (org_id, repo_id, policy_id, policy_name, result, action, source, evaluated_at)
             VALUES ($1, $2, $3, 'r', 'pass', 'warn', 'cli_check', NOW() - INTERVAL '100 days')",
        )
        .bind(org_id)
        .bind(repo_id)
        .bind(rule_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    for _ in 0..2 {
        PolicyRepo::insert_evaluation(
            &pool,
            org_id,
            repo_id,
            rule_id,
            "r",
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
    }

    let cutoff = Utc::now() - chrono::Duration::days(90);
    let purged = PolicyRepo::purge_evaluations_older_than(&pool, cutoff)
        .await
        .unwrap();
    assert_eq!(purged, 3);

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM policy_evaluations WHERE repo_id = $1")
            .bind(repo_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 2);
}
