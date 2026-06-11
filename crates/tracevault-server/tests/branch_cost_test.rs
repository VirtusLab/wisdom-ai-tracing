mod common;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

// Run the real branches query against seeded data.
const GET_BRANCHES_SQL: &str = include_str!("../src/api/traces_ui/sql/get_branches.sql");

type BranchRow = (
    String,                // branch
    String,                // repo_name
    Option<String>,        // tag
    i64,                   // commits_count
    i64,                   // sessions_count
    Option<f64>,           // total_cost
    String,                // status
    Option<DateTime<Utc>>, // last_activity
);

/// Regression for #189: a branch's total_cost must sum each attributed session
/// exactly once. The old `SUM(DISTINCT s.estimated_cost_usd)` deduped by cost
/// *value*, collapsing equal-cost sessions; a plain `SUM` would over-count a
/// session attributed to multiple commits. The fix must do neither.
#[sqlx::test(migrations = "./migrations")]
async fn branch_cost_sums_each_session_once(pool: PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    // Two sessions with the SAME cost (0.50) — the value-dedup bug collapses
    // these to a single 0.50.
    let s_a = common::seed_session(&pool, org_id, repo_id, user_id).await;
    let s_b = common::seed_session(&pool, org_id, repo_id, user_id).await;
    for s in [s_a, s_b] {
        sqlx::query("UPDATE sessions SET estimated_cost_usd = 0.50 WHERE id = $1")
            .bind(s)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Branch "feature" with three commits.
    let c1 = common::seed_commit(&pool, repo_id, "sha1").await;
    let c2 = common::seed_commit(&pool, repo_id, "sha2").await;
    let c3 = common::seed_commit(&pool, repo_id, "sha3").await;
    for c in [c1, c2, c3] {
        sqlx::query(
            "INSERT INTO branch_tracking (commit_id, branch, tracking_type, tracked_at) \
             VALUES ($1, 'feature', 'commit', now())",
        )
        .bind(c)
        .execute(&pool)
        .await
        .unwrap();
    }

    // s_a is attributed to TWO commits (must still count once); s_b to one.
    for (c, s) in [(c1, s_a), (c2, s_a), (c3, s_b)] {
        sqlx::query(
            "INSERT INTO commit_attributions (commit_id, session_id, file_path, confidence) \
             VALUES ($1, $2, 'f.rs', 1.0)",
        )
        .bind(c)
        .bind(s)
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = sqlx::query_as::<_, BranchRow>(GET_BRANCHES_SQL)
        .bind(org_id)
        .bind(Option::<Uuid>::None)
        .fetch_all(&pool)
        .await
        .unwrap();

    let feature = rows
        .iter()
        .find(|r| r.0 == "feature")
        .expect("feature branch present");

    // s_a (0.50) + s_b (0.50) = 1.00 — NOT 0.50 (value-dedup collapse) and NOT
    // 1.50 (s_a double-counted across c1 and c2).
    let cost = feature.5.unwrap_or(0.0);
    assert!(
        (cost - 1.00).abs() < 1e-9,
        "expected total_cost 1.00, got {cost}"
    );
    assert_eq!(
        feature.4, 2,
        "two distinct sessions attributed to the branch"
    );
}

/// A branch whose sessions all have $0.00 cost (pricing not configured) must
/// still report each session — count is unaffected and cost is a true 0.00.
#[sqlx::test(migrations = "./migrations")]
async fn branch_cost_zero_cost_sessions_still_counted(pool: PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    let repo_id = common::seed_repo(&pool, org_id).await;

    let s_a = common::seed_session(&pool, org_id, repo_id, user_id).await;
    let s_b = common::seed_session(&pool, org_id, repo_id, user_id).await;
    for s in [s_a, s_b] {
        sqlx::query("UPDATE sessions SET estimated_cost_usd = 0.0 WHERE id = $1")
            .bind(s)
            .execute(&pool)
            .await
            .unwrap();
    }

    let c1 = common::seed_commit(&pool, repo_id, "z1").await;
    let c2 = common::seed_commit(&pool, repo_id, "z2").await;
    for c in [c1, c2] {
        sqlx::query(
            "INSERT INTO branch_tracking (commit_id, branch, tracking_type, tracked_at) \
             VALUES ($1, 'zero', 'commit', now())",
        )
        .bind(c)
        .execute(&pool)
        .await
        .unwrap();
    }
    for (c, s) in [(c1, s_a), (c2, s_b)] {
        sqlx::query(
            "INSERT INTO commit_attributions (commit_id, session_id, file_path, confidence) \
             VALUES ($1, $2, 'f.rs', 1.0)",
        )
        .bind(c)
        .bind(s)
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = sqlx::query_as::<_, BranchRow>(GET_BRANCHES_SQL)
        .bind(org_id)
        .bind(Option::<Uuid>::None)
        .fetch_all(&pool)
        .await
        .unwrap();
    let zero = rows
        .iter()
        .find(|r| r.0 == "zero")
        .expect("zero branch present");
    assert_eq!(
        zero.5.unwrap_or(-1.0),
        0.0,
        "all-zero-cost branch totals 0.00"
    );
    assert_eq!(zero.4, 2, "both sessions counted");
}
