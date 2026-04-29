use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct MemberRow {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

pub struct OrgRepo;

impl OrgRepo {
    pub async fn name_exists(pool: &PgPool, name: &str) -> Result<bool, AppError> {
        let row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM orgs WHERE LOWER(name) = LOWER($1)")
            .bind(name)
            .fetch_optional(pool)
            .await?;

        Ok(row.is_some())
    }

    pub async fn find_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM orgs WHERE LOWER(name) = LOWER($1)")
            .bind(slug)
            .fetch_optional(pool)
            .await?;

        Ok(row.map(|(id,)| id))
    }

    pub async fn create(
        pool: &PgPool,
        name: &str,
        display_name: Option<&str>,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO orgs (name, display_name) VALUES ($1, $2) RETURNING id",
        )
        .bind(name)
        .bind(display_name)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn create_compliance_settings(pool: &PgPool, org_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(org_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn count(pool: &PgPool) -> Result<i64, AppError> {
        let row = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM orgs")
            .fetch_one(pool)
            .await?;

        Ok(row.0)
    }

    pub async fn list_members(pool: &PgPool, org_id: Uuid) -> Result<Vec<MemberRow>, AppError> {
        let rows = sqlx::query_as::<_, MemberRow>(
            "SELECT u.id, u.email, u.name, m.role, m.created_at
             FROM user_org_memberships m
             JOIN users u ON m.user_id = u.id
             WHERE m.org_id = $1
             ORDER BY m.created_at",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
