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

impl UserAnthropicKeyRepo {
    /// Encrypt `plaintext_key` with the configured master `encryption_key`
    /// and upsert it for `user_id`. On conflict the existing row is
    /// overwritten and `updated_at` advances; `created_at` is preserved.
    pub async fn upsert(
        pool: &PgPool,
        encryption_key: &str,
        user_id: Uuid,
        plaintext_key: &str,
    ) -> Result<(), AppError> {
        let (encrypted, nonce) = encryption::encrypt(plaintext_key, encryption_key)
            .map_err(|e| AppError::Internal(format!("failed to encrypt anthropic key: {e}")))?;

        sqlx::query(
            "INSERT INTO user_anthropic_keys (user_id, key_encrypted, key_nonce)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id) DO UPDATE SET
               key_encrypted = EXCLUDED.key_encrypted,
               key_nonce = EXCLUDED.key_nonce,
               updated_at = now()",
        )
        .bind(user_id)
        .bind(&encrypted)
        .bind(&nonce)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Return the encrypted ciphertext and nonce for `user_id`, or `None`
    /// if no key is configured. Callers decrypt via `crate::encryption::decrypt`.
    pub async fn get_ciphertext(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<(String, String)>, AppError> {
        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT key_encrypted, key_nonce FROM user_anthropic_keys WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Return `Some(updated_at)` if a key is configured for `user_id`, `None`
    /// otherwise. Used by the status-only GET endpoint — never reveals key
    /// material.
    pub async fn configured_at(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>, AppError> {
        let row = sqlx::query_scalar::<_, DateTime<Utc>>(
            "SELECT updated_at FROM user_anthropic_keys WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
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
