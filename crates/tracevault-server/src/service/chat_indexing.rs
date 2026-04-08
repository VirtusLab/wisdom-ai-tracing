use sqlx::PgPool;
use uuid::Uuid;

use crate::llm::StoryLlm;
use crate::repo::chat_embeddings::ChatEmbeddingsRepo;
use crate::service::chat_chunking::build_chunk_windows;
use crate::service::chat_embeddings::{EmbeddingService, EMBEDDING_MODEL_VERSION};
use crate::service::chat_summarization::{generate_summary, SessionMetadataForSummary};

pub struct ChatIndexingService;

#[derive(sqlx::FromRow)]
struct TranscriptChunkRow {
    chunk_index: i32,
    data: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct SessionMetadataRow {
    repo_name: String,
    user_email: Option<String>,
    model: Option<String>,
    duration_ms: Option<i64>,
    total_tool_calls: Option<i32>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ChatIndexingService {
    /// Index a single session: summarize, embed summary, build chunk windows, embed chunks.
    pub async fn index_session(
        pool: &PgPool,
        embedding_service: &EmbeddingService,
        llm: &dyn StoryLlm,
        session_id: Uuid,
    ) -> Result<(), String> {
        // 1. Mark as processing
        Self::mark_processing(pool, session_id).await?;

        match Self::index_session_inner(pool, embedding_service, llm, session_id).await {
            Ok(()) => {
                Self::mark_completed(pool, session_id).await?;
                Ok(())
            }
            Err(e) => {
                let _ = Self::mark_failed(pool, session_id, &e).await;
                Err(e)
            }
        }
    }

    async fn index_session_inner(
        pool: &PgPool,
        embedding_service: &EmbeddingService,
        llm: &dyn StoryLlm,
        session_id: Uuid,
    ) -> Result<(), String> {
        // 2. Load transcript chunks
        let chunks: Vec<TranscriptChunkRow> = sqlx::query_as(
            "SELECT chunk_index, data FROM transcript_chunks WHERE session_id = $1 ORDER BY chunk_index",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load transcript chunks: {e}"))?;

        if chunks.is_empty() {
            return Err("No transcript chunks found for session".to_string());
        }

        // 3. Load session metadata
        let metadata: SessionMetadataRow = sqlx::query_as(
            "SELECT r.name AS repo_name, u.email AS user_email, s.model,
                    s.duration_ms, s.total_tool_calls, s.started_at
             FROM sessions s
             JOIN repos r ON r.id = s.repo_id
             JOIN users u ON u.id = s.user_id
             WHERE s.id = $1",
        )
        .bind(session_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to load session metadata: {e}"))?;

        let summary_metadata = SessionMetadataForSummary {
            repo_name: metadata.repo_name,
            user_email: metadata.user_email,
            model: metadata.model,
            duration_ms: metadata.duration_ms.unwrap_or(0),
            total_tool_calls: metadata.total_tool_calls.unwrap_or(0),
            started_at: metadata.started_at,
        };

        // Build owned chunk pairs for summarization
        let chunk_pairs: Vec<(i32, serde_json::Value)> = chunks
            .iter()
            .map(|c| (c.chunk_index, c.data.clone()))
            .collect();

        // 4. Generate summary via LLM
        let summary = generate_summary(llm, &chunk_pairs, &summary_metadata).await?;

        // 5. Embed summary
        let summary_embedding = embedding_service.embed_one(&summary).await?;

        // 6. Store session embedding
        ChatEmbeddingsRepo::upsert_session_embedding(
            pool,
            session_id,
            &summary,
            &summary_embedding,
            EMBEDDING_MODEL_VERSION,
        )
        .await
        .map_err(|e| format!("Failed to store session embedding: {e}"))?;

        // 7. Build chunk windows (window_size=4, overlap=1, max_text_len=2000)
        let chunk_refs: Vec<(i32, &serde_json::Value)> =
            chunks.iter().map(|c| (c.chunk_index, &c.data)).collect();
        let windows = build_chunk_windows(&chunk_refs, 4, 1, 2000);

        if !windows.is_empty() {
            // 8. Embed all chunks in batch
            let window_texts: Vec<String> = windows.iter().map(|w| w.text.clone()).collect();
            let window_embeddings = embedding_service.embed(window_texts).await?;

            // 9. Store chunk embeddings
            let chunk_data: Vec<(i32, i32, String, Vec<f32>)> = windows
                .iter()
                .zip(window_embeddings.into_iter())
                .map(|(w, emb)| (w.chunk_start, w.chunk_end, w.content_preview.clone(), emb))
                .collect();

            ChatEmbeddingsRepo::replace_chunk_embeddings(
                pool,
                session_id,
                &chunk_data,
                EMBEDDING_MODEL_VERSION,
            )
            .await
            .map_err(|e| format!("Failed to store chunk embeddings: {e}"))?;
        }

        Ok(())
    }

    async fn mark_processing(pool: &PgPool, session_id: Uuid) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO chat_indexing_status (session_id, status)
             VALUES ($1, 'processing')
             ON CONFLICT (session_id) DO UPDATE SET
                 status = 'processing', error_text = NULL, updated_at = now()",
        )
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark session as processing: {e}"))?;
        Ok(())
    }

    async fn mark_completed(pool: &PgPool, session_id: Uuid) -> Result<(), String> {
        sqlx::query(
            "UPDATE chat_indexing_status SET status = 'completed', updated_at = now()
             WHERE session_id = $1",
        )
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark session as completed: {e}"))?;
        Ok(())
    }

    pub async fn mark_failed(pool: &PgPool, session_id: Uuid, error: &str) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO chat_indexing_status (session_id, status, error_text)
             VALUES ($1, 'failed', $2)
             ON CONFLICT (session_id) DO UPDATE SET
                 status = 'failed', error_text = $2, updated_at = now()",
        )
        .bind(session_id)
        .bind(error)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to mark session as failed: {e}"))?;
        Ok(())
    }

    /// Find unindexed completed sessions and index them in batches.
    pub async fn backfill(
        pool: &PgPool,
        embedding_service: &EmbeddingService,
        llm: &dyn StoryLlm,
        batch_size: i64,
    ) -> Result<u64, String> {
        let session_ids: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT s.id FROM sessions s
             WHERE NOT EXISTS (
                   SELECT 1 FROM chat_indexing_status ci
                   WHERE ci.session_id = s.id AND ci.status = 'completed'
               )
             ORDER BY s.started_at DESC
             LIMIT $1",
        )
        .bind(batch_size)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to find unindexed sessions: {e}"))?;

        let mut indexed = 0u64;
        for (session_id,) in &session_ids {
            match Self::index_session(pool, embedding_service, llm, *session_id).await {
                Ok(()) => {
                    indexed += 1;
                    tracing::info!("Backfill indexed session {session_id}");
                }
                Err(e) => {
                    tracing::warn!("Backfill failed for session {session_id}: {e}");
                }
            }
        }

        Ok(indexed)
    }
}
