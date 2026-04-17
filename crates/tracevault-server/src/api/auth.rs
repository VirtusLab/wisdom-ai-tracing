use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{generate_device_token, generate_session_token, hash_password, verify_password};
use crate::error::AppError;
use crate::extractors::AuthUser;

/// Basic email format validation: local@domain.tld
pub fn is_valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if local.contains(' ') || domain.contains(' ') {
        return false;
    }
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    parts.iter().all(|p| !p.is_empty())
}

// --- Register ---

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub org_name: String,
    pub email: String,
    pub password: String,
    pub name: Option<String>,
    pub signing_key_seed: Option<String>,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub org_name: String,
    pub token: String,
    pub signing_key_seed: Option<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), AppError> {
    if let Err(reason) = crate::password_policy::validate(&req.password) {
        return Err(AppError::BadRequest(reason.into()));
    }

    if !is_valid_email(&req.email) {
        return Err(AppError::BadRequest("Invalid email format".into()));
    }

    let org_slug = req.org_name.trim().to_lowercase();
    if org_slug.is_empty() || org_slug.len() > 100 {
        return Err(AppError::BadRequest(
            "Organization name must be 1-100 characters".into(),
        ));
    }
    if !org_slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || org_slug.starts_with('-')
        || org_slug.ends_with('-')
    {
        return Err(AppError::BadRequest(
            "Organization name must be lowercase alphanumeric with hyphens, no leading/trailing hyphens".into(),
        ));
    }
    let reserved = ["api", "admin", "settings", "auth", "health", "me"];
    if reserved.contains(&org_slug.as_str()) {
        return Err(AppError::BadRequest(
            "This organization name is reserved".into(),
        ));
    }

    // Check if org already exists
    let org_exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM orgs WHERE name = $1")
        .bind(&org_slug)
        .fetch_optional(&state.pool)
        .await?;

    if org_exists.is_some() {
        return Err(AppError::Conflict(
            "Organization already exists. Ask the admin to invite you.".into(),
        ));
    }

    let password_hash = hash_password(&req.password)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

    // Check if email already taken
    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Email already registered".into()));
    }

    // Create org
    let org_id: Uuid = sqlx::query_scalar("INSERT INTO orgs (name) VALUES ($1) RETURNING id")
        .bind(&org_slug)
        .fetch_one(&state.pool)
        .await?;

    // Validate provided signing key if any
    if let Some(ref seed) = req.signing_key_seed {
        if let Err(e) = crate::org_signing::validate_seed(seed) {
            return Err(AppError::BadRequest(format!("Invalid signing key: {e}")));
        }
    }

    // Store signing key (auto-generate or use provided)
    let signing_key_seed = if let Some(ref enc_key) = state.encryption_key {
        match crate::org_signing::generate_and_store(
            &state.pool,
            org_id,
            enc_key,
            req.signing_key_seed.as_deref(),
        )
        .await
        {
            Ok(seed) => Some(seed),
            Err(e) => {
                tracing::warn!("Failed to store signing key for org: {e}");
                None
            }
        }
    } else {
        None
    };

    // Create default compliance settings
    let _ = sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1)")
        .bind(org_id)
        .execute(&state.pool)
        .await;

    // Create user
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.name)
    .fetch_one(&state.pool)
    .await?;

    // Create owner membership
    sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(user_id)
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    // Create session
    let (raw_token, token_hash) = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);

    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            user_id,
            "user.register",
            "user",
            Some(user_id),
            Some(serde_json::json!({"email": &req.email, "org": &org_slug})),
        ),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            user_id,
            org_id,
            org_name: org_slug,
            token: raw_token,
            signing_key_seed,
        }),
    ))
}

// --- Login ---

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub email: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    let row = sqlx::query_as::<_, (Uuid, Option<String>)>(
        "SELECT id, password_hash FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let (user_id, password_hash_opt) = row;

    let password_hash = password_hash_opt.ok_or(AppError::Unauthorized)?;

    if !verify_password(&req.password, &password_hash) {
        return Err(AppError::Unauthorized);
    }

    // Check SSO enforcement
    let sso_enforced_orgs = sqlx::query_as::<_, (uuid::Uuid, String)>(
        "SELECT m.org_id, m.role FROM user_org_memberships m
         JOIN org_sso_configs s ON m.org_id = s.org_id
         WHERE m.user_id = $1 AND s.enforce = true",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;

    if !sso_enforced_orgs.is_empty() {
        // Check if user has any non-SSO org, or is admin/owner in an SSO org
        let non_sso_orgs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_org_memberships m
             WHERE m.user_id = $1
             AND NOT EXISTS (
                 SELECT 1 FROM org_sso_configs s WHERE s.org_id = m.org_id AND s.enforce = true
             )",
        )
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;

        let is_breakglass = sso_enforced_orgs
            .iter()
            .any(|(_, role)| role == "owner" || role == "admin");

        if non_sso_orgs == 0 && !is_breakglass {
            return Err(AppError::Forbidden(
                "Your organization requires SSO login. Use the SSO button on the login page."
                    .into(),
            ));
        }

        // Log break-glass if applicable
        if is_breakglass && non_sso_orgs == 0 {
            for (org_id, _) in &sso_enforced_orgs {
                crate::audit::log(
                    &state.pool,
                    crate::audit::user_action(
                        *org_id,
                        user_id,
                        "auth.login.breakglass",
                        "user",
                        Some(user_id),
                        Some(serde_json::json!({"email": &req.email})),
                    ),
                )
                .await;
            }
        }
    }

    let (raw_token, token_hash) = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);

    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&token_hash)
        .bind(expires_at)
        .execute(&state.pool)
        .await?;

    Ok(Json(LoginResponse {
        token: raw_token,
        user_id,
        email: req.email,
    }))
}

// --- Device Auth ---

#[derive(Serialize)]
pub struct DeviceAuthResponse {
    pub token: String,
    pub verification_url: String,
    pub expires_in: i64,
}

pub async fn device_start(
    State(state): State<AppState>,
) -> Result<Json<DeviceAuthResponse>, AppError> {
    let token = generate_device_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(10);

    sqlx::query(
        "INSERT INTO device_auth_requests (token, status, expires_at) VALUES ($1, 'pending', $2)",
    )
    .bind(&token)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    let verification_url = format!("/auth/device?token={token}");

    Ok(Json(DeviceAuthResponse {
        verification_url,
        token,
        expires_in: 600,
    }))
}

#[derive(Serialize)]
pub struct DeviceStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

pub async fn device_status(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Json<DeviceStatusResponse>, AppError> {
    let row = sqlx::query_as::<_, (String, Option<Uuid>)>(
        "SELECT status, session_id FROM device_auth_requests WHERE token = $1 AND expires_at > NOW()",
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| {
        AppError::NotFound("Device auth request not found or expired".into())
    })?;

    let (status, session_id) = row;

    if status == "approved" {
        if let Some(sid) = session_id {
            let info = sqlx::query_as::<_, (String,)>(
                "SELECT u.email FROM auth_sessions s
                 JOIN users u ON s.user_id = u.id
                 WHERE s.id = $1",
            )
            .bind(sid)
            .fetch_optional(&state.pool)
            .await?;

            if let Some((email,)) = info {
                let raw_token: Option<String> = sqlx::query_scalar(
                    "SELECT session_token FROM device_auth_requests WHERE token = $1",
                )
                .bind(&token)
                .fetch_optional(&state.pool)
                .await?
                .flatten();

                return Ok(Json(DeviceStatusResponse {
                    status: "approved".into(),
                    token: raw_token,
                    email: Some(email),
                }));
            }
        }
    }

    Ok(Json(DeviceStatusResponse {
        status,
        token: None,
        email: None,
    }))
}

pub async fn device_approve(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<StatusCode, AppError> {
    let row = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, status FROM device_auth_requests WHERE token = $1 AND expires_at > NOW()",
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Device auth request not found or expired".into()))?;

    let (request_id, status) = row;
    if status != "pending" {
        return Err(AppError::Conflict(
            "Device auth request already processed".into(),
        ));
    }

    // Create a new session for the CLI
    let (raw_token, token_hash) = generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);

    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(auth.user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .fetch_one(&state.pool)
    .await?;

    // Update device request with approval + session reference + raw token
    sqlx::query(
        "UPDATE device_auth_requests SET status = 'approved', user_id = $1, session_id = $2, session_token = $3 WHERE id = $4",
    )
    .bind(auth.user_id)
    .bind(session_id)
    .bind(&raw_token)
    .bind(request_id)
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::OK)
}

// --- Logout ---

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, AppError> {
    let raw_token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::BadRequest("Missing token".into()))?;

    let token_hash = crate::auth::sha256_hex(raw_token);

    sqlx::query("DELETE FROM auth_sessions WHERE token_hash = $1 AND user_id = $2")
        .bind(&token_hash)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::OK)
}

// --- Me ---

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub email: String,
    pub name: Option<String>,
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<MeResponse>, AppError> {
    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT email, name FROM users WHERE id = $1",
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(MeResponse {
        user_id: auth.user_id,
        email: row.0,
        name: row.1,
    }))
}

// --- List My Orgs ---

#[derive(Serialize)]
pub struct OrgMembership {
    pub org_id: Uuid,
    pub org_name: String,
    pub display_name: Option<String>,
    pub role: String,
}

pub async fn list_my_orgs(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<OrgMembership>>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, String)>(
        "SELECT o.id, o.name, o.display_name, m.role
         FROM user_org_memberships m
         JOIN orgs o ON m.org_id = o.id
         WHERE m.user_id = $1
         ORDER BY o.name",
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, display, role)| OrgMembership {
                org_id: id,
                org_name: name,
                display_name: display,
                role,
            })
            .collect(),
    ))
}

// --- Public Orgs (no auth, for invitation request form) ---

#[derive(Serialize)]
pub struct PublicOrg {
    pub name: String,
    pub display_name: Option<String>,
    pub sso_enabled: bool,
    pub sso_enforce: bool,
}

pub async fn list_public_orgs(
    State(state): State<AppState>,
) -> Result<Json<Vec<PublicOrg>>, AppError> {
    let rows = sqlx::query_as::<_, (String, Option<String>, bool, bool)>(
        "SELECT o.name, o.display_name,
                (s.org_id IS NOT NULL) AS sso_enabled,
                COALESCE(s.enforce, false) AS sso_enforce
         FROM orgs o
         LEFT JOIN org_sso_configs s ON s.org_id = o.id
         ORDER BY o.name",
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|(name, display_name, sso_enabled, sso_enforce)| PublicOrg {
                name,
                display_name,
                sso_enabled,
                sso_enforce,
            })
            .collect(),
    ))
}

// --- Invitation Request (no auth) ---

#[derive(Deserialize)]
pub struct InvitationRequestInput {
    pub org_name: String,
    pub email: String,
    pub name: Option<String>,
}

pub async fn request_invitation(
    State(state): State<AppState>,
    Json(req): Json<InvitationRequestInput>,
) -> Result<StatusCode, AppError> {
    let org_id: Uuid = sqlx::query_scalar("SELECT id FROM orgs WHERE name = $1")
        .bind(&req.org_name)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Organization not found".into()))?;

    // Check for duplicate pending request
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM invitation_requests WHERE org_id = $1 AND email = $2 AND status = 'pending'",
    )
    .bind(org_id)
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(
            "An invitation request is already pending for this email".into(),
        ));
    }

    sqlx::query("INSERT INTO invitation_requests (org_id, email, name) VALUES ($1, $2, $3)")
        .bind(org_id)
        .bind(&req.email)
        .bind(&req.name)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_emails() {
        assert!(is_valid_email("user@example.com"));
        assert!(is_valid_email("user.name@example.co.uk"));
        assert!(is_valid_email("user+tag@domain.org"));
        assert!(is_valid_email("a@b.co"));
    }

    #[test]
    fn invalid_emails() {
        assert!(!is_valid_email(""));
        assert!(!is_valid_email("notanemail"));
        assert!(!is_valid_email("@domain.com"));
        assert!(!is_valid_email("user@"));
        assert!(!is_valid_email("user@.com"));
        assert!(!is_valid_email("user@domain"));
        assert!(!is_valid_email("user @domain.com"));
        assert!(!is_valid_email("user@domain .com"));
    }
}
