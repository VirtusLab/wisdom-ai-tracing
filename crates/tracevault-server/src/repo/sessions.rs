use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracevault_core::streaming::SessionFinalStats;
use uuid::Uuid;

use crate::error::AppError;

pub struct UpsertSession {
    pub org_id: Uuid,
    pub repo_id: Uuid,
    pub user_id: Uuid,
    pub session_id: String,
    pub model: Option<String>,
    pub cwd: Option<String>,
    pub tool: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

pub struct TokenBatch {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub estimated_cost_usd: f64,
    pub model: Option<String>,
}

pub struct SessionRepo;

impl SessionRepo {
    /// INSERT INTO sessions ... ON CONFLICT (repo_id, session_id) DO UPDATE ... RETURNING id
    pub async fn upsert(pool: &PgPool, req: &UpsertSession) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO sessions (org_id, repo_id, user_id, session_id, model, cwd, tool, started_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (repo_id, session_id) DO UPDATE SET
                 updated_at = now(),
                 model = COALESCE(EXCLUDED.model, sessions.model),
                 cwd = COALESCE(EXCLUDED.cwd, sessions.cwd),
                 status = CASE WHEN sessions.status = 'completed' THEN 'active' ELSE sessions.status END
             RETURNING id",
        )
        .bind(req.org_id)
        .bind(req.repo_id)
        .bind(req.user_id)
        .bind(&req.session_id)
        .bind(&req.model)
        .bind(&req.cwd)
        .bind(&req.tool)
        .bind(req.timestamp)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    /// UPDATE sessions token counters and estimated cost.
    pub async fn update_tokens(
        pool: &PgPool,
        session_id: Uuid,
        batch: &TokenBatch,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE sessions SET
                input_tokens = input_tokens + $2,
                output_tokens = output_tokens + $3,
                cache_read_tokens = cache_read_tokens + $4,
                cache_write_tokens = cache_write_tokens + $5,
                total_tokens = total_tokens + $2 + $3 + $4 + $5,
                estimated_cost_usd = estimated_cost_usd + $6,
                model = COALESCE($7, model),
                updated_at = now()
             WHERE id = $1",
        )
        .bind(session_id)
        .bind(batch.input_tokens)
        .bind(batch.output_tokens)
        .bind(batch.cache_read_tokens)
        .bind(batch.cache_write_tokens)
        .bind(batch.estimated_cost_usd)
        .bind(&batch.model)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Increment total_tool_calls by 1.
    pub async fn increment_tool_calls(pool: &PgPool, session_id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET total_tool_calls = total_tool_calls + 1 WHERE id = $1")
            .bind(session_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Mark session completed with final stats from the client.
    pub async fn complete_with_stats(
        pool: &PgPool,
        session_id: Uuid,
        ended_at: Option<DateTime<Utc>>,
        stats: &SessionFinalStats,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE sessions SET
                 status = 'completed',
                 ended_at = $2,
                 duration_ms = COALESCE($3, duration_ms),
                 total_tokens = COALESCE($4, total_tokens),
                 input_tokens = COALESCE($5, input_tokens),
                 output_tokens = COALESCE($6, output_tokens),
                 cache_read_tokens = COALESCE($7, cache_read_tokens),
                 cache_write_tokens = COALESCE($8, cache_write_tokens),
                 user_messages = COALESCE($9, user_messages),
                 assistant_messages = COALESCE($10, assistant_messages),
                 total_tool_calls = COALESCE($11, total_tool_calls)
             WHERE id = $1",
        )
        .bind(session_id)
        .bind(ended_at)
        .bind(stats.duration_ms)
        .bind(stats.total_tokens)
        .bind(stats.input_tokens)
        .bind(stats.output_tokens)
        .bind(stats.cache_read_tokens)
        .bind(stats.cache_write_tokens)
        .bind(stats.user_messages)
        .bind(stats.assistant_messages)
        .bind(stats.total_tool_calls)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Mark session completed with only an ended_at timestamp (no stats).
    pub async fn complete_minimal(
        pool: &PgPool,
        session_id: Uuid,
        ended_at: Option<DateTime<Utc>>,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE sessions SET status = 'completed', ended_at = $2 WHERE id = $1")
            .bind(session_id)
            .bind(ended_at)
            .execute(pool)
            .await?;
        Ok(())
    }
}
