use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct ChatMessageRepo;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct ChatMessageRow {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub referenced_sessions: Option<Vec<Uuid>>,
    pub referenced_commits: Option<Vec<String>>,
    pub filters_applied: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ChatMessageRepo {
    pub async fn insert(
        pool: &PgPool,
        conversation_id: Uuid,
        role: &str,
        content: &str,
        referenced_sessions: Option<&[Uuid]>,
        referenced_commits: Option<&[String]>,
        filters_applied: Option<serde_json::Value>,
    ) -> Result<ChatMessageRow, AppError> {
        let row = sqlx::query_as::<_, ChatMessageRow>(
            "INSERT INTO chat_messages
                (conversation_id, role, content, referenced_sessions, referenced_commits, filters_applied)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *",
        )
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(referenced_sessions)
        .bind(referenced_commits)
        .bind(filters_applied)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn get_recent(
        pool: &PgPool,
        conversation_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatMessageRow>, AppError> {
        let rows = sqlx::query_as::<_, ChatMessageRow>(
            "SELECT * FROM (
                SELECT * FROM chat_messages
                WHERE conversation_id = $1
                ORDER BY created_at DESC
                LIMIT $2
             ) sub
             ORDER BY created_at ASC",
        )
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_all(
        pool: &PgPool,
        conversation_id: Uuid,
    ) -> Result<Vec<ChatMessageRow>, AppError> {
        let rows = sqlx::query_as::<_, ChatMessageRow>(
            "SELECT * FROM chat_messages
             WHERE conversation_id = $1
             ORDER BY created_at ASC",
        )
        .bind(conversation_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
