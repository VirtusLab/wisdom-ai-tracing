//! Storage for per-user, named upstream credentials used by the LLM proxy.
//! Each credential carries a `protocol` (how to talk — only `anthropic` today)
//! and a `base_url` (where). Plaintext keys are AES-256-GCM encrypted via
//! `crate::encryption` and decrypted only on the proxy hot path.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::encryption;
use crate::error::AppError;

pub struct CredentialRepo;

/// A credential resolved for the proxy hot path: where to send, how to talk,
/// the (still-encrypted) key material, and the concurrency cap.
pub struct ResolvedCredential {
    pub protocol: String,
    pub base_url: String,
    pub encrypted: String,
    pub nonce: String,
    pub max_concurrent: i32,
}

/// Status for the GET endpoint — never reveals key material.
pub struct CredentialStatus {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub configured_at: DateTime<Utc>,
    pub max_concurrent: i32,
}

/// A credential resolved for a specific request model: where/how/key/cap,
/// plus the provider-side model to rewrite to (None = forward as-is).
pub struct RoutedCredential {
    pub protocol: String,
    pub base_url: String,
    pub encrypted: String,
    pub nonce: String,
    pub max_concurrent: i32,
    pub provider_model: Option<String>,
}

/// One credential as listed in the management UI (no key material).
pub struct CredentialListItem {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub max_concurrent: i32,
    pub configured_at: DateTime<Utc>,
}

impl CredentialRepo {
    /// Resolve the credential for a request `model`: an exact-match routing
    /// rule wins; otherwise the default rule. Returns None if the user has no
    /// usable rule/credential.
    pub async fn resolve_for_model(
        pool: &PgPool,
        user_id: Uuid,
        model: Option<&str>,
    ) -> Result<Option<RoutedCredential>, AppError> {
        // A single query: pick the rule that matches the model, else the
        // default (match_model IS NULL), preferring the model match. ORDER BY
        // puts the exact match first; LIMIT 1 takes it.
        let row = sqlx::query_as::<_, (String, String, String, String, i32, Option<String>)>(
            "SELECT c.protocol, c.base_url, c.key_encrypted, c.key_nonce, c.max_concurrent, r.provider_model
             FROM proxy_routing_rules r
             JOIN credentials c ON c.user_id = r.user_id AND c.name = r.credential_name
             WHERE r.user_id = $1 AND (r.match_model = $2 OR r.match_model IS NULL)
             ORDER BY (r.match_model = $2) DESC NULLS LAST
             LIMIT 1",
        )
        .bind(user_id)
        .bind(model) // Option<&str> binds as NULL when None; `match_model = NULL` is never true, so only the default matches
        .fetch_optional(pool)
        .await?;

        Ok(row.map(
            |(protocol, base_url, encrypted, nonce, max_concurrent, provider_model)| {
                RoutedCredential {
                    protocol,
                    base_url,
                    encrypted,
                    nonce,
                    max_concurrent,
                    provider_model,
                }
            },
        ))
    }

    /// All of `user_id`'s credentials for the management UI (no key material).
    pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<CredentialListItem>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, String, i32, DateTime<Utc>)>(
            "SELECT name, protocol, base_url, max_concurrent, updated_at
             FROM credentials WHERE user_id = $1 ORDER BY name",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(name, protocol, base_url, max_concurrent, configured_at)| CredentialListItem {
                    name,
                    protocol,
                    base_url,
                    max_concurrent,
                    configured_at,
                },
            )
            .collect())
    }

    /// Upsert a named credential for `user_id`. On conflict (same user_id+name)
    /// the key/base_url are overwritten and `updated_at` advances; the cap is
    /// kept when `max_concurrent` is None, or set when Some.
    pub async fn upsert(
        pool: &PgPool,
        encryption_key: &str,
        user_id: Uuid,
        name: &str,
        base_url: &str,
        plaintext_key: &str,
        max_concurrent: Option<i32>,
    ) -> Result<(), AppError> {
        let (encrypted, nonce) = encryption::encrypt(plaintext_key, encryption_key)
            .map_err(|e| AppError::Internal(format!("failed to encrypt credential: {e}")))?;

        sqlx::query(
            "INSERT INTO credentials
               (user_id, name, protocol, base_url, key_encrypted, key_nonce, max_concurrent)
             VALUES ($1, $2, 'anthropic', $3, $4, $5, COALESCE($6, 8))
             ON CONFLICT (user_id, name) DO UPDATE SET
               base_url = EXCLUDED.base_url,
               key_encrypted = EXCLUDED.key_encrypted,
               key_nonce = EXCLUDED.key_nonce,
               max_concurrent = COALESCE($6, credentials.max_concurrent),
               updated_at = now()",
        )
        .bind(user_id)
        .bind(name)
        .bind(base_url)
        .bind(&encrypted)
        .bind(&nonce)
        .bind(max_concurrent)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Resolve the credential a user's request should use: follow the user's
    /// default routing rule to a credential by name. The proxy hot-path read.
    /// `None` when the user has no default rule or it points at a missing
    /// credential.
    pub async fn resolve_default(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<ResolvedCredential>, AppError> {
        let row = sqlx::query_as::<_, (String, String, String, String, i32)>(
            "SELECT c.protocol, c.base_url, c.key_encrypted, c.key_nonce, c.max_concurrent
             FROM proxy_routing_rules r
             JOIN credentials c
               ON c.user_id = r.user_id AND c.name = r.credential_name
             WHERE r.user_id = $1 AND r.match_model IS NULL",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(
            |(protocol, base_url, encrypted, nonce, max_concurrent)| ResolvedCredential {
                protocol,
                base_url,
                encrypted,
                nonce,
                max_concurrent,
            },
        ))
    }

    /// Status of the user's default credential (for the GET endpoint), or None.
    pub async fn default_status(
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<Option<CredentialStatus>, AppError> {
        let row = sqlx::query_as::<_, (String, String, String, DateTime<Utc>, i32)>(
            "SELECT c.name, c.protocol, c.base_url, c.updated_at, c.max_concurrent
             FROM proxy_routing_rules r
             JOIN credentials c
               ON c.user_id = r.user_id AND c.name = r.credential_name
             WHERE r.user_id = $1 AND r.match_model IS NULL",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(
            |(name, protocol, base_url, configured_at, max_concurrent)| CredentialStatus {
                name,
                protocol,
                base_url,
                configured_at,
                max_concurrent,
            },
        ))
    }

    /// Update only the cap on a named credential. Returns false if no such row.
    pub async fn update_max_concurrent(
        pool: &PgPool,
        user_id: Uuid,
        name: &str,
        max_concurrent: i32,
    ) -> Result<bool, AppError> {
        let res = sqlx::query(
            "UPDATE credentials SET max_concurrent = $3, updated_at = now()
             WHERE user_id = $1 AND name = $2",
        )
        .bind(user_id)
        .bind(name)
        .bind(max_concurrent)
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Delete a named credential. Idempotent.
    pub async fn delete(pool: &PgPool, user_id: Uuid, name: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM credentials WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(name)
            .execute(pool)
            .await?;
        Ok(())
    }
}
