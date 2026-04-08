use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct ChatConversationRepo;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct ConversationRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ChatConversationRepo {
    pub async fn create(
        pool: &PgPool,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<ConversationRow, AppError> {
        let row = sqlx::query_as::<_, ConversationRow>(
            "INSERT INTO chat_conversations (org_id, user_id)
             VALUES ($1, $2)
             RETURNING *",
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_for_user(
        pool: &PgPool,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Vec<ConversationRow>, AppError> {
        let rows = sqlx::query_as::<_, ConversationRow>(
            "SELECT * FROM chat_conversations
             WHERE user_id = $1 AND org_id = $2
             ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .bind(org_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ConversationRow>, AppError> {
        let row = sqlx::query_as::<_, ConversationRow>(
            "SELECT * FROM chat_conversations WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn rename(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        title: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE chat_conversations
             SET title = $3, updated_at = now()
             WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .bind(title)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM chat_conversations WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn touch(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
        sqlx::query("UPDATE chat_conversations SET updated_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
