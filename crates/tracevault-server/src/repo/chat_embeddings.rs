use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(sqlx::FromRow)]
pub struct SessionSearchResult {
    pub session_id: Uuid,
    pub summary: String,
    pub session_external_id: String,
    pub repo_name: String,
    pub user_email: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub total_tool_calls: Option<i32>,
    pub distance: f64,
}

#[derive(sqlx::FromRow)]
pub struct ChunkSearchResult {
    pub session_id: Uuid,
    pub chunk_start: i32,
    pub chunk_end: i32,
    pub content_preview: String,
    pub distance: f64,
}

pub struct SessionSearchFilter<'a> {
    pub repo_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub time_from: Option<DateTime<Utc>>,
    pub time_to: Option<DateTime<Utc>>,
    pub model_filter: Option<&'a str>,
}

pub struct ChatEmbeddingsRepo;

impl ChatEmbeddingsRepo {
    /// Upsert a session embedding (one summary vector per session).
    pub async fn upsert_session_embedding(
        pool: &PgPool,
        session_id: Uuid,
        summary: &str,
        embedding: &[f32],
        model_version: &str,
    ) -> Result<(), AppError> {
        let embedding_str = format_embedding(embedding);
        sqlx::query(
            "INSERT INTO session_embeddings (session_id, summary, embedding, embedding_model_version)
             VALUES ($1, $2, $3::vector, $4)
             ON CONFLICT (session_id) DO UPDATE SET
                 summary = EXCLUDED.summary,
                 embedding = EXCLUDED.embedding,
                 embedding_model_version = EXCLUDED.embedding_model_version,
                 created_at = now()",
        )
        .bind(session_id)
        .bind(summary)
        .bind(&embedding_str)
        .bind(model_version)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Replace all chunk embeddings for a session (delete + insert).
    pub async fn replace_chunk_embeddings(
        pool: &PgPool,
        session_id: Uuid,
        chunks: &[(i32, i32, String, Vec<f32>)],
        model_version: &str,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;

        sqlx::query("DELETE FROM chunk_embeddings WHERE session_id = $1")
            .bind(session_id)
            .execute(&mut *tx)
            .await?;

        for (chunk_start, chunk_end, content_preview, embedding) in chunks {
            let embedding_str = format_embedding(embedding);
            sqlx::query(
                "INSERT INTO chunk_embeddings (session_id, chunk_start, chunk_end, content_preview, embedding, embedding_model_version)
                 VALUES ($1, $2, $3, $4, $5::vector, $6)",
            )
            .bind(session_id)
            .bind(chunk_start)
            .bind(chunk_end)
            .bind(content_preview)
            .bind(&embedding_str)
            .bind(model_version)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Cosine distance search over session summary embeddings with metadata pre-filtering.
    pub async fn search_session_summaries(
        pool: &PgPool,
        org_id: Uuid,
        query_embedding: &[f32],
        limit: i64,
        filter: &SessionSearchFilter<'_>,
    ) -> Result<Vec<SessionSearchResult>, AppError> {
        let embedding_str = format_embedding(query_embedding);
        let rows = sqlx::query_as::<_, SessionSearchResult>(
            "SELECT
                 s.id AS session_id,
                 se.summary,
                 s.session_id AS session_external_id,
                 r.name AS repo_name,
                 u.email AS user_email,
                 s.model,
                 s.started_at,
                 s.total_tool_calls,
                 (se.embedding <=> $1::vector) AS distance
             FROM session_embeddings se
             JOIN sessions s ON s.id = se.session_id
             JOIN repos r ON r.id = s.repo_id
             JOIN users u ON u.id = s.user_id
             WHERE s.org_id = $2
               AND ($3::uuid IS NULL OR s.repo_id = $3)
               AND ($4::uuid IS NULL OR s.user_id = $4)
               AND ($5::timestamptz IS NULL OR s.started_at >= $5)
               AND ($6::timestamptz IS NULL OR s.started_at <= $6)
               AND ($7::text IS NULL OR s.model = $7)
             ORDER BY distance ASC
             LIMIT $8",
        )
        .bind(&embedding_str)
        .bind(org_id)
        .bind(filter.repo_id)
        .bind(filter.user_id)
        .bind(filter.time_from)
        .bind(filter.time_to)
        .bind(filter.model_filter)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Search chunk embeddings within a specific set of sessions.
    pub async fn search_chunks_in_sessions(
        pool: &PgPool,
        session_ids: &[Uuid],
        query_embedding: &[f32],
        limit: i64,
    ) -> Result<Vec<ChunkSearchResult>, AppError> {
        let embedding_str = format_embedding(query_embedding);
        let rows = sqlx::query_as::<_, ChunkSearchResult>(
            "SELECT
                 ce.session_id,
                 ce.chunk_start,
                 ce.chunk_end,
                 ce.content_preview,
                 (ce.embedding <=> $1::vector) AS distance
             FROM chunk_embeddings ce
             WHERE ce.session_id = ANY($2)
             ORDER BY distance ASC
             LIMIT $3",
        )
        .bind(&embedding_str)
        .bind(session_ids)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}

/// Format a float slice as a pgvector literal: `[0.1,0.2,...]`
fn format_embedding(embedding: &[f32]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(embedding.len() * 8);
    s.push('[');
    for (i, v) in embedding.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(s, "{v}");
    }
    s.push(']');
    s
}
