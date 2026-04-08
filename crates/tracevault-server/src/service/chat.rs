use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::llm::StoryLlm;
use crate::repo::chat_embeddings::{
    ChatEmbeddingsRepo, ChunkSearchResult, SessionSearchFilter, SessionSearchResult,
};
use crate::repo::chat_messages::{ChatMessageRepo, ChatMessageRow};
use crate::service::chat_embeddings::EmbeddingService;

#[derive(Debug, serde::Deserialize, serde::Serialize, Default)]
pub struct ExtractedFilters {
    pub query: String,
    pub user: Option<String>,
    pub repo: Option<String>,
    pub time_from: Option<String>,
    pub time_to: Option<String>,
    pub model: Option<String>,
}

pub struct ChatResponse {
    pub content: String,
    pub filters: ExtractedFilters,
    pub referenced_sessions: Vec<SessionSearchResult>,
    pub referenced_commits: Vec<ReferencedCommit>,
}

#[derive(serde::Serialize)]
pub struct ReferencedCommit {
    pub sha: String,
    pub message: String,
    pub session_id: Uuid,
}

#[derive(Default)]
pub struct MentionOverrides {
    pub user_id: Option<Uuid>,
    pub repo_id: Option<Uuid>,
    pub model: Option<String>,
}

pub struct ChatService;

impl ChatService {
    pub async fn query(
        pool: &PgPool,
        llm: &dyn StoryLlm,
        embedding_service: &EmbeddingService,
        org_id: Uuid,
        conversation_id: Uuid,
        user_message: &str,
        overrides: &MentionOverrides,
    ) -> Result<ChatResponse, AppError> {
        // 1. Load conversation history (last 10 messages)
        let history = ChatMessageRepo::get_recent(pool, conversation_id, 10).await?;

        // 2. Extract filters via LLM
        let filters = Self::extract_filters(llm, user_message, &history).await;

        // 3. Resolve filter values (mentions override LLM-extracted filters)
        let repo_id = if let Some(id) = overrides.repo_id {
            Some(id)
        } else if let Some(ref repo_name) = filters.repo {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM repos WHERE org_id = $1 AND name = $2")
                .bind(org_id)
                .bind(repo_name)
                .fetch_optional(pool)
                .await?
        } else {
            None
        };

        let user_id = if let Some(id) = overrides.user_id {
            Some(id)
        } else if let Some(ref user_email) = filters.user {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT u.id FROM users u
                 JOIN user_org_memberships m ON m.user_id = u.id
                 WHERE m.org_id = $1 AND u.email = $2",
            )
            .bind(org_id)
            .bind(user_email)
            .fetch_optional(pool)
            .await?
        } else {
            None
        };

        let time_from: Option<DateTime<Utc>> = filters
            .time_from
            .as_deref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());

        let time_to: Option<DateTime<Utc>> = filters
            .time_to
            .as_deref()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());

        let model_filter = overrides.model.as_deref().or(filters.model.as_deref());

        // 4. Embed query
        let query_embedding = embedding_service
            .embed_one(&filters.query)
            .await
            .map_err(|e| AppError::Internal(format!("Embedding failed: {e}")))?;

        // 5. Coarse retrieval: session summaries, top 20
        let search_filter = SessionSearchFilter {
            repo_id,
            user_id,
            time_from,
            time_to,
            model_filter,
        };
        let sessions = ChatEmbeddingsRepo::search_session_summaries(
            pool,
            org_id,
            &query_embedding,
            20,
            &search_filter,
        )
        .await?;

        if sessions.is_empty() {
            return Ok(ChatResponse {
                content: "I couldn't find any relevant sessions matching your query. Try broadening your search or adjusting the filters.".to_string(),
                filters,
                referenced_sessions: vec![],
                referenced_commits: vec![],
            });
        }

        // 6. Fine retrieval: chunks within matched sessions, top 15
        let session_ids: Vec<Uuid> = sessions.iter().map(|s| s.session_id).collect();
        let chunks =
            ChatEmbeddingsRepo::search_chunks_in_sessions(pool, &session_ids, &query_embedding, 15)
                .await?;

        // 7. Fetch chunk texts from DB
        let chunk_texts = Self::fetch_chunk_texts(pool, &chunks).await?;

        // 8. Fetch linked commits
        let referenced_commits = Self::fetch_commits_for_sessions(pool, &session_ids).await?;

        // 9. Generate response via LLM
        let content =
            Self::generate_response(llm, user_message, &history, &sessions, &chunk_texts).await?;

        Ok(ChatResponse {
            content,
            filters,
            referenced_sessions: sessions,
            referenced_commits,
        })
    }

    async fn extract_filters(
        llm: &dyn StoryLlm,
        user_message: &str,
        history: &[ChatMessageRow],
    ) -> ExtractedFilters {
        let history_text = history
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let now = Utc::now();
        let today = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let prompt = format!(
            r#"You are a filter extraction assistant. Given a user's question about coding sessions, extract structured filters.

Current datetime (UTC): {today}

Conversation history:
{history_text}

Current user message: {user_message}

Return ONLY valid JSON with these fields:
- "query": the core search query (always required, rewrite for semantic search)
- "user": email of a specific user if mentioned, or null
- "repo": repository name if mentioned, or null
- "time_from": ISO 8601 datetime string if a start time is mentioned (e.g. "last week" -> appropriate date), or null
- "time_to": ISO 8601 datetime string if an end time is mentioned, or null. If the range includes today or "now", either set to null or use the current datetime above. Never use midnight of today as the end time — that excludes the entire day.
- "model": AI model name if mentioned (e.g. "claude-3.5-sonnet"), or null

If the user asks about recent activity without specifying a clear end date, set time_to to null (meaning "up to now").

Respond with ONLY the JSON object, no markdown, no explanation."#
        );

        let result = llm.generate(&prompt, 500).await;

        match result {
            Ok(text) => {
                // Try to parse the JSON, stripping any markdown fences
                let cleaned = text
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();

                serde_json::from_str::<ExtractedFilters>(cleaned).unwrap_or(ExtractedFilters {
                    query: user_message.to_string(),
                    ..Default::default()
                })
            }
            Err(e) => {
                tracing::warn!("Filter extraction LLM call failed: {e}");
                ExtractedFilters {
                    query: user_message.to_string(),
                    ..Default::default()
                }
            }
        }
    }

    async fn fetch_chunk_texts(
        pool: &PgPool,
        chunks: &[ChunkSearchResult],
    ) -> Result<Vec<String>, AppError> {
        let mut texts = Vec::with_capacity(chunks.len());

        for chunk in chunks {
            let rows: Vec<(i32, serde_json::Value)> = sqlx::query_as(
                "SELECT chunk_index, data FROM transcript_chunks
                 WHERE session_id = $1 AND chunk_index >= $2 AND chunk_index <= $3
                 ORDER BY chunk_index",
            )
            .bind(chunk.session_id)
            .bind(chunk.chunk_start)
            .bind(chunk.chunk_end)
            .fetch_all(pool)
            .await?;

            let text: String = rows
                .iter()
                .map(|(_, data)| crate::service::chat_chunking::extract_text_from_chunk(data))
                .collect::<Vec<_>>()
                .join("\n");

            texts.push(text);
        }

        Ok(texts)
    }

    async fn fetch_commits_for_sessions(
        pool: &PgPool,
        session_ids: &[Uuid],
    ) -> Result<Vec<ReferencedCommit>, AppError> {
        if session_ids.is_empty() {
            return Ok(vec![]);
        }

        let rows: Vec<(String, Option<String>, Uuid)> = sqlx::query_as(
            "SELECT c.commit_sha, c.message, ca.session_id
             FROM commit_attributions ca
             JOIN commits c ON c.id = ca.commit_id
             WHERE ca.session_id = ANY($1)
             GROUP BY c.commit_sha, c.message, ca.session_id
             ORDER BY c.commit_sha",
        )
        .bind(session_ids)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(sha, message, session_id)| ReferencedCommit {
                sha,
                message: message.unwrap_or_default(),
                session_id,
            })
            .collect())
    }

    async fn generate_response(
        llm: &dyn StoryLlm,
        user_message: &str,
        history: &[ChatMessageRow],
        sessions: &[SessionSearchResult],
        chunk_texts: &[String],
    ) -> Result<String, AppError> {
        let history_text = history
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let session_context: String = sessions
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, s)| {
                format!(
                    "Session {} ({}): repo={}, user={}, model={}, started={}\nSummary: {}",
                    i + 1,
                    s.session_external_id,
                    s.repo_name,
                    s.user_email.as_deref().unwrap_or("unknown"),
                    s.model.as_deref().unwrap_or("unknown"),
                    s.started_at
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_else(|| "unknown".to_string()),
                    s.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let chunk_context: String = chunk_texts
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, text)| {
                let truncated = if text.len() > 1500 {
                    &text[..1500]
                } else {
                    text
                };
                format!("--- Chunk {} ---\n{}", i + 1, truncated)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            r#"You are a helpful assistant that answers questions about coding sessions tracked in TraceVault.

Use the provided session summaries and transcript excerpts to answer the user's question accurately.
Reference specific sessions by their IDs when relevant. Be concise but thorough.

Conversation history:
{history_text}

Session summaries:
{session_context}

Relevant transcript excerpts:
{chunk_context}

User question: {user_message}

Provide a helpful, well-structured answer based on the context above. If the context doesn't contain enough information to fully answer, say so."#
        );

        llm.generate(&prompt, 2000)
            .await
            .map_err(|e| AppError::Internal(format!("LLM generation failed: {e}")))
    }
}
