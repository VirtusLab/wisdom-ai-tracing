//! User-scoped (`/api/v1/me/...`) endpoints that are not org-bound.
//!
//! Currently only carries the Anthropic-key management endpoints used by the
//! transparent LLM proxy (issue softwaremill/tracevault#207, parent #181).
//! Future per-user settings (preferences, personal access tokens, etc.) belong
//! here as they're added.

use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::repo::user_anthropic_keys::UserAnthropicKeyRepo;
use crate::AppState;

#[derive(Serialize)]
pub struct AnthropicKeyStatus {
    pub configured: bool,
    pub configured_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct PutAnthropicKeyRequest {
    pub key: String,
}

/// Reject the synthetic nil user_id that the AuthUser extractor returns when
/// the request was authenticated with an org-scoped api_key rather than a
/// user session token. The proxy is fundamentally per-user — there is no
/// "current user" for an api_key.
fn require_real_user(auth: &AuthUser) -> Result<Uuid, AppError> {
    if auth.user_id.is_nil() {
        Err(AppError::Forbidden(
            "This endpoint requires a user session token, not an org API key".into(),
        ))
    } else {
        Ok(auth.user_id)
    }
}

/// GET /api/v1/me/anthropic-key
///
/// Returns whether the caller has an Anthropic key configured, plus the
/// timestamp it was last set. The key itself is never returned — there is no
/// API that surfaces decrypted key material.
pub async fn get_anthropic_key_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<AnthropicKeyStatus>, AppError> {
    let user_id = require_real_user(&auth)?;
    let configured_at = UserAnthropicKeyRepo::configured_at(&state.pool, user_id).await?;
    Ok(Json(AnthropicKeyStatus {
        configured: configured_at.is_some(),
        configured_at,
    }))
}

/// PUT /api/v1/me/anthropic-key
///
/// Upserts the caller's Anthropic key, encrypted with the server's master
/// encryption key. Returns 204 on success.
pub async fn put_anthropic_key(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PutAnthropicKeyRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;

    let key = req.key.trim();
    if key.is_empty() {
        return Err(AppError::BadRequest(
            "Anthropic key must not be empty".into(),
        ));
    }
    // Anthropic API keys begin with `sk-ant-` (modern format). We reject
    // anything that doesn't look like one to catch obvious paste mistakes
    // (TV session token, empty string, environment variable name, etc.).
    // We do *not* validate the key against api.anthropic.com here — that
    // would couple this endpoint to upstream availability.
    if !key.starts_with("sk-ant-") {
        return Err(AppError::BadRequest(
            "Anthropic key must start with 'sk-ant-'".into(),
        ));
    }

    let encryption_key = state.encryption_key.as_deref().ok_or_else(|| {
        AppError::Internal(
            "Server is not configured with an encryption key; cannot store Anthropic keys".into(),
        )
    })?;

    UserAnthropicKeyRepo::upsert(&state.pool, encryption_key, user_id, key).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/me/anthropic-key
///
/// Removes the caller's stored Anthropic key. Idempotent — returns 204 even
/// when no key was configured.
pub async fn delete_anthropic_key(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;
    UserAnthropicKeyRepo::delete(&state.pool, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
