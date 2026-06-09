use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct ComplianceSettingsRow {
    pub org_id: Uuid,
    pub retention_days: i32,
    pub signing_enabled: bool,
    pub chain_verification_interval_hours: Option<i32>,
    pub compliance_mode: Option<String>,
    pub usage_source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ComplianceRepo;

impl ComplianceRepo {
    // ── Settings ──────────────────────────────────────────────────────

    pub async fn get_settings(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<Option<ComplianceSettingsRow>, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                i32,
                bool,
                Option<i32>,
                Option<String>,
                String,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            "SELECT org_id, retention_days, signing_enabled,
                    chain_verification_interval_hours, compliance_mode, usage_source, created_at, updated_at
             FROM org_compliance_settings WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| ComplianceSettingsRow {
            org_id: r.0,
            retention_days: r.1,
            signing_enabled: r.2,
            chain_verification_interval_hours: r.3,
            compliance_mode: r.4,
            usage_source: r.5,
            created_at: r.6,
            updated_at: r.7,
        }))
    }

    pub async fn upsert_settings(
        pool: &PgPool,
        org_id: Uuid,
        retention_days: i32,
        signing_enabled: bool,
        chain_verification_interval_hours: Option<i32>,
        compliance_mode: &str,
        usage_source: Option<&str>,
    ) -> Result<ComplianceSettingsRow, AppError> {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                i32,
                bool,
                Option<i32>,
                Option<String>,
                String,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            "INSERT INTO org_compliance_settings (org_id, retention_days, signing_enabled, chain_verification_interval_hours, compliance_mode, usage_source)
             VALUES ($1, $2, $3, $4, $5, COALESCE($6, 'both'))
             ON CONFLICT (org_id) DO UPDATE SET
               retention_days = COALESCE($2, org_compliance_settings.retention_days),
               signing_enabled = COALESCE($3, org_compliance_settings.signing_enabled),
               chain_verification_interval_hours = COALESCE($4, org_compliance_settings.chain_verification_interval_hours),
               compliance_mode = COALESCE($5, org_compliance_settings.compliance_mode),
               usage_source = COALESCE($6, org_compliance_settings.usage_source),
               updated_at = NOW()
             RETURNING org_id, retention_days, signing_enabled, chain_verification_interval_hours, compliance_mode, usage_source, created_at, updated_at",
        )
        .bind(org_id)
        .bind(retention_days)
        .bind(signing_enabled)
        .bind(chain_verification_interval_hours)
        .bind(compliance_mode)
        .bind(usage_source)
        .fetch_one(pool)
        .await?;

        Ok(ComplianceSettingsRow {
            org_id: row.0,
            retention_days: row.1,
            signing_enabled: row.2,
            chain_verification_interval_hours: row.3,
            compliance_mode: row.4,
            usage_source: row.5,
            created_at: row.6,
            updated_at: row.7,
        })
    }

    // ── Chain Verification ───────────────────────────────────────────

    /// Get the latest chain verification status for an org.
    pub async fn get_chain_status(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<
        Option<(
            String,                    // status
            i32,                       // total_commits
            i32,                       // verified_commits
            i32,                       // total_sessions
            i32,                       // verified_sessions
            Option<serde_json::Value>, // errors
            Option<DateTime<Utc>>,     // completed_at
        )>,
        AppError,
    > {
        let row = sqlx::query_as::<
            _,
            (
                String,
                i32,
                i32,
                i32,
                i32,
                Option<serde_json::Value>,
                Option<DateTime<Utc>>,
            ),
        >(
            "SELECT status, total_commits, verified_commits, total_sessions, verified_sessions, errors, completed_at
             FROM chain_verifications WHERE org_id = $1
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(org_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Fetch all sealed commits for chain verification, ordered by seal time.
    pub async fn get_sealed_commits_for_verification(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<
        Vec<(
            Uuid,           // commit id
            Option<String>, // record_hash
            Option<String>, // chain_hash
            Option<String>, // prev_chain_hash
            Option<String>, // signature
            DateTime<Utc>,  // sealed_at
        )>,
        AppError,
    > {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                DateTime<Utc>,
            ),
        >(
            "SELECT c.id, cs.record_hash, cs.chain_hash, cs.prev_chain_hash, cs.signature, cs.sealed_at
             FROM commits c
             JOIN commit_seals cs ON cs.commit_id = c.id
             JOIN repos r ON c.repo_id = r.id
             WHERE r.org_id = $1
             ORDER BY cs.sealed_at ASC, c.created_at ASC, c.id ASC",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Insert a chain verification result.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_chain_verification(
        pool: &PgPool,
        org_id: Uuid,
        status: &str,
        total: i32,
        verified: i32,
        total_sessions: i32,
        verified_sessions: i32,
        errors: &Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO chain_verifications (org_id, status, total_commits, verified_commits, total_sessions, verified_sessions, errors, started_at, completed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())",
        )
        .bind(org_id)
        .bind(status)
        .bind(total)
        .bind(verified)
        .bind(total_sessions)
        .bind(verified_sessions)
        .bind(errors)
        .execute(pool)
        .await?;

        Ok(())
    }

    // ── Audit Log ────────────────────────────────────────────────────

    /// List distinct action values from the audit log for an org.
    pub async fn distinct_audit_actions(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT action FROM audit_log WHERE org_id = $1 ORDER BY action",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Count audit log entries with optional filters.
    pub async fn count_audit_log(
        pool: &PgPool,
        org_id: Uuid,
        action: &Option<String>,
        actor_id: Option<Uuid>,
        resource_type: &Option<String>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
    ) -> Result<i64, AppError> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log
             WHERE org_id = $1
               AND ($2::TEXT IS NULL OR action = $2)
               AND ($3::UUID IS NULL OR actor_id = $3)
               AND ($4::TEXT IS NULL OR resource_type = $4)
               AND ($5::TIMESTAMPTZ IS NULL OR created_at >= $5)
               AND ($6::TIMESTAMPTZ IS NULL OR created_at <= $6)",
        )
        .bind(org_id)
        .bind(action)
        .bind(actor_id)
        .bind(resource_type)
        .bind(from)
        .bind(to)
        .fetch_one(pool)
        .await?;

        Ok(total)
    }

    /// List audit log entries with optional filters and pagination.
    #[allow(clippy::too_many_arguments)]
    pub async fn list_audit_log(
        pool: &PgPool,
        org_id: Uuid,
        action: &Option<String>,
        actor_id: Option<Uuid>,
        resource_type: &Option<String>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        limit: i64,
        offset: i64,
    ) -> Result<
        Vec<(
            Uuid,                      // id
            Option<Uuid>,              // actor_id
            String,                    // action
            String,                    // resource_type
            Option<Uuid>,              // resource_id
            Option<serde_json::Value>, // details
            Option<String>,            // ip_address
            Option<String>,            // user_agent
            DateTime<Utc>,             // created_at
        )>,
        AppError,
    > {
        let rows = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<Uuid>,
                String,
                String,
                Option<Uuid>,
                Option<serde_json::Value>,
                Option<String>,
                Option<String>,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, actor_id, action, resource_type, resource_id, details,
                    host(ip_address)::TEXT, user_agent, created_at
             FROM audit_log
             WHERE org_id = $1
               AND ($2::TEXT IS NULL OR action = $2)
               AND ($3::UUID IS NULL OR actor_id = $3)
               AND ($4::TEXT IS NULL OR resource_type = $4)
               AND ($5::TIMESTAMPTZ IS NULL OR created_at >= $5)
               AND ($6::TIMESTAMPTZ IS NULL OR created_at <= $6)
             ORDER BY created_at DESC
             LIMIT $7 OFFSET $8",
        )
        .bind(org_id)
        .bind(action)
        .bind(actor_id)
        .bind(resource_type)
        .bind(from)
        .bind(to)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    // ── Trace Verification ───────────────────────────────────────────

    /// Fetch a commit seal by commit SHA and org for individual verification.
    pub async fn get_commit_seal_by_sha(
        pool: &PgPool,
        sha: &str,
        org_id: Uuid,
    ) -> Result<
        Option<(
            Uuid,                  // commit_id
            Option<String>,        // record_hash
            Option<String>,        // chain_hash
            Option<String>,        // prev_chain_hash
            Option<String>,        // signature
            Option<DateTime<Utc>>, // sealed_at
        )>,
        AppError,
    > {
        let row = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<DateTime<Utc>>,
            ),
        >(
            "SELECT c.id, cs.record_hash, cs.chain_hash, cs.prev_chain_hash, cs.signature, cs.sealed_at
             FROM commits c
             JOIN repos r ON c.repo_id = r.id
             LEFT JOIN commit_seals cs ON cs.commit_id = c.id
             WHERE c.commit_sha = $1 AND r.org_id = $2",
        )
        .bind(sha)
        .bind(org_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    // ── Org LLM Settings ─────────────────────────────────────────────

    /// Fetch the LLM config for an org (used for code story generation).
    pub async fn get_org_llm_settings(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<
        Option<(
            Option<String>, // llm_provider
            Option<String>, // llm_api_key_encrypted
            Option<String>, // llm_api_key_nonce
            Option<String>, // llm_model
            Option<String>, // llm_base_url
        )>,
        AppError,
    > {
        let row = sqlx::query_as::<
            _,
            (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            "SELECT llm_provider, llm_api_key_encrypted, llm_api_key_nonce, llm_model, llm_base_url
             FROM org_compliance_settings WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }
}
