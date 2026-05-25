use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::{extractors::OrgAuth, AppState};

use super::{CommitListQuery, PaginatedResponse};

// ── Response types ───────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct CommitListRow {
    pub id: Uuid,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub author: String,
    pub message: Option<String>,
    pub files_changed: Option<i64>,
    pub ai_sessions_count: Option<i64>,
    pub committed_at: Option<DateTime<Utc>>,
    pub total_count: i64,
}

#[derive(Debug, Serialize)]
pub struct CommitListItem {
    pub id: Uuid,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub author: String,
    pub message: Option<String>,
    pub files_changed: Option<i64>,
    pub ai_sessions_count: Option<i64>,
    pub committed_at: Option<DateTime<Utc>>,
}

impl From<CommitListRow> for CommitListItem {
    fn from(r: CommitListRow) -> Self {
        Self {
            id: r.id,
            commit_sha: r.commit_sha,
            branch: r.branch,
            author: r.author,
            message: r.message,
            files_changed: r.files_changed,
            ai_sessions_count: r.ai_sessions_count,
            committed_at: r.committed_at,
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommitDetail {
    pub id: Uuid,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub author: String,
    pub message: Option<String>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AttributionByFile {
    pub file_path: String,
    pub sessions: Vec<AttributionSession>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AttributionSession {
    pub session_id: Uuid,
    pub session_short_id: String,
    pub confidence: f32,
    pub line_start: Option<i32>,
    pub line_end: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CommitDetailResponse {
    pub commit: CommitDetail,
    pub diff_data: Option<serde_json::Value>,
    pub attributions_by_file: Vec<AttributionByFile>,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// GET /api/v1/orgs/{slug}/traces/commits
pub async fn list_commits(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(params): Query<CommitListQuery>,
) -> Result<Json<PaginatedResponse<CommitListItem>>, AppError> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let raw_rows = sqlx::query_as::<_, CommitListRow>(include_str!("sql/list_commits.sql"))
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&params.branch)
        .bind(params.from)
        .bind(params.to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    let total = raw_rows.first().map(|r| r.total_count).unwrap_or(0);
    let rows: Vec<CommitListItem> = raw_rows.into_iter().map(Into::into).collect();

    Ok(Json(PaginatedResponse {
        items: rows,
        total,
        limit,
        offset,
    }))
}

/// GET /api/v1/orgs/{slug}/traces/commits/{id}
pub async fn get_commit(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, commit_id)): Path<(String, Uuid)>,
) -> Result<Json<CommitDetailResponse>, AppError> {
    let commit = sqlx::query_as::<_, CommitDetail>(include_str!("sql/get_commit.sql"))
        .bind(commit_id)
        .bind(auth.org_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Commit not found".into()))?;

    let diff_data: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT diff_data FROM commits WHERE id = $1")
            .bind(commit_id)
            .fetch_one(&state.pool)
            .await?;

    let attributions = sqlx::query_as::<_, (String, Uuid, String, f32, Option<i32>, Option<i32>)>(
        include_str!("sql/get_commit_attributions.sql"),
    )
    .bind(commit_id)
    .fetch_all(&state.pool)
    .await?;

    let mut by_file: Vec<AttributionByFile> = Vec::new();
    let mut current_file: Option<String> = None;

    for (file_path, session_id, session_short_id, confidence, line_start, line_end) in attributions
    {
        if current_file.as_deref() != Some(&file_path) {
            by_file.push(AttributionByFile {
                file_path: file_path.clone(),
                sessions: Vec::new(),
            });
            current_file = Some(file_path);
        }
        if let Some(last) = by_file.last_mut() {
            last.sessions.push(AttributionSession {
                session_id,
                session_short_id,
                confidence,
                line_start,
                line_end,
            });
        }
    }

    Ok(Json(CommitDetailResponse {
        commit,
        diff_data,
        attributions_by_file: by_file,
    }))
}
