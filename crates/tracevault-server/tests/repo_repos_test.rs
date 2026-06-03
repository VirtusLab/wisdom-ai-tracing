mod common;

use tracevault_server::repo::repos::GitRepoRepo;

#[sqlx::test(migrations = "./migrations")]
async fn create_idempotent(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;

    let id1 = GitRepoRepo::create(&pool, org_id, "my-repo", None)
        .await
        .unwrap();
    let id2 = GitRepoRepo::create(
        &pool,
        org_id,
        "my-repo",
        Some("https://github.com/org/repo"),
    )
    .await
    .unwrap();

    assert_eq!(id1, id2);
}

#[sqlx::test(migrations = "./migrations")]
async fn create_updates_url_on_conflict(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    GitRepoRepo::create(&pool, org_id, "repo-url", None)
        .await
        .unwrap();
    GitRepoRepo::create(&pool, org_id, "repo-url", Some("https://new-url.com"))
        .await
        .unwrap();

    let repos = GitRepoRepo::list(&pool, org_id).await.unwrap();
    let repo = repos.iter().find(|r| r.name == "repo-url").unwrap();
    assert_eq!(repo.github_url.as_deref(), Some("https://new-url.com"));
}

#[sqlx::test(migrations = "./migrations")]
async fn list_ordered_by_name(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    GitRepoRepo::create(&pool, org_id, "z-repo", None)
        .await
        .unwrap();
    GitRepoRepo::create(&pool, org_id, "a-repo", None)
        .await
        .unwrap();

    let repos = GitRepoRepo::list(&pool, org_id).await.unwrap();
    assert!(repos[0].name < repos[1].name);
}

#[sqlx::test(migrations = "./migrations")]
async fn set_clone_status_and_list_ready(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = GitRepoRepo::create(&pool, org_id, "sync-repo", None)
        .await
        .unwrap();

    let ready = GitRepoRepo::list_ready_for_sync(&pool).await.unwrap();
    assert!(!ready.iter().any(|r| r.id == repo_id));

    GitRepoRepo::set_clone_status(&pool, repo_id, "ready", Some("/data/repos/123"))
        .await
        .unwrap();

    let ready = GitRepoRepo::list_ready_for_sync(&pool).await.unwrap();
    assert!(ready.iter().any(|r| r.id == repo_id));
}

#[sqlx::test(migrations = "./migrations")]
async fn reset_orphaned_clones_returns_and_unwedges_cloning(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let stuck = GitRepoRepo::create(&pool, org_id, "stuck-repo", Some("git@github.com:o/r.git"))
        .await
        .unwrap();
    let ready = GitRepoRepo::create(&pool, org_id, "ready-repo", None)
        .await
        .unwrap();

    // Orphaned in-flight clone (server died mid-clone) with a stale retry
    // budget, plus a healthy repo that must be left untouched.
    sqlx::query("UPDATE repos SET clone_status = 'cloning', clone_retry_count = 2 WHERE id = $1")
        .bind(stuck)
        .execute(&pool)
        .await
        .unwrap();
    GitRepoRepo::set_clone_status(&pool, ready, "ready", Some("/data/repos/ready"))
        .await
        .unwrap();

    let orphaned = GitRepoRepo::reset_orphaned_clones(&pool).await.unwrap();
    assert_eq!(
        orphaned.len(),
        1,
        "only the stuck 'cloning' repo is returned"
    );
    assert_eq!(orphaned[0].id, stuck);
    assert_eq!(
        orphaned[0].github_url.as_deref(),
        Some("git@github.com:o/r.git"),
        "returned so the caller can re-clone immediately"
    );

    let (status, err, count): (String, Option<String>, i32) = sqlx::query_as(
        "SELECT clone_status, clone_error, clone_retry_count FROM repos WHERE id = $1",
    )
    .bind(stuck)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        status, "pending",
        "un-wedged even if the re-clone never runs"
    );
    assert!(err.is_none(), "error cleared — we re-clone immediately");
    assert_eq!(
        count, 0,
        "retry budget reset for a fresh immediate+backoff cycle"
    );

    let (ready_status,): (String,) = sqlx::query_as("SELECT clone_status FROM repos WHERE id = $1")
        .bind(ready)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ready_status, "ready", "ready repos must be left untouched");
}

#[sqlx::test(migrations = "./migrations")]
async fn claim_clones_for_retry_respects_backoff_and_cap(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;

    // Helper: put a repo into 'error' with a given failure age and retry count.
    async fn set_failed(pool: &sqlx::PgPool, id: uuid::Uuid, mins_ago: i64, retries: i32) {
        sqlx::query(
            "UPDATE repos SET clone_status = 'error', \
             clone_failed_at = now() - ($2 || ' minutes')::interval, \
             clone_retry_count = $3 WHERE id = $1",
        )
        .bind(id)
        .bind(mins_ago.to_string())
        .bind(retries)
        .execute(pool)
        .await
        .unwrap();
    }

    let url = Some("git@github.com:o/r.git");
    let due = GitRepoRepo::create(&pool, org_id, "due", url)
        .await
        .unwrap();
    let fresh = GitRepoRepo::create(&pool, org_id, "fresh", url)
        .await
        .unwrap();
    let exhausted = GitRepoRepo::create(&pool, org_id, "exhausted", url)
        .await
        .unwrap();
    let no_url = GitRepoRepo::create(&pool, org_id, "no-url", None)
        .await
        .unwrap();

    set_failed(&pool, due, 20, 0).await; // 20m old, 0 retries -> due (>15m)
    set_failed(&pool, fresh, 5, 0).await; // 5m old -> not yet due
    set_failed(&pool, exhausted, 120, 2).await; // hit the cap -> never retried
    set_failed(&pool, no_url, 120, 0).await; // no github_url -> skipped

    let claimed = GitRepoRepo::claim_clones_for_retry(&pool).await.unwrap();
    let ids: std::collections::HashSet<_> = claimed.iter().map(|c| c.id).collect();

    assert!(
        ids.contains(&due),
        "a repo past its backoff window is claimed"
    );
    assert!(!ids.contains(&fresh), "a recently-failed repo waits");
    assert!(
        !ids.contains(&exhausted),
        "a repo at the retry cap is dropped"
    );
    assert!(
        !ids.contains(&no_url),
        "a repo without a github_url is skipped"
    );

    // Claiming bumps the counter so an overlapping sweep won't re-claim it.
    let (count,): (i32,) = sqlx::query_as("SELECT clone_retry_count FROM repos WHERE id = $1")
        .bind(due)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // A second-attempt repo (count = 1) needs the longer 30m window.
    set_failed(&pool, fresh, 20, 1).await; // 20m old but < 30m -> not yet due
    let claimed = GitRepoRepo::claim_clones_for_retry(&pool).await.unwrap();
    assert!(
        !claimed.iter().any(|c| c.id == fresh),
        "second retry waits for the longer backoff"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn mark_fetched_updates_timestamp(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let repo_id = GitRepoRepo::create(&pool, org_id, "fetch-repo", None)
        .await
        .unwrap();

    GitRepoRepo::mark_fetched(&pool, repo_id).await.unwrap();

    let (ts,): (Option<chrono::DateTime<chrono::Utc>>,) =
        sqlx::query_as("SELECT last_fetched_at FROM repos WHERE id = $1")
            .bind(repo_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ts.is_some());
}
