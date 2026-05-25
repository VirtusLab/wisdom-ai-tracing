use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::pricing;
use crate::{extractors::OrgAuth, AppState};

use super::{PaginatedResponse, SessionListQuery};
use crate::api::session_detail::{parse_transcript, TranscriptRecord};

// ── Response types ───────────────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SessionListItem {
    pub id: Uuid,
    pub session_id: String,
    pub repo_id: Uuid,
    pub repo_name: String,
    pub user_id: Uuid,
    pub user_email: String,
    pub status: String,
    pub model: Option<String>,
    pub tool: Option<String>,
    pub total_tool_calls: Option<i32>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SessionDetail {
    pub id: Uuid,
    pub session_id: String,
    pub repo_name: String,
    pub user_email: String,
    pub status: String,
    pub model: Option<String>,
    pub tool: Option<String>,
    pub total_tool_calls: Option<i32>,
    pub total_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EventRow {
    pub id: Uuid,
    pub event_index: i32,
    pub event_type: String,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FileChangeRow {
    pub id: Uuid,
    pub file_path: String,
    pub change_type: String,
    pub diff_text: Option<String>,
    pub content_hash: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TranscriptChunkRow {
    pub chunk_index: i32,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LinkedCommitRow {
    pub commit_id: Uuid,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct SessionMetadataResponse {
    pub session: SessionDetail,
    pub counts: SessionCounts,
}

#[derive(Debug, Serialize)]
pub struct SessionCounts {
    pub events: i64,
    pub file_changes: i64,
    pub transcript_records: i64,
    pub linked_commits: i64,
}

#[derive(Debug, Serialize)]
pub struct TranscriptResponse {
    pub transcript_chunks: Vec<TranscriptChunkRow>,
    pub transcript_records: Vec<TranscriptRecord>,
}

// ── Helpers ──────────────────────────────────────────────────────────

pub async fn verify_session_access(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    org_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sessions s
            JOIN repos r ON s.repo_id = r.id
            WHERE s.id = $1 AND r.org_id = $2
        )",
    )
    .bind(session_id)
    .bind(org_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        return Err(AppError::NotFound("Session not found".into()));
    }
    Ok(())
}

// ── Handlers ─────────────────────────────────────────────────────────

/// GET /api/v1/orgs/{slug}/traces/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(params): Query<SessionListQuery>,
) -> Result<Json<PaginatedResponse<SessionListItem>>, AppError> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let (status_filter, use_stale) = match params.status.as_deref() {
        Some("stale") => (Some("active".to_string()), true),
        other => (other.map(String::from), false),
    };

    let (rows, total) = tokio::try_join!(
        sqlx::query_as::<_, SessionListItem>(
            "SELECT s.id, s.session_id, s.repo_id, r.name AS repo_name,
                    s.user_id, u.email AS user_email, s.status, s.model, s.tool,
                    s.total_tool_calls, s.total_tokens, s.estimated_cost_usd,
                    s.cwd, s.started_at, s.updated_at
             FROM sessions s
             JOIN repos r ON s.repo_id = r.id
             JOIN users u ON s.user_id = u.id
             WHERE r.org_id = $1
               AND ($2::UUID IS NULL OR s.repo_id = $2)
               AND ($3::TEXT IS NULL OR s.status = $3)
               AND ($4::BOOL = FALSE OR s.updated_at < now() - interval '30 minutes')
               AND ($5::TIMESTAMPTZ IS NULL OR s.started_at >= $5)
               AND ($6::TIMESTAMPTZ IS NULL OR s.started_at <= $6)
             ORDER BY s.updated_at DESC
             LIMIT $7 OFFSET $8",
        )
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&status_filter)
        .bind(use_stale)
        .bind(params.from)
        .bind(params.to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM sessions s
             JOIN repos r ON s.repo_id = r.id
             WHERE r.org_id = $1
               AND ($2::UUID IS NULL OR s.repo_id = $2)
               AND ($3::TEXT IS NULL OR s.status = $3)
               AND ($4::BOOL = FALSE OR s.updated_at < now() - interval '30 minutes')
               AND ($5::TIMESTAMPTZ IS NULL OR s.started_at >= $5)
               AND ($6::TIMESTAMPTZ IS NULL OR s.started_at <= $6)",
        )
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&status_filter)
        .bind(use_stale)
        .bind(params.from)
        .bind(params.to)
        .fetch_one(&state.pool),
    )?;

    Ok(Json(PaginatedResponse {
        items: rows,
        total,
        limit,
        offset,
    }))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/{id}
pub async fn get_session(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_id)): Path<(String, Uuid)>,
) -> Result<Json<SessionMetadataResponse>, AppError> {
    let session = sqlx::query_as::<_, SessionDetail>(
        "SELECT s.id, s.session_id, r.name AS repo_name, u.email AS user_email,
                s.status, s.model, s.tool, s.total_tool_calls, s.total_tokens,
                s.estimated_cost_usd, s.cwd, s.started_at, s.ended_at, s.updated_at
         FROM sessions s
         JOIN repos r ON s.repo_id = r.id
         JOIN users u ON s.user_id = u.id
         WHERE s.id = $1 AND r.org_id = $2",
    )
    .bind(session_id)
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let events_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(&state.pool)
        .await?;

    let file_changes_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
            SELECT DISTINCT ON (file_path, change_type, COALESCE(diff_text, ''))
                   id
            FROM file_changes
            WHERE session_id = $1
            ORDER BY file_path, change_type, COALESCE(diff_text, ''), timestamp DESC
        ) sub",
    )
    .bind(session_id)
    .fetch_one(&state.pool)
    .await?;

    let transcript_records_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transcript_chunks WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await?;

    let linked_commits_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT ca.commit_id)
         FROM commit_attributions ca
         JOIN commits c ON ca.commit_id = c.id
         WHERE ca.session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(SessionMetadataResponse {
        session,
        counts: SessionCounts {
            events: events_count,
            file_changes: file_changes_count,
            transcript_records: transcript_records_count,
            linked_commits: linked_commits_count,
        },
    }))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/{id}/events
pub async fn get_session_events(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<EventRow>>, AppError> {
    verify_session_access(&state.pool, session_id, auth.org_id).await?;

    let events = sqlx::query_as::<_, EventRow>(
        "SELECT id, event_index, event_type, tool_name, tool_input, tool_response, timestamp
         FROM events
         WHERE session_id = $1
         ORDER BY event_index ASC",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(events))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/{id}/file-changes
pub async fn get_session_file_changes(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<FileChangeRow>>, AppError> {
    verify_session_access(&state.pool, session_id, auth.org_id).await?;

    let file_changes = sqlx::query_as::<_, FileChangeRow>(
        "SELECT DISTINCT ON (file_path, change_type, COALESCE(diff_text, ''))
                id, file_path, change_type, diff_text, content_hash, timestamp
         FROM file_changes
         WHERE session_id = $1
         ORDER BY file_path, change_type, COALESCE(diff_text, ''), timestamp DESC",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(file_changes))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/{id}/transcript
pub async fn get_session_transcript(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_id)): Path<(String, Uuid)>,
) -> Result<Json<TranscriptResponse>, AppError> {
    verify_session_access(&state.pool, session_id, auth.org_id).await?;

    let session_model: Option<String> =
        sqlx::query_scalar("SELECT model FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await?;

    let session_started_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT started_at FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await?;

    let transcript_chunks = sqlx::query_as::<_, TranscriptChunkRow>(
        "SELECT chunk_index, data
         FROM transcript_chunks
         WHERE session_id = $1
         ORDER BY chunk_index ASC",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    let pricing = pricing::fetch_pricing_for_model(
        &state.pool,
        session_model.as_deref().unwrap_or("sonnet"),
        session_started_at,
    )
    .await;

    let transcript_array: Vec<serde_json::Value> =
        transcript_chunks.iter().map(|c| c.data.clone()).collect();
    let transcript_val = serde_json::Value::Array(transcript_array);
    let (_, transcript_records, _, _, _) = parse_transcript(&transcript_val, &pricing);

    Ok(Json(TranscriptResponse {
        transcript_chunks,
        transcript_records,
    }))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/{id}/linked-commits
pub async fn get_session_linked_commits(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, session_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<LinkedCommitRow>>, AppError> {
    verify_session_access(&state.pool, session_id, auth.org_id).await?;

    let linked_commits = sqlx::query_as::<_, LinkedCommitRow>(
        "SELECT ca.commit_id, c.commit_sha, c.branch, MAX(ca.confidence) AS confidence
         FROM commit_attributions ca
         JOIN commits c ON ca.commit_id = c.id
         WHERE ca.session_id = $1
         GROUP BY ca.commit_id, c.commit_sha, c.branch, c.committed_at
         ORDER BY c.committed_at DESC NULLS LAST",
    )
    .bind(session_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(linked_commits))
}
