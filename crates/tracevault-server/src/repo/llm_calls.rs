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
}

pub struct LlmCallRepo;

impl LlmCallRepo {
    pub async fn insert(pool: &PgPool, rec: &LlmCallRecord) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO llm_calls (
                org_id, user_id, credential_id, auth_session_id, client_session_id,
                repo_id, branch, requested_model, provider_model, response_model,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens,
                total_tokens, estimated_cost_usd, stop_reason, http_status, outcome,
                duration_ms, anthropic_request_id, path
             ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22
             )
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
        .fetch_one(pool)
        .await?;
        Ok(id)
    }
}
