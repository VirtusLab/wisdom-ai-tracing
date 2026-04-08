use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;

use crate::error::{self, AppError};
use crate::extractors::OrgAuth;
use crate::permissions::Permission;
use crate::service::chat_indexing::ChatIndexingService;
use crate::AppState;

#[derive(Serialize)]
pub struct IndexingStatusResponse {
    pub total_sessions: i64,
    pub indexed_sessions: i64,
    pub pending: i64,
    pub failed: i64,
}

#[derive(Serialize)]
pub struct BackfillResponse {
    pub message: String,
}

fn check_chat_admin(state: &AppState, auth: &OrgAuth) -> Result<(), AppError> {
    if !state.extensions.features.chat_search {
        return Err(AppError::Forbidden(
            "Chat search is an enterprise feature".into(),
        ));
    }
    error::require_permission(&state.extensions, &auth.role, Permission::OrgSettingsManage)?;
    Ok(())
}

pub async fn get_indexing_status(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<IndexingStatusResponse>, AppError> {
    check_chat_admin(&state, &auth)?;

    let (total_sessions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions s
         WHERE s.org_id = $1
           AND EXISTS (SELECT 1 FROM transcript_chunks tc WHERE tc.session_id = s.id)",
    )
    .bind(auth.org_id)
    .fetch_one(&state.pool)
    .await?;

    let (indexed_sessions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions s
         JOIN chat_indexing_status ci ON ci.session_id = s.id
         WHERE s.org_id = $1
           AND ci.indexed_chunk_count >= (
               SELECT COUNT(*) FROM transcript_chunks tc WHERE tc.session_id = s.id
           )",
    )
    .bind(auth.org_id)
    .fetch_one(&state.pool)
    .await?;

    let (failed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM chat_indexing_status ci
         JOIN sessions s ON s.id = ci.session_id
         WHERE s.org_id = $1 AND ci.status = 'failed'",
    )
    .bind(auth.org_id)
    .fetch_one(&state.pool)
    .await?;

    let pending = (total_sessions - indexed_sessions - failed).max(0);

    Ok(Json(IndexingStatusResponse {
        total_sessions,
        indexed_sessions,
        pending,
        failed,
    }))
}

pub async fn trigger_backfill(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<(StatusCode, Json<BackfillResponse>), AppError> {
    check_chat_admin(&state, &auth)?;

    let embedding_service = state
        .embedding_service
        .clone()
        .ok_or_else(|| AppError::Internal("Embedding service not available".into()))?;

    let llm = crate::api::orgs::resolve_chat_llm(&state, auth.org_id)
        .await
        .ok_or_else(|| {
            AppError::BadRequest(
                "Chat summarization LLM not configured. Configure it in chat settings.".into(),
            )
        })?;

    let pool = state.pool.clone();

    tokio::spawn(async move {
        match ChatIndexingService::backfill(&pool, &embedding_service, llm.as_ref(), 50).await {
            Ok(count) => {
                tracing::info!("Chat backfill completed: indexed {count} sessions");
            }
            Err(e) => {
                tracing::error!("Chat backfill failed: {e}");
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(BackfillResponse {
            message: "Backfill started".to_string(),
        }),
    ))
}
