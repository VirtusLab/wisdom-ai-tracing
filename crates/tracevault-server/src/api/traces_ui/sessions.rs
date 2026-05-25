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

#[derive(Debug, sqlx::FromRow)]
struct SessionListRow {
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
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
    pub total_count: i64,
}

#[derive(Debug, Serialize)]
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
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub estimated_cost_usd: Option<f64>,
    pub cwd: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl From<SessionListRow> for SessionListItem {
    fn from(r: SessionListRow) -> Self {
        Self {
            id: r.id,
            session_id: r.session_id,
            repo_id: r.repo_id,
            repo_name: r.repo_name,
            user_id: r.user_id,
            user_email: r.user_email,
            status: r.status,
            model: r.model,
            tool: r.tool,
            total_tool_calls: r.total_tool_calls,
            total_tokens: r.total_tokens,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            cache_read_tokens: r.cache_read_tokens,
            cache_write_tokens: r.cache_write_tokens,
            estimated_cost_usd: r.estimated_cost_usd,
            cwd: r.cwd,
            started_at: r.started_at,
            updated_at: r.updated_at,
        }
    }
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
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
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

pub(super) async fn verify_session_access(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    org_id: Uuid,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(include_str!("sql/verify_session_access.sql"))
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

    // Parse comma-separated filter params into Option<Vec<_>>
    let user_ids: Option<Vec<Uuid>> = params.user_ids.as_deref().and_then(|s| {
        let ids: Vec<Uuid> = s
            .split(',')
            .filter(|p| !p.trim().is_empty())
            .filter_map(|p| Uuid::parse_str(p.trim()).ok())
            .collect();
        if ids.is_empty() {
            None
        } else {
            Some(ids)
        }
    });

    let tool_names: Option<Vec<String>> = params.tool_names.as_deref().and_then(|s| {
        let names: Vec<String> = s
            .split(',')
            .filter(|p| !p.trim().is_empty())
            .map(|p| p.trim().to_string())
            .collect();
        if names.is_empty() {
            None
        } else {
            Some(names)
        }
    });

    let raw_rows = sqlx::query_as::<_, SessionListRow>(include_str!("sql/list_sessions.sql"))
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&status_filter)
        .bind(use_stale)
        .bind(params.from)
        .bind(params.to)
        .bind(&user_ids)
        .bind(&tool_names)
        .bind(params.has_file_changes)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let total = raw_rows.first().map(|r| r.total_count).unwrap_or(0);
    let rows: Vec<SessionListItem> = raw_rows.into_iter().map(Into::into).collect();

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
    let session = sqlx::query_as::<_, SessionDetail>(include_str!("sql/get_session.sql"))
        .bind(session_id)
        .bind(auth.org_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".into()))?;

    let events_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE session_id = $1")
        .bind(session_id)
        .fetch_one(&state.pool)
        .await?;

    let file_changes_count: i64 =
        sqlx::query_scalar(include_str!("sql/count_session_file_changes.sql"))
            .bind(session_id)
            .fetch_one(&state.pool)
            .await?;

    let transcript_records_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transcript_chunks WHERE session_id = $1")
            .bind(session_id)
            .fetch_one(&state.pool)
            .await?;

    let linked_commits_count: i64 =
        sqlx::query_scalar(include_str!("sql/count_session_linked_commits.sql"))
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

    let events = sqlx::query_as::<_, EventRow>(include_str!("sql/get_session_events.sql"))
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

    let file_changes =
        sqlx::query_as::<_, FileChangeRow>(include_str!("sql/get_session_file_changes.sql"))
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

    let transcript_chunks = sqlx::query_as::<_, TranscriptChunkRow>(include_str!(
        "sql/get_session_transcript_chunks.sql"
    ))
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

    let linked_commits =
        sqlx::query_as::<_, LinkedCommitRow>(include_str!("sql/get_session_linked_commits.sql"))
            .bind(session_id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(linked_commits))
}

/// GET /api/v1/orgs/{slug}/traces/sessions/filter-options
/// Returns distinct tool names and org members for filter dropdowns.
#[derive(Debug, serde::Serialize)]
pub struct SessionFilterOptions {
    pub tool_names: Vec<String>,
    pub users: Vec<SessionFilterUser>,
}

#[derive(Debug, serde::Serialize)]
pub struct SessionFilterUser {
    pub id: Uuid,
    pub email: String,
}

pub async fn get_session_filter_options(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<SessionFilterOptions>, AppError> {
    let (tool_names_rows, users_rows) = tokio::try_join!(
        sqlx::query_as::<_, (String,)>(
            "SELECT DISTINCT e.tool_name
             FROM events e
             JOIN sessions s ON s.id = e.session_id
             JOIN repos r ON r.id = s.repo_id
             WHERE r.org_id = $1 AND e.tool_name IS NOT NULL
             ORDER BY e.tool_name",
        )
        .bind(auth.org_id)
        .fetch_all(&state.pool),
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT u.id, u.email
             FROM users u
             JOIN user_org_memberships m ON m.user_id = u.id
             WHERE m.org_id = $1
             ORDER BY u.email",
        )
        .bind(auth.org_id)
        .fetch_all(&state.pool),
    )?;

    Ok(Json(SessionFilterOptions {
        tool_names: tool_names_rows.into_iter().map(|(t,)| t).collect(),
        users: users_rows
            .into_iter()
            .map(|(id, email)| SessionFilterUser { id, email })
            .collect(),
    }))
}
