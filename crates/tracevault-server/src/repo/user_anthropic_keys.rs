//! Storage for per-user Anthropic API keys used by the transparent LLM proxy
//! (issue softwaremill/tracevault#207, parent #181).
//!
//! Plaintext keys are encrypted with AES-256-GCM (via `crate::encryption`)
//! before being persisted, and decrypted only inside the proxy hot path —
//! they are never returned through any HTTP response.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::encryption;
use crate::error::AppError;

pub struct UserAnthropicKeyRepo;

/// The plaintext key returned by `get_credential`, plus its concurrency cap.
/// We pull the cap out of the same query so the proxy hot path stays one
/// round-trip per request.
pub struct StoredCredential {
    pub encrypted: String,
    pub nonce: String,
    pub max_concurrent: i32,
}

/// Status returned by the GET endpoint: when the key was set + the
/// current concurrency cap. Never reveals key material.
pub struct StoredStatus {
    pub configured_at: DateTime<Utc>,
    pub max_concurrent: i32,
}

impl UserAnthropicKeyRepo {
    /// Encrypt `plaintext_key` with the configured master `encryption_key`
    /// and upsert it for `user_id`. On conflict the existing row is
    /// overwritten and `updated_at` advances; `created_at` is preserved.
    ///
    /// `max_concurrent` is `Some(N)` to set or change the cap, or `None`
    /// to keep the existing value on update (or fall back to the DB
    /// default `8` on insert).
    pub async fn upsert(
        pool: &PgPool,
        encryption_key: &str,
        user_id: Uuid,
        plaintext_key: &str,
        max_concurrent: Option<i32>,
    ) -> Result<(), AppError> {
        let (encrypted, nonce) = encryption::encrypt(plaintext_key, encryption_key)
            .map_err(|e| AppError::Internal(format!("failed to encrypt anthropic key: {e}")))?;

        // COALESCE-based update lets us either accept an explicit new cap
        // or preserve whatever was already stored. On INSERT, EXCLUDED's
        // max_concurrent is NULL when the caller didn't specify one and
        // the DB default kicks in for the column.
        sqlx::query(
            "INSERT INTO user_anthropic_keys (user_id, key_encrypted, key_nonce, max_concurrent)
             VALUES ($1, $2, $3, COALESCE($4, 8))
             ON CONFLICT (user_id) DO UPDATE SET
               key_encrypted = EXCLUDED.key_encrypted,
               key_nonce = EXCLUDED.key_nonce,
               max_concurrent = COALESCE($4, user_anthropic_keys.max_concurrent),
               updated_at = now()",
        )
        .bind(user_id)
        .bind(&encrypted)
        .bind(&nonce)
        .bind(max_concurrent)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Return the stored credential (ciphertext + nonce + cap) for
    /// `user_id`, or `None` if no key is configured. The proxy calls this
    /// on every request — it is the one read on the hot path.
    pub async fn get_credential(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<StoredCredential>, AppError> {
        let row = sqlx::query_as::<_, (String, String, i32)>(
            "SELECT key_encrypted, key_nonce, max_concurrent
             FROM user_anthropic_keys
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(
            row.map(|(encrypted, nonce, max_concurrent)| StoredCredential {
                encrypted,
                nonce,
                max_concurrent,
            }),
        )
    }

    /// Return `Some(StoredStatus)` if a key is configured for `user_id`,
    /// `None` otherwise. Used by the status-only GET endpoint — never
    /// reveals key material.
    pub async fn status(pool: &PgPool, user_id: Uuid) -> Result<Option<StoredStatus>, AppError> {
        let row = sqlx::query_as::<_, (DateTime<Utc>, i32)>(
            "SELECT updated_at, max_concurrent
             FROM user_anthropic_keys
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(configured_at, max_concurrent)| StoredStatus {
            configured_at,
            max_concurrent,
        }))
    }

    /// Remove the row for `user_id`. Idempotent — returns Ok even if no row
    /// existed.
    pub async fn delete(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_anthropic_keys WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
