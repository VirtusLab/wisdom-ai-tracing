use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct RepoRow {
    pub id: Uuid,
    pub name: String,
    pub github_url: Option<String>,
    pub clone_status: String,
    pub clone_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ReadyRepo {
    pub id: Uuid,
    pub deploy_key_encrypted: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct RetryableClone {
    pub id: Uuid,
    pub github_url: Option<String>,
    pub deploy_key_encrypted: Option<String>,
}

/// Max automatic retries per error streak before giving up (manual sync still
/// works). Backoff before each retry: 1st after [`RETRY_DELAY_1`], 2nd after
/// [`RETRY_DELAY_2`]. Both are SQL `interval` literals.
const MAX_CLONE_RETRIES: i32 = 2;
const RETRY_DELAY_1: &str = "15 minutes";
const RETRY_DELAY_2: &str = "30 minutes";

pub struct GitRepoRepo;

impl GitRepoRepo {
    pub async fn create(
        pool: &PgPool,
        org_id: Uuid,
        name: &str,
        github_url: Option<&str>,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO repos (org_id, name, github_url) VALUES ($1, $2, $3) \
             ON CONFLICT (org_id, name) DO UPDATE SET github_url = COALESCE(EXCLUDED.github_url, repos.github_url) \
             RETURNING id",
        )
        .bind(org_id)
        .bind(name)
        .bind(github_url)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn list(pool: &PgPool, org_id: Uuid) -> Result<Vec<RepoRow>, AppError> {
        let rows = sqlx::query_as::<_, RepoRow>(
            "SELECT id, name, github_url, clone_status, clone_error, created_at \
             FROM repos WHERE org_id = $1 ORDER BY name",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_ready_for_sync(pool: &PgPool) -> Result<Vec<ReadyRepo>, AppError> {
        let rows = sqlx::query_as::<_, ReadyRepo>(
            "SELECT id, deploy_key_encrypted FROM repos WHERE clone_status = 'ready'",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Recover clones orphaned by a server restart, returning them so the
    /// caller can re-clone immediately.
    ///
    /// A clone runs in a detached in-process task that cannot survive a restart,
    /// so on startup any row still in 'cloning' is orphaned. Reset it to
    /// 'pending' with a fresh retry budget — so it is un-wedged even if the
    /// immediate re-clone never runs — and hand the repos back to be re-cloned
    /// right away; if that fails, the normal backoff retry takes over.
    pub async fn reset_orphaned_clones(pool: &PgPool) -> Result<Vec<RetryableClone>, AppError> {
        let rows = sqlx::query_as::<_, RetryableClone>(
            "UPDATE repos \
             SET clone_status = 'pending', \
                 clone_error = NULL, \
                 clone_started_at = NULL, \
                 clone_retry_count = 0 \
             WHERE clone_status = 'cloning' \
             RETURNING id, github_url, deploy_key_encrypted",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Atomically claim failed clones that are due for an automatic retry,
    /// bumping their retry counter so concurrent/overlapping sweeps can't
    /// double-claim the same repo. A repo is due when it is in 'error' with a
    /// github_url, still under the retry cap, and its last failure is older than
    /// the backoff window for its current attempt number. Returns the claimed
    /// repos for the caller to re-clone.
    pub async fn claim_clones_for_retry(pool: &PgPool) -> Result<Vec<RetryableClone>, AppError> {
        let rows = sqlx::query_as::<_, RetryableClone>(
            "UPDATE repos SET clone_retry_count = clone_retry_count + 1 \
             WHERE id IN ( \
                 SELECT id FROM repos \
                 WHERE clone_status = 'error' \
                   AND github_url IS NOT NULL \
                   AND clone_failed_at IS NOT NULL \
                   AND clone_retry_count < $1 \
                   AND clone_failed_at < now() - (CASE WHEN clone_retry_count = 0 \
                       THEN $2::interval ELSE $3::interval END) \
             ) \
             RETURNING id, github_url, deploy_key_encrypted",
        )
        .bind(MAX_CLONE_RETRIES)
        .bind(RETRY_DELAY_1)
        .bind(RETRY_DELAY_2)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    pub async fn mark_fetched(pool: &PgPool, repo_id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE repos SET last_fetched_at = now() WHERE id = $1")
            .bind(repo_id)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn set_clone_status(
        pool: &PgPool,
        repo_id: Uuid,
        status: &str,
        clone_path: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE repos SET clone_status = $2, clone_path = COALESCE($3, clone_path) WHERE id = $1",
        )
        .bind(repo_id)
        .bind(status)
        .bind(clone_path)
        .execute(pool)
        .await?;

        Ok(())
    }
}
