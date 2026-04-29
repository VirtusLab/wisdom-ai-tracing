use crate::error::{self, AppError};
use crate::extractors::OrgAuth;
use crate::permissions::Permission;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- SSO Status (public) ---

#[derive(Serialize)]
pub struct SsoStatusResponse {
    pub sso_enabled: bool,
    pub enforce: bool,
}

pub async fn sso_status(
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Result<Json<SsoStatusResponse>, AppError> {
    let org_row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM orgs WHERE LOWER(name) = LOWER($1)")
        .bind(&slug)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Organization '{slug}' not found")))?;

    let org_id = org_row.0;

    let config_row =
        sqlx::query_as::<_, (bool,)>("SELECT enforce FROM org_sso_configs WHERE org_id = $1")
            .bind(org_id)
            .fetch_optional(&state.pool)
            .await?;

    match config_row {
        Some((enforce,)) => Ok(Json(SsoStatusResponse {
            sso_enabled: true,
            enforce,
        })),
        None => Ok(Json(SsoStatusResponse {
            sso_enabled: false,
            enforce: false,
        })),
    }
}

// --- Get SSO Config (OrgAuth, admin+) ---

#[derive(Serialize)]
pub struct SsoConfigResponse {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret_set: bool,
    pub allowed_domains: Vec<String>,
    pub enforce: bool,
    pub auto_provision: bool,
    pub default_role: String,
    pub linked_users: i64,
}

pub async fn get_sso_config(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<SsoConfigResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::OrgSettingsManage)?;

    if !state.extensions.features.sso {
        return Err(AppError::Forbidden(
            "SSO is not available in this edition".into(),
        ));
    }

    let row = sqlx::query_as::<_, (String, String, Vec<String>, bool, bool, String)>(
        "SELECT issuer_url, client_id, allowed_domains, enforce, auto_provision, default_role
         FROM org_sso_configs WHERE org_id = $1",
    )
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("SSO is not configured for this organization".into()))?;

    let (issuer_url, client_id, allowed_domains, enforce, auto_provision, default_role) = row;

    let linked_users: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM user_sso_links WHERE org_id = $1")
            .bind(auth.org_id)
            .fetch_one(&state.pool)
            .await?;

    Ok(Json(SsoConfigResponse {
        issuer_url,
        client_id,
        client_secret_set: true,
        allowed_domains,
        enforce,
        auto_provision,
        default_role,
        linked_users: linked_users.0,
    }))
}

// --- Upsert SSO Config (OrgAuth, owner only) ---

#[derive(Deserialize)]
pub struct UpsertSsoConfigRequest {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub allowed_domains: Vec<String>,
    pub enforce: bool,
    pub auto_provision: bool,
    pub default_role: String,
}

pub async fn upsert_sso_config(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<UpsertSsoConfigRequest>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" {
        return Err(AppError::Forbidden("Requires owner role".into()));
    }

    if !state.extensions.features.sso {
        return Err(AppError::Forbidden(
            "SSO is not available in this edition".into(),
        ));
    }

    // Normalize domains to lowercase
    let allowed_domains: Vec<String> = req
        .allowed_domains
        .iter()
        .map(|d| d.to_lowercase())
        .collect();

    // Determine the encrypted secret to store
    let (secret_encrypted, secret_nonce) = if let Some(ref secret) = req.client_secret {
        // Encrypt the provided secret
        let (ct, nonce) = state
            .extensions
            .encryption
            .encrypt(secret)
            .map_err(|e| AppError::Internal(format!("Encryption error: {e}")))?;
        (ct, nonce)
    } else {
        // Reuse existing secret if it exists, otherwise error
        let existing = sqlx::query_as::<_, (String, String)>(
            "SELECT client_secret_encrypted, client_secret_nonce FROM org_sso_configs WHERE org_id = $1",
        )
        .bind(auth.org_id)
        .fetch_optional(&state.pool)
        .await?;

        match existing {
            Some((ct, nonce)) => (ct, nonce),
            None => {
                return Err(AppError::BadRequest(
                    "client_secret is required when creating a new SSO configuration".into(),
                ))
            }
        }
    };

    sqlx::query(
        "INSERT INTO org_sso_configs
            (org_id, issuer_url, client_id, client_secret_encrypted, client_secret_nonce,
             allowed_domains, enforce, auto_provision, default_role)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (org_id) DO UPDATE SET
            issuer_url = EXCLUDED.issuer_url,
            client_id = EXCLUDED.client_id,
            client_secret_encrypted = EXCLUDED.client_secret_encrypted,
            client_secret_nonce = EXCLUDED.client_secret_nonce,
            allowed_domains = EXCLUDED.allowed_domains,
            enforce = EXCLUDED.enforce,
            auto_provision = EXCLUDED.auto_provision,
            default_role = EXCLUDED.default_role,
            updated_at = NOW()",
    )
    .bind(auth.org_id)
    .bind(&req.issuer_url)
    .bind(&req.client_id)
    .bind(&secret_encrypted)
    .bind(&secret_nonce)
    .bind(&allowed_domains)
    .bind(req.enforce)
    .bind(req.auto_provision)
    .bind(&req.default_role)
    .execute(&state.pool)
    .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "sso.config.update",
            "sso_config",
            Some(auth.org_id),
            Some(serde_json::json!({
                "issuer_url": &req.issuer_url,
                "client_id": &req.client_id,
                "allowed_domains": &allowed_domains,
                "enforce": req.enforce,
                "auto_provision": req.auto_provision,
                "default_role": &req.default_role,
            })),
        ),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

// --- Delete SSO Config (OrgAuth, owner only) ---

#[derive(Serialize)]
pub struct DeleteSsoResponse {
    pub affected_passwordless_users: i64,
}

pub async fn delete_sso_config(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<DeleteSsoResponse>, AppError> {
    if auth.role != "owner" {
        return Err(AppError::Forbidden("Requires owner role".into()));
    }

    // Count passwordless users that have SSO links in this org
    let affected: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT usl.user_id)
         FROM user_sso_links usl
         JOIN users u ON u.id = usl.user_id
         WHERE usl.org_id = $1 AND u.password_hash IS NULL",
    )
    .bind(auth.org_id)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query("DELETE FROM org_sso_configs WHERE org_id = $1")
        .bind(auth.org_id)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "sso.config.delete",
            "sso_config",
            Some(auth.org_id),
            Some(serde_json::json!({
                "affected_passwordless_users": affected.0,
            })),
        ),
    )
    .await;

    Ok(Json(DeleteSsoResponse {
        affected_passwordless_users: affected.0,
    }))
}

// --- SSO Initiate (public, no auth) ---

pub async fn sso_initiate(
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Result<Redirect, AppError> {
    if !state.extensions.sso.is_enabled() {
        return Err(AppError::Forbidden(
            "SSO is not available in this edition".into(),
        ));
    }

    let org_row = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM orgs WHERE LOWER(name) = LOWER($1)")
        .bind(&slug)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Organization '{slug}' not found")))?;

    let org_id = org_row.0;

    let config_row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT issuer_url, client_id, client_secret_encrypted, client_secret_nonce
         FROM org_sso_configs WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("SSO is not configured for this organization".into()))?;

    let (issuer_url, client_id, secret_encrypted, secret_nonce) = config_row;

    let client_secret = state
        .extensions
        .encryption
        .decrypt(&secret_encrypted, &secret_nonce)
        .map_err(|e| AppError::Internal(format!("Failed to decrypt SSO secret: {e}")))?;

    let csrf_state = crate::auth::generate_device_token();

    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);
    sqlx::query("INSERT INTO sso_auth_requests (org_id, state, expires_at) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(&csrf_state)
        .bind(expires_at)
        .execute(&state.pool)
        .await?;

    let redirect_uri = format!("{}/api/v1/auth/sso/{}/callback", state.cors_origin, slug);

    let auth_url = state
        .extensions
        .sso
        .authorization_url(
            &issuer_url,
            &client_id,
            &client_secret,
            &redirect_uri,
            &csrf_state,
        )
        .await
        .map_err(|e| AppError::Internal(format!("Failed to build SSO authorization URL: {e}")))?;

    Ok(Redirect::to(&auth_url))
}

// --- SSO Callback (public, no auth) ---

#[derive(Deserialize)]
pub struct SsoCallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn sso_callback(
    State(state): State<AppState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<SsoCallbackQuery>,
) -> Result<Redirect, AppError> {
    // Validate CSRF state and get org_id
    let auth_req = sqlx::query_as::<_, (Uuid,)>(
        "DELETE FROM sso_auth_requests WHERE state = $1 AND expires_at > NOW() RETURNING org_id",
    )
    .bind(&query.state)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid or expired SSO state".into()))?;

    let org_id = auth_req.0;

    // Verify slug matches the org from the state
    let org_slug: (String,) = sqlx::query_as("SELECT name FROM orgs WHERE id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await?;

    if org_slug.0 != slug {
        return Err(AppError::BadRequest(
            "SSO state does not match organization".into(),
        ));
    }

    let config_row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Vec<String>,
            bool,
            bool,
            String,
        ),
    >(
        "SELECT issuer_url, client_id, client_secret_encrypted, client_secret_nonce,
                allowed_domains, enforce, auto_provision, default_role
         FROM org_sso_configs WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("SSO is not configured for this organization".into()))?;

    let (
        issuer_url,
        client_id,
        secret_encrypted,
        secret_nonce,
        allowed_domains,
        _enforce,
        auto_provision,
        default_role,
    ) = config_row;

    let client_secret = state
        .extensions
        .encryption
        .decrypt(&secret_encrypted, &secret_nonce)
        .map_err(|e| AppError::Internal(format!("Failed to decrypt SSO secret: {e}")))?;

    let redirect_uri = format!("{}/api/v1/auth/sso/{}/callback", state.cors_origin, slug);

    let user_info = state
        .extensions
        .sso
        .exchange_code(
            &issuer_url,
            &client_id,
            &client_secret,
            &redirect_uri,
            &query.code,
        )
        .await
        .map_err(|e| AppError::Internal(format!("SSO code exchange failed: {e}")))?;

    // Validate email domain
    if !allowed_domains.is_empty() {
        let email_lower = user_info.email.to_lowercase();
        let domain = email_lower.split('@').nth(1).unwrap_or("");
        let domain_allowed = allowed_domains.iter().any(|d| d.to_lowercase() == domain);
        if !domain_allowed {
            let error_msg = urlencoding::encode(
                "Email domain is not allowed for SSO login to this organization",
            );
            let login_url = format!("{}/auth/login?error={}", state.cors_origin, error_msg);
            return Ok(Redirect::to(&login_url));
        }
    }

    let user_id = resolve_sso_user(
        &state,
        org_id,
        &user_info,
        &issuer_url,
        auto_provision,
        &default_role,
    )
    .await?;

    // Create auth session
    let (raw_token, token_hash) = crate::auth::generate_session_token();
    let session_expires = chrono::Utc::now() + chrono::Duration::days(30);

    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(session_expires)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            user_id,
            "sso.login",
            "user",
            Some(user_id),
            Some(serde_json::json!({
                "email": &user_info.email,
                "org": &slug,
            })),
        ),
    )
    .await;

    let redirect_url = format!(
        "{}/auth/sso-complete#token={}&org={}",
        state.cors_origin,
        urlencoding::encode(&raw_token),
        urlencoding::encode(&slug),
    );

    Ok(Redirect::to(&redirect_url))
}

// --- Resolve SSO User (private helper) ---

async fn resolve_sso_user(
    state: &AppState,
    org_id: Uuid,
    user_info: &crate::extensions::SsoUserInfo,
    issuer_url: &str,
    auto_provision: bool,
    default_role: &str,
) -> Result<Uuid, AppError> {
    // 1. Check for existing SSO link by (org_id, subject)
    let linked = sqlx::query_as::<_, (Uuid,)>(
        "SELECT user_id FROM user_sso_links WHERE org_id = $1 AND subject = $2",
    )
    .bind(org_id)
    .bind(&user_info.subject)
    .fetch_optional(&state.pool)
    .await?;

    if let Some((user_id,)) = linked {
        // Ensure membership exists (could have been removed)
        sqlx::query(
            "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (user_id, org_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(default_role)
        .execute(&state.pool)
        .await?;
        return Ok(user_id);
    }

    // 2. Check for existing user by email
    let existing_user = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(&user_info.email)
        .fetch_optional(&state.pool)
        .await?;

    if let Some((user_id,)) = existing_user {
        // Ensure membership
        sqlx::query(
            "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (user_id, org_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(default_role)
        .execute(&state.pool)
        .await?;

        // Create SSO link
        sqlx::query(
            "INSERT INTO user_sso_links (user_id, org_id, issuer, subject) VALUES ($1, $2, $3, $4)
             ON CONFLICT (org_id, subject) DO NOTHING",
        )
        .bind(user_id)
        .bind(org_id)
        .bind(issuer_url)
        .bind(&user_info.subject)
        .execute(&state.pool)
        .await?;

        crate::audit::log(
            &state.pool,
            crate::audit::user_action(
                org_id,
                user_id,
                "sso.link",
                "user",
                Some(user_id),
                Some(serde_json::json!({
                    "email": &user_info.email,
                    "issuer": issuer_url,
                })),
            ),
        )
        .await;

        return Ok(user_id);
    }

    // 3. No user found — auto-provision if enabled
    if !auto_provision {
        return Err(AppError::Forbidden(
            "No account found for this email address. Contact your organization administrator."
                .into(),
        ));
    }

    // Create user with NULL password_hash (SSO-only)
    let user_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, NULL, $2) RETURNING id",
    )
    .bind(&user_info.email)
    .bind(&user_info.name)
    .fetch_one(&state.pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            // Handle race condition on users.email UNIQUE constraint
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                // Retry as lookup
                sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
                    .bind(&user_info.email)
                    .fetch_one(&state.pool)
                    .await
                    .map(|(id,)| id)?
            } else {
                return Err(AppError::Sqlx(e));
            }
        }
    };

    sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, org_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(default_role)
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO user_sso_links (user_id, org_id, issuer, subject) VALUES ($1, $2, $3, $4)
         ON CONFLICT (org_id, subject) DO NOTHING",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(issuer_url)
    .bind(&user_info.subject)
    .execute(&state.pool)
    .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            user_id,
            "sso.provision",
            "user",
            Some(user_id),
            Some(serde_json::json!({
                "email": &user_info.email,
                "issuer": issuer_url,
                "default_role": default_role,
            })),
        ),
    )
    .await;

    Ok(user_id)
}
