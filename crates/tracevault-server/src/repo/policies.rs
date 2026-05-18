use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct PolicyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub repo_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub condition: serde_json::Value,
    pub action: String,
    pub severity: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct PolicyEvaluationRow {
    pub id: Uuid,
    pub policy_id: Option<Uuid>,
    pub policy_name: String,
    pub session_id: Option<String>,
    pub commit_sha: Option<String>,
    pub result: String,
    pub action: String,
    pub details: String,
    pub source: String,
    pub actor_id: Option<Uuid>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct PolicyEvaluationFilter {
    pub policy_id: Option<Uuid>,
    pub result: Option<String>,
    pub source: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}

pub struct PolicyRepo;

impl PolicyRepo {
    /// Check if a repo belongs to the given org.
    pub async fn repo_belongs_to_org(
        pool: &PgPool,
        repo_id: Uuid,
        org_id: Uuid,
    ) -> Result<bool, AppError> {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM repos WHERE id = $1 AND org_id = $2)")
                .bind(repo_id)
                .bind(org_id)
                .fetch_one(pool)
                .await?;
        Ok(exists)
    }

    /// List all policies for an org/repo (repo-specific + org-wide).
    pub async fn list_for_repo(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Vec<PolicyRow>, AppError> {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                Option<Uuid>,
                String,
                String,
                serde_json::Value,
                String,
                String,
                bool,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, org_id, repo_id, name, description, condition, action, severity, enabled, created_at, updated_at
             FROM policies
             WHERE org_id = $1 AND (repo_id = $2 OR repo_id IS NULL)
             ORDER BY created_at",
        )
        .bind(org_id)
        .bind(repo_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PolicyRow {
                id: r.0,
                org_id: r.1,
                repo_id: r.2,
                name: r.3,
                description: r.4,
                condition: r.5,
                action: r.6,
                severity: r.7,
                enabled: r.8,
                created_at: r.9,
                updated_at: r.10,
            })
            .collect())
    }

    /// Create a policy bound to a specific repo.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
        name: &str,
        description: &str,
        condition: &serde_json::Value,
        action: &str,
        severity: &str,
        enabled: bool,
    ) -> Result<(Uuid, DateTime<Utc>, DateTime<Utc>), AppError> {
        let row = sqlx::query_as::<_, (Uuid, DateTime<Utc>, DateTime<Utc>)>(
            "INSERT INTO policies (org_id, repo_id, name, description, condition, action, severity, enabled)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id, created_at, updated_at",
        )
        .bind(org_id)
        .bind(repo_id)
        .bind(name)
        .bind(description)
        .bind(condition)
        .bind(action)
        .bind(severity)
        .bind(enabled)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    /// Update a policy (COALESCE for partial updates). Returns None if not found.
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        org_id: Uuid,
        name: &Option<String>,
        description: &Option<String>,
        condition: &Option<serde_json::Value>,
        action: &Option<String>,
        severity: &Option<String>,
        enabled: Option<bool>,
    ) -> Result<Option<PolicyRow>, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<Uuid>,
                String,
                String,
                serde_json::Value,
                String,
                String,
                bool,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            "UPDATE policies SET
                name = COALESCE($3, name),
                description = COALESCE($4, description),
                condition = COALESCE($5, condition),
                action = COALESCE($6, action),
                severity = COALESCE($7, severity),
                enabled = COALESCE($8, enabled),
                updated_at = NOW()
             WHERE id = $1 AND org_id = $2
             RETURNING org_id, repo_id, name, description, condition, action, severity, enabled, created_at, updated_at",
        )
        .bind(id)
        .bind(org_id)
        .bind(name)
        .bind(description)
        .bind(condition)
        .bind(action)
        .bind(severity)
        .bind(enabled)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| PolicyRow {
            id,
            org_id: r.0,
            repo_id: r.1,
            name: r.2,
            description: r.3,
            condition: r.4,
            action: r.5,
            severity: r.6,
            enabled: r.7,
            created_at: r.8,
            updated_at: r.9,
        }))
    }

    /// Delete a policy. Returns the number of rows affected (0 or 1).
    pub async fn delete(pool: &PgPool, id: Uuid, org_id: Uuid) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM policies WHERE id = $1 AND org_id = $2")
            .bind(id)
            .bind(org_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Fetch all enabled policies for a repo (repo-specific + org-wide) for evaluation.
    /// Returns (id, name, condition, action, severity) per rule.
    pub async fn list_enabled_for_check(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Vec<(Uuid, String, serde_json::Value, String, String)>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, String, serde_json::Value, String, String)>(
            "SELECT id, name, condition, action, severity
             FROM policies
             WHERE org_id = $1 AND (repo_id = $2 OR repo_id IS NULL) AND enabled = true
             ORDER BY created_at",
        )
        .bind(org_id)
        .bind(repo_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// List recent policy evaluations for a repo with optional filters.
    pub async fn list_evaluations(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
        filter: &PolicyEvaluationFilter,
    ) -> Result<Vec<PolicyEvaluationRow>, AppError> {
        // Dynamic WHERE without a query builder crate: each filter is a
        // nullable bind that the SQL treats as "no filter" when NULL.
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<Uuid>,
                String,
                Option<String>,
                Option<String>,
                String,
                String,
                String,
                String,
                Option<Uuid>,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, policy_id, policy_name, session_id, commit_sha,
                    result, action, details, source, actor_id, evaluated_at
             FROM policy_evaluations
             WHERE org_id = $1 AND repo_id = $2
               AND ($3::uuid IS NULL OR policy_id = $3)
               AND ($4::text IS NULL OR result = $4)
               AND ($5::text IS NULL OR source = $5)
               AND ($6::timestamptz IS NULL OR evaluated_at >= $6)
             ORDER BY evaluated_at DESC
             LIMIT $7 OFFSET $8",
        )
        .bind(org_id)
        .bind(repo_id)
        .bind(filter.policy_id)
        .bind(filter.result.as_deref())
        .bind(filter.source.as_deref())
        .bind(filter.since)
        .bind(filter.limit)
        .bind(filter.offset)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| PolicyEvaluationRow {
                id: r.0,
                policy_id: r.1,
                policy_name: r.2,
                session_id: r.3,
                commit_sha: r.4,
                result: r.5,
                action: r.6,
                details: r.7,
                source: r.8,
                actor_id: r.9,
                evaluated_at: r.10,
            })
            .collect())
    }

    /// Count policy evaluations matching the same filters (no limit/offset).
    pub async fn count_evaluations(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
        filter: &PolicyEvaluationFilter,
    ) -> Result<i64, AppError> {
        let count: i64 = sqlx::query_scalar(include_str!("sql/count_evaluations.sql"))
        .bind(org_id)
        .bind(repo_id)
        .bind(filter.policy_id)
        .bind(filter.result.as_deref())
        .bind(filter.source.as_deref())
        .bind(filter.since)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    /// Persist a single policy evaluation. Best-effort: callers should log
    /// and continue on failure so a DB hiccup doesn't turn a passing push
    /// into a blocked one.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_evaluation(
        pool: &PgPool,
        org_id: Uuid,
        repo_id: Uuid,
        policy_id: Uuid,
        policy_name: &str,
        session_id: Option<&str>,
        commit_sha: Option<&str>,
        result: &str,
        action: &str,
        details: &str,
        source: &str,
        actor_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO policy_evaluations
               (org_id, repo_id, policy_id, policy_name, session_id, commit_sha,
                result, action, details, source, actor_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(org_id)
        .bind(repo_id)
        .bind(policy_id)
        .bind(policy_name)
        .bind(session_id)
        .bind(commit_sha)
        .bind(result)
        .bind(action)
        .bind(details)
        .bind(source)
        .bind(actor_id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
