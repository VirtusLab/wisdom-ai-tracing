use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{generate_invite_token, sha256_hex};
use crate::error::{self, AppError};
use crate::extractors::{AuthUser, OrgAuth};
use crate::permissions::Permission;

// --- Create Invite ---

#[derive(Deserialize)]
pub struct CreateInviteRequest {
    pub email: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct CreateInviteResponse {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub invite_url: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create_invite(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<CreateInviteRequest>,
) -> Result<(StatusCode, Json<CreateInviteResponse>), AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    if !crate::api::auth::is_valid_email(&req.email) {
        return Err(AppError::BadRequest("Invalid email format".into()));
    }

    if !state.extensions.permissions.is_valid_role(&req.role) || req.role == "owner" {
        return Err(AppError::BadRequest(
            "Role must be one of: admin, policy_admin, developer, auditor".into(),
        ));
    }

    // Check if email is already a member of this org
    let existing_member: Option<(Uuid,)> = sqlx::query_as(
        "SELECT u.id FROM users u
         JOIN user_org_memberships m ON u.id = m.user_id
         WHERE u.email = $1 AND m.org_id = $2",
    )
    .bind(&req.email)
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?;

    if existing_member.is_some() {
        return Err(AppError::Conflict(
            "User is already a member of this organization".into(),
        ));
    }

    // Revoke any existing pending non-expired invite for same email+org
    sqlx::query(
        "UPDATE org_invites SET status = 'revoked'
         WHERE org_id = $1 AND email = $2 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(auth.org_id)
    .bind(&req.email)
    .execute(&state.pool)
    .await?;

    let (raw_token, token_hash) = generate_invite_token();
    let expires_at =
        chrono::Utc::now() + chrono::Duration::minutes(state.invite_expiry_minutes as i64);

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO org_invites (org_id, email, role, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(auth.org_id)
    .bind(&req.email)
    .bind(&req.role)
    .bind(&token_hash)
    .bind(auth.user_id)
    .bind(expires_at)
    .fetch_one(&state.pool)
    .await?;

    let invite_url = format!("{}/invite/{}", state.cors_origin, raw_token);

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "invite.create",
            "invite",
            Some(id),
            Some(serde_json::json!({"email": &req.email, "role": &req.role})),
        ),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateInviteResponse {
            id,
            email: req.email,
            role: req.role,
            invite_url,
            expires_at,
        }),
    ))
}

// --- List Invites ---

#[derive(Serialize)]
pub struct InviteListItem {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub invited_by: Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_invites(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<Vec<InviteListItem>>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            String,
            String,
            Uuid,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT id, email, role, status, invited_by, expires_at, created_at
         FROM org_invites
         WHERE org_id = $1
         ORDER BY created_at DESC",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    let now = chrono::Utc::now();
    let items = rows
        .into_iter()
        .map(
            |(id, email, role, status, invited_by, expires_at, created_at)| {
                let effective_status = if status == "pending" && expires_at < now {
                    "expired".to_string()
                } else {
                    status
                };
                InviteListItem {
                    id,
                    email,
                    role,
                    status: effective_status,
                    invited_by,
                    expires_at,
                    created_at,
                }
            },
        )
        .collect();

    Ok(Json(items))
}

// --- Revoke Invite ---

pub async fn revoke_invite(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, invite_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    let result = sqlx::query(
        "UPDATE org_invites SET status = 'revoked'
         WHERE id = $1 AND org_id = $2 AND status = 'pending'",
    )
    .bind(invite_id)
    .bind(auth.org_id)
    .execute(&state.pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Invite not found or already processed".into(),
        ));
    }

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "invite.revoke",
            "invite",
            Some(invite_id),
            None,
        ),
    )
    .await;

    Ok(StatusCode::NO_CONTENT)
}

// --- Get Invite Details (public, no auth) ---

#[derive(Serialize)]
pub struct InviteDetailsResponse {
    pub email: String,
    pub org_name: String,
    pub org_slug: String,
    pub existing_user: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_invite_details(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InviteDetailsResponse>, AppError> {
    let token_hash = sha256_hex(&token);

    let row = sqlx::query_as::<_, (String, Uuid, chrono::DateTime<chrono::Utc>)>(
        "SELECT email, org_id, expires_at
         FROM org_invites
         WHERE token_hash = $1 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;

    let (email, org_id, expires_at) = row;

    let org_row = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT name, display_name FROM orgs WHERE id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await?;

    let (org_slug, display_name) = org_row;
    let org_name = display_name.unwrap_or_else(|| org_slug.clone());

    let existing_user = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?
        .is_some();

    Ok(Json(InviteDetailsResponse {
        email,
        org_name,
        org_slug,
        existing_user,
        expires_at,
    }))
}

// --- Accept Invite (new user, no auth) ---

#[derive(Deserialize)]
pub struct AcceptInviteRequest {
    pub password: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct AcceptInviteNewUserResponse {
    pub token: String,
    pub user_id: Uuid,
    pub email: String,
    pub org_name: String,
}

pub async fn accept_invite_new_user(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<AcceptInviteRequest>,
) -> Result<Json<AcceptInviteNewUserResponse>, AppError> {
    let token_hash = sha256_hex(&token);

    let invite_row = sqlx::query_as::<_, (Uuid, String, Uuid, String)>(
        "SELECT id, email, org_id, role
         FROM org_invites
         WHERE token_hash = $1 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;

    let (invite_id, email, org_id, role) = invite_row;

    let existing = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?;

    if existing.is_some() {
        return Err(AppError::ConflictCode {
            code: "account_exists",
            message: "Account already exists. Log in to accept this invite.".into(),
        });
    }

    if let Err(reason) = crate::password_policy::validate(&req.password) {
        return Err(AppError::BadRequest(reason.into()));
    }

    let password_hash = crate::auth::hash_password(&req.password)
        .map_err(|e| AppError::Internal(format!("Failed to hash password: {e}")))?;

    let org_name: String = sqlx::query_scalar("SELECT name FROM orgs WHERE id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await?;

    let mut tx = state.pool.begin().await?;

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&req.name)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(org_id)
        .bind(&role)
        .execute(&mut *tx)
        .await?;

    sqlx::query("UPDATE org_invites SET status = 'accepted' WHERE id = $1")
        .bind(invite_id)
        .execute(&mut *tx)
        .await?;

    let (raw_token, session_hash) = crate::auth::generate_session_token();
    let session_expires = chrono::Utc::now() + chrono::Duration::days(30);

    sqlx::query("INSERT INTO auth_sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&session_hash)
        .bind(session_expires)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            user_id,
            "invite.accept",
            "user",
            Some(user_id),
            Some(serde_json::json!({"email": &email, "method": "new_user"})),
        ),
    )
    .await;

    Ok(Json(AcceptInviteNewUserResponse {
        token: raw_token,
        user_id,
        email,
        org_name,
    }))
}

// --- Accept Invite (existing user, auth required) ---

#[derive(Serialize)]
pub struct AcceptInviteExistingUserResponse {
    pub message: String,
    pub org_name: String,
}

pub async fn accept_invite_existing_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(token): Path<String>,
) -> Result<Json<AcceptInviteExistingUserResponse>, AppError> {
    let token_hash = sha256_hex(&token);

    let invite_row = sqlx::query_as::<_, (Uuid, String, Uuid, String)>(
        "SELECT id, email, org_id, role
         FROM org_invites
         WHERE token_hash = $1 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invite not found or expired".into()))?;

    let (invite_id, email, org_id, role) = invite_row;

    let user_email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_one(&state.pool)
        .await?;

    if user_email != email {
        return Err(AppError::Forbidden("Email does not match invite".into()));
    }

    let org_name: String = sqlx::query_scalar("SELECT name FROM orgs WHERE id = $1")
        .bind(org_id)
        .fetch_one(&state.pool)
        .await?;

    let mut tx = state.pool.begin().await?;

    let membership_result = sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, org_id) DO NOTHING",
    )
    .bind(auth.user_id)
    .bind(org_id)
    .bind(&role)
    .execute(&mut *tx)
    .await?;

    if membership_result.rows_affected() == 0 {
        return Err(AppError::Conflict(
            "User is already a member of this organization".into(),
        ));
    }

    sqlx::query("UPDATE org_invites SET status = 'accepted' WHERE id = $1")
        .bind(invite_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            auth.user_id,
            "invite.accept",
            "user",
            Some(auth.user_id),
            Some(serde_json::json!({"email": &email, "method": "existing_user"})),
        ),
    )
    .await;

    Ok(Json(AcceptInviteExistingUserResponse {
        message: "Membership created".into(),
        org_name,
    }))
}
