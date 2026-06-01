//! Per-user proxy routing rules. Step 1 uses only the default rule
//! (`match_model IS NULL`), which names the credential the proxy forwards to
//! for any request. Step 2 adds exact-match model rules to the same table.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct RoutingRepo;

impl RoutingRepo {
    /// Ensure a default rule exists for `user_id`. If none exists, create one
    /// pointing at `credential_name`. If one already exists it is left alone
    /// (the user owns where the default points; first credential just seeds it).
    pub async fn ensure_default(
        pool: &PgPool,
        user_id: Uuid,
        credential_name: &str,
    ) -> Result<(), AppError> {
        // The conflict-inference predicate matches the partial unique index
        // `idx_routing_default_per_user (user_id) WHERE match_model IS NULL`,
        // so a second call with a different name is a no-op (non-overwriting).
        sqlx::query(
            "INSERT INTO proxy_routing_rules (user_id, match_model, credential_name)
             VALUES ($1, NULL, $2)
             ON CONFLICT (user_id) WHERE match_model IS NULL DO NOTHING",
        )
        .bind(user_id)
        .bind(credential_name)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Repoint the user's default rule at a different credential. Returns false
    /// if the user has no default rule yet.
    pub async fn set_default_credential(
        pool: &PgPool,
        user_id: Uuid,
        credential_name: &str,
    ) -> Result<bool, AppError> {
        let res = sqlx::query(
            "UPDATE proxy_routing_rules
             SET credential_name = $2, updated_at = now()
             WHERE user_id = $1 AND match_model IS NULL",
        )
        .bind(user_id)
        .bind(credential_name)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// The credential name the default rule points at, if any.
    pub async fn default_credential_name(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<String>, AppError> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT credential_name FROM proxy_routing_rules
             WHERE user_id = $1 AND match_model IS NULL",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }
}
