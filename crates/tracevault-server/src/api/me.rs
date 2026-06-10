//! User-scoped (`/api/v1/me/...`) endpoints that are not org-bound.
//!
//! Currently only carries the Anthropic-key management endpoints used by the
//! transparent LLM proxy (issue VirtusLab/visdom-ai-tracing#207, parent #181).
//! Future per-user settings (preferences, personal access tokens, etc.) belong
//! here as they're added.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::AuthUser;
use crate::repo::credentials::CredentialRepo;
use crate::repo::routing::RoutingRepo;
use crate::AppState;

#[derive(Serialize)]
pub struct AnthropicKeyStatus {
    pub configured: bool,
    pub configured_at: Option<DateTime<Utc>>,
    /// Per-credential proxy concurrency cap. `None` when no key is
    /// configured; otherwise the value stored on the row.
    pub max_concurrent: Option<i32>,
    /// Name of the default credential, when configured.
    pub name: Option<String>,
    /// Where the default credential forwards to, when configured.
    pub base_url: Option<String>,
}

#[derive(Deserialize)]
pub struct PutAnthropicKeyRequest {
    /// Optional new Anthropic key. When omitted the existing ciphertext is
    /// preserved — this is the "cap only" update path, used from the UI
    /// when the user wants to change `max_concurrent` without rotating
    /// the key. At least one of `key`, `max_concurrent`, or `base_url`
    /// must be present.
    #[serde(default)]
    pub key: Option<String>,
    /// Optional per-credential proxy concurrency cap. Omit to keep the
    /// existing value on update, or fall back to the DB default (8) on
    /// first insert.
    #[serde(default)]
    pub max_concurrent: Option<i32>,
    /// Credential name. Defaults to "default" (the single-credential case).
    #[serde(default)]
    pub name: Option<String>,
    /// Upstream base URL. Defaults to the server's configured Anthropic base.
    #[serde(default)]
    pub base_url: Option<String>,
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
    let status = CredentialRepo::default_status(&state.pool, user_id).await?;
    Ok(Json(match status {
        Some(s) => AnthropicKeyStatus {
            configured: true,
            configured_at: Some(s.configured_at),
            max_concurrent: Some(s.max_concurrent),
            name: Some(s.name),
            base_url: Some(s.base_url),
        },
        None => AnthropicKeyStatus {
            configured: false,
            configured_at: None,
            max_concurrent: None,
            name: None,
            base_url: None,
        },
    }))
}

/// PUT /api/v1/me/anthropic-key
///
/// Upserts the caller's Anthropic key and/or its concurrency cap. The
/// request body has four optional fields — `key`, `max_concurrent`, `name`
/// (defaults to "default"), and `base_url` (defaults to the server's
/// configured Anthropic base, and is only settable alongside a `key`) — but
/// at least one of `key` or `max_concurrent` must be present. (`base_url`
/// alone is not enough: it rides along with a `key` and is rejected on the
/// no-key path, so the guard does not count it.)
/// Use cases:
///
///   * `{ key: "sk-ant-...", max_concurrent: 16 }` — first-time setup or
///     full rotation.
///   * `{ key: "sk-ant-...", base_url: "https://..." }` — store/rotate the
///     key and point the credential at a new upstream.
///   * `{ key: "sk-ant-..." }` — rotate the key; cap preserved (default 8
///     applied if no row yet).
///   * `{ max_concurrent: 16 }` — change only the cap; key must already
///     exist (400 otherwise).
///
/// In all cases the in-memory per-credential semaphore for this user is
/// dropped from the DashMap so the *next* proxy request rebuilds it
/// against the new cap value. In-flight requests keep their permits on
/// the old (dropped) semaphore for the lifetime of their response,
/// effectively letting the cap change apply at the natural next quiet
/// point.
pub async fn put_anthropic_key(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PutAnthropicKeyRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;
    let name = req.name.as_deref().unwrap_or("default").to_string();

    // `base_url` is intentionally absent from this guard: on this legacy
    // single-key endpoint it only applies alongside a `key` (see the no-key
    // branch below, which rejects it). Counting it here would let a
    // base_url-only request pass the guard and then 400 anyway — a
    // contradictory accepted-but-impossible path.
    if req.key.is_none() && req.max_concurrent.is_none() {
        return Err(AppError::BadRequest(
            "Request must include `key` or `max_concurrent`".into(),
        ));
    }

    // Validate max_concurrent if the caller specified one. Bounds mirror
    // the DB CHECK constraint so we fail fast with a clear 400 instead of
    // surfacing a generic constraint-violation 500 from the upsert.
    if let Some(n) = req.max_concurrent {
        if !(1..=256).contains(&n) {
            return Err(AppError::BadRequest(
                "max_concurrent must be between 1 and 256".into(),
            ));
        }
    }

    match req.key.as_deref() {
        // Rotate or first-time store the key (optional cap + base_url ride along).
        Some(raw_key) => {
            store_credential(
                &state,
                user_id,
                &name,
                raw_key,
                req.base_url.as_deref(),
                req.max_concurrent,
            )
            .await?;
            // First credential seeds the default rule pointing at it.
            RoutingRepo::ensure_default(&state.pool, user_id, &name).await?;
        }
        None => {
            // Settings-only update on an existing named credential.
            // base_url can only be applied alongside a key (the row is re-encrypted on
            // store); reject it on the no-key path rather than silently dropping it.
            if req.base_url.is_some() {
                return Err(AppError::BadRequest(
                    "Changing base_url requires also providing the key".into(),
                ));
            }
            if let Some(n) = req.max_concurrent {
                let updated =
                    CredentialRepo::update_max_concurrent(&state.pool, user_id, &name, n).await?;
                if !updated {
                    return Err(AppError::BadRequest(
                        "Cannot update settings: no credential configured yet".into(),
                    ));
                }
            }
        }
    }

    // Flush the in-memory per-credential semaphore so the next request
    // rebuilds it against the new cap (or the freshly-persisted row).
    // In-flight requests still hold permits on the old, now-orphaned
    // Arc<Semaphore> — when they finish they release naturally and the
    // arc drops.
    evict_credential_semaphore(&state, user_id, &name).await;

    Ok(StatusCode::NO_CONTENT)
}

/// Flush the in-memory concurrency semaphore for the named credential so a
/// changed `max_concurrent` (or a deletion) takes effect on the next request
/// rather than only after a restart. The DashMap is keyed by credential id, so
/// resolve the id from `(user_id, name)` first; a no-op if the credential
/// doesn't exist or has never had a semaphore created.
async fn evict_credential_semaphore(state: &AppState, user_id: Uuid, name: &str) {
    if let Ok(Some(id)) =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM credentials WHERE user_id = $1 AND name = $2")
            .bind(user_id)
            .bind(name)
            .fetch_optional(&state.pool)
            .await
    {
        state.proxy_per_credential_semaphores.remove(&id);
    }
}

/// Validate and store (rotate or first-time insert) a named credential,
/// optionally setting the per-credential concurrency cap alongside it.
/// `base_url` defaults to the server's configured Anthropic base and is
/// SSRF-validated when provided.
async fn store_credential(
    state: &AppState,
    user_id: Uuid,
    name: &str,
    raw_key: &str,
    base_url: Option<&str>,
    max_concurrent: Option<i32>,
) -> Result<(), AppError> {
    let key = raw_key.trim();
    if key.is_empty() {
        return Err(AppError::BadRequest(
            "Anthropic key must not be empty".into(),
        ));
    }
    // Real Anthropic keys are ~110 chars; cap at 256 to leave generous
    // headroom for future formats while preventing the endpoint from
    // accepting a ~2 MB junk string and persisting it encrypted.
    if key.len() > 256 {
        return Err(AppError::BadRequest(
            "Anthropic key is unreasonably long (max 256 chars)".into(),
        ));
    }
    if !key.starts_with("sk-ant-") {
        return Err(AppError::BadRequest(
            "Anthropic key must start with 'sk-ant-'".into(),
        ));
    }
    let base = match base_url {
        Some(u) => crate::validate_base_url(u)?,
        None => state.default_credential_base_url.clone(),
    };
    let encryption_key = state.encryption_key.as_deref().ok_or_else(|| {
        AppError::Internal(
            "Server is not configured with an encryption key; cannot store credentials".into(),
        )
    })?;
    CredentialRepo::upsert(
        &state.pool,
        encryption_key,
        user_id,
        name,
        &base,
        key,
        max_concurrent,
    )
    .await
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
    // Step 1: the UI manages a single "default" credential.
    CredentialRepo::delete(&state.pool, user_id, "default").await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Named credentials CRUD (`/api/v1/me/credentials`) -------------------

/// One credential as surfaced by `GET /api/v1/me/credentials` — never carries
/// key material.
#[derive(Serialize)]
pub struct CredentialItem {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub max_concurrent: i32,
    pub configured_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct PutCredentialRequest {
    /// Optional new key. Omit on a settings-only (cap) update of an existing
    /// credential. At least one of `key`, `max_concurrent`, or `base_url`
    /// must be present.
    #[serde(default)]
    pub key: Option<String>,
    /// Optional upstream base URL. Only settable alongside a `key` (the row is
    /// re-encrypted on store), mirroring `put_anthropic_key`.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Optional per-credential proxy concurrency cap (1..=256).
    #[serde(default)]
    pub max_concurrent: Option<i32>,
}

/// GET /api/v1/me/credentials
///
/// Lists all of the caller's named credentials (no key material).
pub async fn list_credentials(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<CredentialItem>>, AppError> {
    let user_id = require_real_user(&auth)?;
    let items = CredentialRepo::list(&state.pool, user_id).await?;
    Ok(Json(
        items
            .into_iter()
            .map(|c| CredentialItem {
                name: c.name,
                protocol: c.protocol,
                base_url: c.base_url,
                max_concurrent: c.max_concurrent,
                configured_at: c.configured_at,
            })
            .collect(),
    ))
}

/// PUT /api/v1/me/credentials/{name}
///
/// Creates or updates the named credential. The validation rules mirror
/// `put_anthropic_key` exactly (key prefix/length, `base_url` SSRF-validation,
/// cap bounds, and "base_url requires a key"). On the user's first-ever
/// credential the default routing rule is seeded to point at it.
pub async fn put_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(name): Path<String>,
    Json(req): Json<PutCredentialRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;

    if req.key.is_none() && req.max_concurrent.is_none() && req.base_url.is_none() {
        return Err(AppError::BadRequest(
            "Request must include `key`, `max_concurrent`, or `base_url`".into(),
        ));
    }

    if let Some(n) = req.max_concurrent {
        if !(1..=256).contains(&n) {
            return Err(AppError::BadRequest(
                "max_concurrent must be between 1 and 256".into(),
            ));
        }
    }

    match req.key.as_deref() {
        Some(raw_key) => {
            store_credential(
                &state,
                user_id,
                &name,
                raw_key,
                req.base_url.as_deref(),
                req.max_concurrent,
            )
            .await?;
            // First credential seeds the default rule pointing at it. A no-op
            // once the user already has a default rule.
            RoutingRepo::ensure_default(&state.pool, user_id, &name).await?;
        }
        None => {
            // No new key: update metadata (base_url and/or cap) on an EXISTING
            // credential. base_url is a plain column, so it can change without
            // re-supplying the key. Validate base_url; require the credential to
            // already exist (creating one needs a key).
            let validated_base = match req.base_url.as_deref() {
                Some(u) => Some(crate::validate_base_url(u)?),
                None => None,
            };
            let updated = CredentialRepo::update_metadata(
                &state.pool,
                user_id,
                &name,
                validated_base.as_deref(),
                req.max_concurrent,
            )
            .await?;
            if !updated {
                return Err(AppError::BadRequest(format!(
                    "No credential named '{name}' — provide `key` to create one"
                )));
            }
        }
    }

    // Flush the in-memory per-credential semaphore so the next request
    // rebuilds it against the new cap (or freshly-persisted row).
    evict_credential_semaphore(&state, user_id, &name).await;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/me/credentials/{name}
///
/// Removes the named credential. Idempotent — returns 204 even when no such
/// credential exists. The FK cascade drops any routing rules pointing at it.
pub async fn delete_credential(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(name): Path<String>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;
    // Evict before deleting so the credential id is still resolvable.
    evict_credential_semaphore(&state, user_id, &name).await;
    CredentialRepo::delete(&state.pool, user_id, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

// --- Proxy routing rules (`/api/v1/me/proxy-routing`) --------------------

/// One routing rule as surfaced by `GET /api/v1/me/proxy-routing`. A `null`
/// `match_model` is the default rule (all otherwise-unmatched models).
#[derive(Serialize)]
pub struct RoutingRuleItem {
    pub id: Uuid,
    pub match_model: Option<String>,
    pub credential_name: String,
    pub provider_model: Option<String>,
}

#[derive(Deserialize)]
pub struct PutRoutingRuleRequest {
    /// The request model to match exactly. `null`/omitted repoints the default
    /// rule (all otherwise-unmatched models).
    #[serde(default)]
    pub match_model: Option<String>,
    /// The credential this rule routes to. Must be one of the caller's
    /// existing credentials.
    pub credential_name: String,
    /// Optional provider-side model to rewrite the request to. `null`/omitted
    /// forwards the requested model verbatim.
    #[serde(default)]
    pub provider_model: Option<String>,
}

/// GET /api/v1/me/proxy-routing
///
/// Lists the caller's routing rules (default rule first, then model rules).
pub async fn list_routing_rules(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<RoutingRuleItem>>, AppError> {
    let user_id = require_real_user(&auth)?;
    let rules = RoutingRepo::list(&state.pool, user_id).await?;
    Ok(Json(
        rules
            .into_iter()
            .map(|r| RoutingRuleItem {
                id: r.id,
                match_model: r.match_model,
                credential_name: r.credential_name,
                provider_model: r.provider_model,
            })
            .collect(),
    ))
}

/// PUT /api/v1/me/proxy-routing
///
/// Creates or updates a routing rule keyed by `match_model` (`null` = the
/// default rule). The referenced `credential_name` must already exist among
/// the caller's credentials — we check up front and return a 400 with a clear
/// message, so the FK constraint never surfaces as a generic 500.
pub async fn put_routing_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<PutRoutingRuleRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;

    // Guard the FK before the upsert: a rule pointing at a missing credential
    // would otherwise fail with a 500 from the constraint violation.
    let credentials = CredentialRepo::list(&state.pool, user_id).await?;
    if !credentials.iter().any(|c| c.name == req.credential_name) {
        return Err(AppError::BadRequest(format!(
            "no credential named '{}'",
            req.credential_name
        )));
    }

    RoutingRepo::upsert_rule(
        &state.pool,
        user_id,
        req.match_model.as_deref(),
        &req.credential_name,
        req.provider_model.as_deref(),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/me/proxy-routing/{id}
///
/// Removes the model routing rule with the given id. Returns 404 when no such
/// rule exists or it is the (non-deletable) default rule.
pub async fn delete_routing_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = require_real_user(&auth)?;
    let deleted = RoutingRepo::delete_rule(&state.pool, user_id, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("routing rule not found".into()))
    }
}
