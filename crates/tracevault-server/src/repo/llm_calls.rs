use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// One assembled ledger row, ready to insert. Token fields are stored verbatim
/// from the API usage object (input_tokens is already cache-excluded / fresh).
#[derive(Debug, Clone)]
pub struct LlmCallRecord {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Option<Uuid>,
    pub auth_session_id: Option<Uuid>,
    pub client_session_id: Option<String>,
    pub repo_id: Option<Uuid>,
    pub branch: Option<String>,
    pub requested_model: Option<String>,
    pub provider_model: Option<String>,
    pub response_model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub stop_reason: Option<String>,
    pub http_status: i32,
    pub outcome: String,
    pub duration_ms: i64,
    pub anthropic_request_id: Option<String>,
    pub path: String,
    pub anthropic_message_id: Option<String>,
}

/// Scalar token/cost sums over llm_calls for the analytics filters. Filters
/// mirror the sessions queries: repo (by repos.name), author (by users.email),
/// and the from/to window on created_at. The repo filter excludes header-less
/// proxy rows (NULL repo_id) because they cannot match a name.
#[derive(Debug, Clone, Default)]
pub struct LedgerKpis {
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
}

pub struct LlmCallRepo;

impl LlmCallRepo {
    /// Aggregate ledger token/cost sums for the analytics filters (DB read).
    pub async fn fetch_ledger_kpis(
        pool: &PgPool,
        org_id: Uuid,
        repo: Option<&str>,
        author: Option<&str>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
        dedup: bool,
    ) -> Result<LedgerKpis, AppError> {
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, f64)>(
            "SELECT
                COALESCE(SUM(COALESCE(c.total_tokens,0)),0)::BIGINT,
                COALESCE(SUM(COALESCE(c.input_tokens,0)),0)::BIGINT,
                COALESCE(SUM(COALESCE(c.output_tokens,0)),0)::BIGINT,
                COALESCE(SUM(COALESCE(c.cache_read_tokens,0)),0)::BIGINT,
                COALESCE(SUM(COALESCE(c.cache_write_tokens,0)),0)::BIGINT,
                COALESCE(SUM(COALESCE(c.estimated_cost_usd,0.0)),0.0)
             FROM llm_calls c
             LEFT JOIN repos r ON c.repo_id = r.id
             LEFT JOIN users u ON c.user_id = u.id
             WHERE c.org_id = $1
               AND ($2::TEXT IS NULL OR r.name = $2)
               AND ($3::TEXT IS NULL OR u.email = $3)
               AND ($4::TIMESTAMPTZ IS NULL OR c.created_at >= $4)
               AND ($5::TIMESTAMPTZ IS NULL OR c.created_at <= $5)
               AND ($6 = FALSE OR NOT EXISTS (
                     SELECT 1 FROM session_message_ids sm
                     WHERE sm.anthropic_message_id = c.anthropic_message_id
                       AND sm.org_id = c.org_id))",
        )
        .bind(org_id)
        .bind(repo)
        .bind(author)
        .bind(from)
        .bind(to)
        .bind(dedup)
        .fetch_one(pool)
        .await?;
        Ok(LedgerKpis {
            total_tokens: row.0,
            input_tokens: row.1,
            output_tokens: row.2,
            cache_read_tokens: row.3,
            cache_write_tokens: row.4,
            cost_usd: row.5,
        })
    }

    /// Insert one ledger row. Returns the new id, or `None` if a row with the
    /// same `anthropic_request_id` already exists (idempotent no-op against the
    /// partial unique index on `anthropic_request_id`) — so a retried/replayed
    /// request_id is silently skipped rather than erroring.
    pub async fn insert(pool: &PgPool, rec: &LlmCallRecord) -> Result<Option<Uuid>, AppError> {
        let id: Option<Uuid> = sqlx::query_scalar(
            "INSERT INTO llm_calls (
                org_id, user_id, credential_id, auth_session_id, client_session_id,
                repo_id, branch, requested_model, provider_model, response_model,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                total_tokens, estimated_cost_usd, stop_reason, http_status, outcome,
                duration_ms, anthropic_request_id, path, anthropic_message_id
             ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23
             )
             ON CONFLICT (anthropic_request_id) WHERE anthropic_request_id IS NOT NULL DO NOTHING
             RETURNING id",
        )
        .bind(rec.org_id)
        .bind(rec.user_id)
        .bind(rec.credential_id)
        .bind(rec.auth_session_id)
        .bind(&rec.client_session_id)
        .bind(rec.repo_id)
        .bind(&rec.branch)
        .bind(&rec.requested_model)
        .bind(&rec.provider_model)
        .bind(&rec.response_model)
        .bind(rec.input_tokens)
        .bind(rec.output_tokens)
        .bind(rec.cache_read_tokens)
        .bind(rec.cache_write_tokens)
        .bind(rec.total_tokens)
        .bind(rec.estimated_cost_usd)
        .bind(&rec.stop_reason)
        .bind(rec.http_status)
        .bind(&rec.outcome)
        .bind(rec.duration_ms)
        .bind(&rec.anthropic_request_id)
        .bind(&rec.path)
        .bind(&rec.anthropic_message_id)
        .fetch_optional(pool)
        .await?;
        Ok(id)
    }
}
