use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::error::{self, AppError};
use crate::extractors::OrgAuth;
use crate::permissions::Permission;
use crate::repo::chat_conversations::{ChatConversationRepo, ConversationRow};
use crate::repo::chat_messages::{ChatMessageRepo, ChatMessageRow};
use crate::service::chat::{ChatService, ExtractedFilters};
use crate::AppState;

// --- Request/Response types ---

#[derive(serde::Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

#[derive(serde::Deserialize)]
pub struct RenameRequest {
    pub title: String,
}

#[derive(serde::Serialize)]
pub struct SendMessageResponse {
    pub content: String,
    pub filters: ExtractedFilters,
    pub referenced_sessions: Vec<SessionRef>,
    pub referenced_commits: Vec<CommitRef>,
}

#[derive(serde::Serialize)]
pub struct SessionRef {
    pub session_id: Uuid,
    pub session_external_id: String,
    pub repo_name: String,
    pub user_email: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub summary_snippet: String,
}

#[derive(serde::Serialize)]
pub struct CommitRef {
    pub sha: String,
    pub message: String,
    pub session_id: Uuid,
}

#[derive(serde::Serialize)]
pub struct ConversationWithMessages {
    pub conversation: ConversationRow,
    pub messages: Vec<ChatMessageRow>,
}

#[derive(serde::Serialize)]
pub struct MentionUser {
    pub id: Uuid,
    pub display: String,
    pub email: String,
}

#[derive(serde::Serialize)]
pub struct MentionRepo {
    pub id: Uuid,
    pub display: String,
}

#[derive(serde::Serialize)]
pub struct MentionModel {
    pub display: String,
}

#[derive(serde::Serialize)]
pub struct MentionsResponse {
    pub users: Vec<MentionUser>,
    pub repos: Vec<MentionRepo>,
    pub models: Vec<MentionModel>,
}

// --- Guards ---

fn check_chat_enabled(state: &AppState, auth: &OrgAuth) -> Result<(), AppError> {
    if !state.extensions.features.chat_search {
        return Err(AppError::Forbidden(
            "Chat search is an enterprise feature".into(),
        ));
    }
    error::require_permission(&state.extensions, &auth.role, Permission::ChatUse)?;
    Ok(())
}

// --- Handlers ---

pub async fn create_conversation(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<(StatusCode, Json<ConversationRow>), AppError> {
    check_chat_enabled(&state, &auth)?;

    let conversation = ChatConversationRepo::create(&state.pool, auth.org_id, auth.user_id).await?;

    Ok((StatusCode::CREATED, Json(conversation)))
}

pub async fn list_conversations(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<Vec<ConversationRow>>, AppError> {
    check_chat_enabled(&state, &auth)?;

    let conversations =
        ChatConversationRepo::list_for_user(&state.pool, auth.user_id, auth.org_id).await?;

    Ok(Json(conversations))
}

pub async fn get_conversation(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
) -> Result<Json<ConversationWithMessages>, AppError> {
    check_chat_enabled(&state, &auth)?;

    let conversation = ChatConversationRepo::get(&state.pool, id, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".into()))?;

    let messages = ChatMessageRepo::get_all(&state.pool, id).await?;

    Ok(Json(ConversationWithMessages {
        conversation,
        messages,
    }))
}

pub async fn rename_conversation(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
    Json(req): Json<RenameRequest>,
) -> Result<StatusCode, AppError> {
    check_chat_enabled(&state, &auth)?;

    let updated = ChatConversationRepo::rename(&state.pool, id, auth.user_id, &req.title).await?;

    if !updated {
        return Err(AppError::NotFound("Conversation not found".into()));
    }

    Ok(StatusCode::OK)
}

pub async fn delete_conversation(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    check_chat_enabled(&state, &auth)?;

    let deleted = ChatConversationRepo::delete(&state.pool, id, auth.user_id).await?;

    if !deleted {
        return Err(AppError::NotFound("Conversation not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn send_message(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, AppError> {
    check_chat_enabled(&state, &auth)?;

    // Verify conversation exists and belongs to user
    let conversation = ChatConversationRepo::get(&state.pool, id, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".into()))?;

    // Save user message
    ChatMessageRepo::insert(&state.pool, id, "user", &req.message, None, None, None).await?;

    // Auto-title: first 60 chars of user message if no title
    if conversation.title.is_none() {
        let auto_title: String = req
            .message
            .chars()
            .take(60)
            .collect::<String>()
            .trim()
            .to_string();
        if !auto_title.is_empty() {
            let _ = ChatConversationRepo::rename(&state.pool, id, auth.user_id, &auto_title).await;
        }
    }

    // Resolve Chat LLM
    let llm = crate::api::orgs::resolve_chat_llm(&state, auth.org_id)
        .await
        .ok_or_else(|| {
            AppError::BadRequest(
                "Chat LLM not configured for this organization. Configure it in Chat LLM settings."
                    .into(),
            )
        })?;

    // Get embedding service
    let embedding_service = state
        .embedding_service
        .as_ref()
        .ok_or_else(|| AppError::Internal("Embedding service not available".into()))?;

    // Call query pipeline
    let response = ChatService::query(
        &state.pool,
        llm.as_ref(),
        embedding_service,
        auth.org_id,
        id,
        &req.message,
    )
    .await?;

    // Build references for storage
    let session_ids: Vec<Uuid> = response
        .referenced_sessions
        .iter()
        .map(|s| s.session_id)
        .collect();
    let commit_shas: Vec<String> = response
        .referenced_commits
        .iter()
        .map(|c| c.sha.clone())
        .collect();
    let filters_json = serde_json::to_value(&response.filters).ok();

    // Save assistant message with references
    ChatMessageRepo::insert(
        &state.pool,
        id,
        "assistant",
        &response.content,
        Some(&session_ids),
        Some(&commit_shas),
        filters_json,
    )
    .await?;

    // Touch conversation updated_at
    ChatConversationRepo::touch(&state.pool, id).await?;

    // Build response
    let session_refs: Vec<SessionRef> = response
        .referenced_sessions
        .iter()
        .map(|s| SessionRef {
            session_id: s.session_id,
            session_external_id: s.session_external_id.clone(),
            repo_name: s.repo_name.clone(),
            user_email: s.user_email.clone(),
            started_at: s.started_at,
            summary_snippet: if s.summary.len() > 200 {
                s.summary[..200].to_string()
            } else {
                s.summary.clone()
            },
        })
        .collect();

    let commit_refs: Vec<CommitRef> = response
        .referenced_commits
        .into_iter()
        .map(|c| CommitRef {
            sha: c.sha,
            message: c.message,
            session_id: c.session_id,
        })
        .collect();

    Ok(Json(SendMessageResponse {
        content: response.content,
        filters: response.filters,
        referenced_sessions: session_refs,
        referenced_commits: commit_refs,
    }))
}

pub async fn list_mentions(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<MentionsResponse>, AppError> {
    check_chat_enabled(&state, &auth)?;

    let users: Vec<(Uuid, Option<String>, String)> = sqlx::query_as(
        "SELECT u.id, u.name, u.email FROM users u
         JOIN user_org_memberships m ON m.user_id = u.id
         WHERE m.org_id = $1
         ORDER BY u.email",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    let repos: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, name FROM repos WHERE org_id = $1 ORDER BY name",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    let models: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT model FROM sessions WHERE org_id = $1 AND model IS NOT NULL ORDER BY model",
    )
    .bind(auth.org_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(MentionsResponse {
        users: users
            .into_iter()
            .map(|(id, name, email)| {
                let display = name.unwrap_or_else(|| {
                    email.split('@').next().unwrap_or(&email).to_string()
                });
                MentionUser { id, display, email }
            })
            .collect(),
        repos: repos
            .into_iter()
            .map(|(id, name)| MentionRepo { id, display: name })
            .collect(),
        models: models
            .into_iter()
            .map(|(model,)| MentionModel { display: model })
            .collect(),
    }))
}
