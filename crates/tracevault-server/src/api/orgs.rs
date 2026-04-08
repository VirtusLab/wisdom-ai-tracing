use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::hash_password;
use crate::error::{self, AppError};
use crate::extractors::{AuthUser, OrgAuth};
use crate::permissions::Permission;

// --- Create Org ---

#[derive(Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub signing_key_seed: Option<String>,
}

#[derive(Serialize)]
pub struct CreateOrgResponse {
    pub id: Uuid,
    pub name: String,
    pub signing_key_seed: Option<String>,
}

const RESERVED_SLUGS: &[&str] = &["api", "admin", "settings", "auth", "health", "me"];

fn is_valid_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 100 {
        return false;
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        return false;
    }
    slug.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub async fn create_org(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<CreateOrgResponse>), AppError> {
    // Normalize and validate slug
    let req_name = req.name.trim().to_lowercase();
    if !is_valid_slug(&req_name) {
        return Err(AppError::BadRequest(
            "Slug must be 1-100 lowercase alphanumeric characters or hyphens, no leading/trailing hyphens".into(),
        ));
    }
    if RESERVED_SLUGS.contains(&req_name.as_str()) {
        return Err(AppError::BadRequest(format!(
            "'{}' is a reserved name",
            req_name
        )));
    }

    // Community edition: only one org allowed
    if !state.extensions.features.multi_org {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM orgs")
            .fetch_one(&state.pool)
            .await?;
        if count.0 > 0 {
            return Err(AppError::Forbidden(
                "Community edition supports one organization".into(),
            ));
        }
    }

    // Create org
    let org_row = sqlx::query_as::<_, (Uuid,)>(
        "INSERT INTO orgs (name, display_name) VALUES ($1, $2) RETURNING id",
    )
    .bind(&req_name)
    .bind(&req.display_name)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("unique") {
            AppError::Conflict("Organization name already taken".into())
        } else {
            AppError::Sqlx(e)
        }
    })?;

    let org_id = org_row.0;

    // Validate provided signing key if any
    if let Some(ref seed) = req.signing_key_seed {
        crate::org_signing::validate_seed(seed)
            .map_err(|e| AppError::BadRequest(format!("Invalid signing key: {e}")))?;
    }

    // Store signing key (auto-generate or use provided)
    let signing_key_seed = if let Some(ref encryption_key) = state.encryption_key {
        Some(
            crate::org_signing::generate_and_store(
                &state.pool,
                org_id,
                encryption_key,
                req.signing_key_seed.as_deref(),
            )
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate signing key: {e}")))?,
        )
    } else {
        None
    };

    // Create membership (owner)
    sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(auth.user_id)
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    // Create default compliance settings
    sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1)")
        .bind(org_id)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            org_id,
            auth.user_id,
            "org.create",
            "org",
            Some(org_id),
            Some(serde_json::json!({"name": &req_name})),
        ),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateOrgResponse {
            id: org_id,
            name: req_name,
            signing_key_seed,
        }),
    ))
}

// --- Get / Update Org ---

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: Uuid,
    pub name: String,
    pub display_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_org(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<OrgResponse>, AppError> {
    if auth.role != "owner" && auth.role != "admin" {
        return Err(AppError::Forbidden("Requires admin role".into()));
    }

    let row = sqlx::query_as::<_, (Uuid, String, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, name, display_name, created_at FROM orgs WHERE id = $1",
    )
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Org not found".into()))?;

    Ok(Json(OrgResponse {
        id: row.0,
        name: row.1,
        display_name: row.2,
        created_at: row.3,
    }))
}

#[derive(Deserialize)]
pub struct UpdateOrgRequest {
    pub display_name: Option<String>,
}

pub async fn update_org(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<UpdateOrgRequest>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" {
        return Err(AppError::Forbidden("Requires owner role".into()));
    }

    if let Some(display_name) = &req.display_name {
        sqlx::query("UPDATE orgs SET display_name = $1 WHERE id = $2")
            .bind(display_name)
            .bind(auth.org_id)
            .execute(&state.pool)
            .await?;
    }

    Ok(StatusCode::OK)
}

// --- Members ---

#[derive(Serialize)]
pub struct MemberResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_members(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<Vec<MemberResponse>>, AppError> {
    if !state
        .extensions
        .permissions
        .has_permission(&auth.role, Permission::UserManage)
        && !state
            .extensions
            .permissions
            .has_permission(&auth.role, Permission::AuditLogView)
    {
        return Err(AppError::Forbidden("Requires admin or auditor role".into()));
    }

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            String,
            Option<String>,
            String,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT u.id, u.email, u.name, m.role, m.created_at
         FROM user_org_memberships m
         JOIN users u ON m.user_id = u.id
         WHERE m.org_id = $1
         ORDER BY m.created_at",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    let members = rows
        .into_iter()
        .map(|r| MemberResponse {
            id: r.0,
            email: r.1,
            name: r.2,
            role: r.3,
            created_at: r.4,
        })
        .collect();

    Ok(Json(members))
}

pub async fn remove_member(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, user_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" {
        return Err(AppError::Forbidden("Requires owner role".into()));
    }
    if auth.user_id == user_id {
        return Err(AppError::BadRequest("Cannot remove yourself".into()));
    }

    sqlx::query("DELETE FROM user_org_memberships WHERE user_id = $1 AND org_id = $2")
        .bind(user_id)
        .bind(auth.org_id)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "member.remove",
            "user",
            Some(user_id),
            None,
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct ChangeRoleRequest {
    pub role: String,
}

pub async fn change_role(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, user_id)): Path<(String, Uuid)>,
    Json(req): Json<ChangeRoleRequest>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" {
        return Err(AppError::Forbidden("Requires owner role".into()));
    }
    if !state.extensions.permissions.is_valid_role(&req.role) || req.role == "owner" {
        return Err(AppError::BadRequest(
            "Role must be one of: admin, policy_admin, developer, auditor".into(),
        ));
    }

    sqlx::query("UPDATE user_org_memberships SET role = $1 WHERE user_id = $2 AND org_id = $3")
        .bind(&req.role)
        .bind(user_id)
        .bind(auth.org_id)
        .execute(&state.pool)
        .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "role.change",
            "user",
            Some(user_id),
            Some(serde_json::json!({"new_role": &req.role})),
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

// --- LLM Settings ---

#[derive(Serialize)]
pub struct LlmSettingsResponse {
    pub provider: Option<String>,
    pub has_api_key: bool,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub async fn get_llm_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<LlmSettingsResponse>, AppError> {
    if auth.role != "owner" && auth.role != "admin" {
        return Err(AppError::Forbidden("Requires admin role".into()));
    }

    let row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT llm_provider, llm_api_key_encrypted, llm_model, llm_base_url
         FROM org_compliance_settings WHERE org_id = $1",
    )
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(r) = row {
        Ok(Json(LlmSettingsResponse {
            provider: r.0,
            has_api_key: r.1.is_some(),
            model: r.2,
            base_url: r.3,
        }))
    } else {
        Ok(Json(LlmSettingsResponse {
            provider: None,
            has_api_key: false,
            model: None,
            base_url: None,
        }))
    }
}

#[derive(Deserialize)]
pub struct UpdateLlmSettingsRequest {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

pub async fn update_llm_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<UpdateLlmSettingsRequest>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" && auth.role != "admin" {
        return Err(AppError::Forbidden("Requires admin role".into()));
    }

    if let Some(ref provider) = req.provider {
        if provider != "anthropic" && provider != "openai" {
            return Err(AppError::BadRequest(
                "Provider must be 'anthropic' or 'openai'".into(),
            ));
        }
    }

    // Encrypt API key if provided
    let (encrypted_key, nonce) = if let Some(ref api_key) = req.api_key {
        let (ct, n) = state
            .extensions
            .encryption
            .encrypt(api_key)
            .map_err(|e| AppError::Internal(format!("Encryption error: {e}")))?;
        (Some(ct), Some(n))
    } else {
        (None, None)
    };

    let org_id = auth.org_id;

    // Upsert: ensure row exists for this org
    sqlx::query(
        "INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT (org_id) DO NOTHING",
    )
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    // Build dynamic UPDATE
    let mut set_clauses = vec!["updated_at = NOW()".to_string()];
    let mut param_idx = 2u32; // $1 is org_id

    if req.provider.is_some() {
        set_clauses.push(format!("llm_provider = ${param_idx}"));
        param_idx += 1;
    }
    if encrypted_key.is_some() {
        set_clauses.push(format!("llm_api_key_encrypted = ${param_idx}"));
        param_idx += 1;
        set_clauses.push(format!("llm_api_key_nonce = ${param_idx}"));
        param_idx += 1;
    }
    if req.model.is_some() {
        set_clauses.push(format!("llm_model = ${param_idx}"));
        param_idx += 1;
    }
    if req.base_url.is_some() {
        set_clauses.push(format!("llm_base_url = ${param_idx}"));
    }

    let sql = format!(
        "UPDATE org_compliance_settings SET {} WHERE org_id = $1",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql).bind(org_id);

    if let Some(ref provider) = req.provider {
        query = query.bind(provider);
    }
    if let Some(ref ek) = encrypted_key {
        query = query.bind(ek);
        query = query.bind(nonce.as_ref().unwrap());
    }
    if let Some(ref model) = req.model {
        query = query.bind(model);
    }
    if let Some(ref base_url) = req.base_url {
        query = query.bind(base_url);
    }

    query.execute(&state.pool).await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "llm_settings.update",
            "org",
            Some(org_id),
            Some(serde_json::json!({
                "provider": req.provider,
                "model": req.model,
                "base_url": req.base_url,
                "api_key_changed": req.api_key.is_some(),
            })),
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

// --- Chat Summarization Settings ---

#[derive(Serialize)]
pub struct ChatSettingsResponse {
    pub provider: Option<String>,
    pub has_api_key: bool,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub auto_summarize: bool,
}

pub async fn get_chat_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<ChatSettingsResponse>, AppError> {
    if auth.role != "owner" && auth.role != "admin" {
        return Err(AppError::Forbidden("Requires admin role".into()));
    }

    let row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            bool,
        ),
    >(
        "SELECT chat_summarization_provider, chat_summarization_api_key_encrypted, chat_summarization_model, chat_summarization_base_url, chat_auto_summarize
         FROM org_compliance_settings WHERE org_id = $1",
    )
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(r) = row {
        Ok(Json(ChatSettingsResponse {
            provider: r.0,
            has_api_key: r.1.is_some(),
            model: r.2,
            base_url: r.3,
            auto_summarize: r.4,
        }))
    } else {
        Ok(Json(ChatSettingsResponse {
            provider: None,
            has_api_key: false,
            model: None,
            base_url: None,
            auto_summarize: false,
        }))
    }
}

#[derive(Deserialize)]
pub struct UpdateChatSettingsRequest {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub auto_summarize: Option<bool>,
}

pub async fn update_chat_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<UpdateChatSettingsRequest>,
) -> Result<StatusCode, AppError> {
    if auth.role != "owner" && auth.role != "admin" {
        return Err(AppError::Forbidden("Requires admin role".into()));
    }

    if let Some(ref provider) = req.provider {
        if provider != "anthropic" && provider != "openai" {
            return Err(AppError::BadRequest(
                "Provider must be 'anthropic' or 'openai'".into(),
            ));
        }
    }

    // Encrypt API key if provided
    let (encrypted_key, nonce) = if let Some(ref api_key) = req.api_key {
        let (ct, n) = state
            .extensions
            .encryption
            .encrypt(api_key)
            .map_err(|e| AppError::Internal(format!("Encryption error: {e}")))?;
        (Some(ct), Some(n))
    } else {
        (None, None)
    };

    let org_id = auth.org_id;

    // Upsert: ensure row exists for this org
    sqlx::query(
        "INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT (org_id) DO NOTHING",
    )
    .bind(org_id)
    .execute(&state.pool)
    .await?;

    // Build dynamic UPDATE
    let mut set_clauses = vec!["updated_at = NOW()".to_string()];
    let mut param_idx = 2u32; // $1 is org_id

    if req.provider.is_some() {
        set_clauses.push(format!("chat_summarization_provider = ${param_idx}"));
        param_idx += 1;
    }
    if encrypted_key.is_some() {
        set_clauses.push(format!(
            "chat_summarization_api_key_encrypted = ${param_idx}"
        ));
        param_idx += 1;
        set_clauses.push(format!("chat_summarization_api_key_nonce = ${param_idx}"));
        param_idx += 1;
    }
    if req.model.is_some() {
        set_clauses.push(format!("chat_summarization_model = ${param_idx}"));
        param_idx += 1;
    }
    if req.base_url.is_some() {
        set_clauses.push(format!("chat_summarization_base_url = ${param_idx}"));
        param_idx += 1;
    }
    if req.auto_summarize.is_some() {
        set_clauses.push(format!("chat_auto_summarize = ${param_idx}"));
    }

    let sql = format!(
        "UPDATE org_compliance_settings SET {} WHERE org_id = $1",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql).bind(org_id);

    if let Some(ref provider) = req.provider {
        query = query.bind(provider);
    }
    if let Some(ref ek) = encrypted_key {
        query = query.bind(ek);
        query = query.bind(nonce.as_ref().unwrap());
    }
    if let Some(ref model) = req.model {
        query = query.bind(model);
    }
    if let Some(ref base_url) = req.base_url {
        query = query.bind(base_url);
    }
    if let Some(auto_summarize) = req.auto_summarize {
        query = query.bind(auto_summarize);
    }

    query.execute(&state.pool).await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "chat_settings.update",
            "org",
            Some(org_id),
            Some(serde_json::json!({
                "provider": req.provider,
                "model": req.model,
                "base_url": req.base_url,
                "api_key_changed": req.api_key.is_some(),
                "auto_summarize": req.auto_summarize,
            })),
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

// --- Chat LLM resolution helpers ---

pub async fn resolve_chat_llm(
    state: &AppState,
    org_id: Uuid,
) -> Option<Box<dyn crate::llm::StoryLlm>> {
    resolve_chat_llm_from_pool(&state.pool, org_id, state.extensions.encryption.as_ref()).await
}

pub async fn resolve_chat_llm_from_pool(
    pool: &sqlx::PgPool,
    org_id: Uuid,
    encryption: &dyn crate::extensions::EncryptionProvider,
) -> Option<Box<dyn crate::llm::StoryLlm>> {
    let row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT chat_summarization_provider, chat_summarization_api_key_encrypted, chat_summarization_api_key_nonce, chat_summarization_model, chat_summarization_base_url
         FROM org_compliance_settings WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .ok()??;

    let provider = row.0?;
    let encrypted_key = row.1?;
    let nonce = row.2?;

    let api_key = encryption
        .decrypt(&encrypted_key, &nonce)
        .map_err(|e| tracing::error!("Failed to decrypt chat LLM API key: {e}"))
        .ok()?;

    crate::llm::create_llm_from_params(&provider, api_key, row.3, row.4)
}

// --- Invitation Requests (admin) ---

#[derive(Serialize)]
pub struct InvitationRequestItem {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_invitation_requests(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<Vec<InvitationRequestItem>>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, email, name, status, created_at FROM invitation_requests WHERE org_id = $1 ORDER BY created_at DESC",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, email, name, status, created_at)| InvitationRequestItem {
                    id,
                    email,
                    name,
                    status,
                    created_at,
                },
            )
            .collect(),
    ))
}

pub async fn approve_invitation_request(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, request_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    let row = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT email, name, status FROM invitation_requests WHERE id = $1 AND org_id = $2",
    )
    .bind(request_id)
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Invitation request not found".into()))?;

    let (email, name, status) = row;
    if status != "pending" {
        return Err(AppError::Conflict("Request already processed".into()));
    }

    // Create user if doesn't exist
    let user_id: Uuid = match sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&state.pool)
        .await?
    {
        Some(id) => id,
        None => {
            // Create user with a random password — they'll need to reset it
            let temp_hash = hash_password(&uuid::Uuid::new_v4().to_string())
                .map_err(|e| AppError::Internal(format!("Hash error: {e}")))?;
            sqlx::query_scalar(
                "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
            )
            .bind(&email)
            .bind(&temp_hash)
            .bind(&name)
            .fetch_one(&state.pool)
            .await?
        }
    };

    // Add membership (ignore if already exists)
    sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, 'developer') ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(auth.org_id)
    .execute(&state.pool)
    .await?;

    // Update request status
    sqlx::query(
        "UPDATE invitation_requests SET status = 'approved', reviewed_by = $1 WHERE id = $2",
    )
    .bind(auth.user_id)
    .bind(request_id)
    .execute(&state.pool)
    .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "invitation_request.approve",
            "user",
            Some(user_id),
            Some(serde_json::json!({"email": &email})),
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

pub async fn reject_invitation_request(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, request_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::UserManage)?;

    let updated = sqlx::query(
        "UPDATE invitation_requests SET status = 'rejected', reviewed_by = $1 WHERE id = $2 AND org_id = $3 AND status = 'pending'",
    )
    .bind(auth.user_id)
    .bind(request_id)
    .bind(auth.org_id)
    .execute(&state.pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Request not found or already processed".into(),
        ));
    }

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "invitation_request.reject",
            "invitation_request",
            Some(request_id),
            None,
        ),
    )
    .await;

    Ok(StatusCode::OK)
}
