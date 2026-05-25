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

#[derive(Debug, Serialize, sqlx::FromRow)]
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

    let (rows, total) = tokio::try_join!(
        sqlx::query_as::<_, CommitListItem>(
            "SELECT c.id, c.commit_sha, c.branch, c.author, c.message,
                    COUNT(DISTINCT ca.file_path) AS files_changed,
                    COUNT(DISTINCT ca.session_id) AS ai_sessions_count,
                    c.committed_at
             FROM commits c
             JOIN repos r ON c.repo_id = r.id
             LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
             WHERE r.org_id = $1
               AND ($2::UUID IS NULL OR c.repo_id = $2)
               AND ($3::TEXT IS NULL OR c.branch = $3)
               AND ($4::TIMESTAMPTZ IS NULL OR c.committed_at >= $4)
               AND ($5::TIMESTAMPTZ IS NULL OR c.committed_at <= $5)
             GROUP BY c.id
             ORDER BY c.committed_at DESC NULLS LAST
             LIMIT $6 OFFSET $7",
        )
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&params.branch)
        .bind(params.from)
        .bind(params.to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool),
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT c.id)
             FROM commits c
             JOIN repos r ON c.repo_id = r.id
             WHERE r.org_id = $1
               AND ($2::UUID IS NULL OR c.repo_id = $2)
               AND ($3::TEXT IS NULL OR c.branch = $3)
               AND ($4::TIMESTAMPTZ IS NULL OR c.committed_at >= $4)
               AND ($5::TIMESTAMPTZ IS NULL OR c.committed_at <= $5)",
        )
        .bind(auth.org_id)
        .bind(params.repo_id)
        .bind(&params.branch)
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

/// GET /api/v1/orgs/{slug}/traces/commits/{id}
pub async fn get_commit(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, commit_id)): Path<(String, Uuid)>,
) -> Result<Json<CommitDetailResponse>, AppError> {
    let commit = sqlx::query_as::<_, CommitDetail>(
        "SELECT c.id, c.commit_sha, c.branch, c.author, c.message, c.committed_at
         FROM commits c
         JOIN repos r ON c.repo_id = r.id
         WHERE c.id = $1 AND r.org_id = $2",
    )
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
        "SELECT ca.file_path, ca.session_id, s.session_id AS session_short_id,
                MAX(ca.confidence) AS confidence,
                MIN(ca.line_start) AS line_start,
                MAX(ca.line_end) AS line_end
         FROM commit_attributions ca
         JOIN sessions s ON ca.session_id = s.id
         WHERE ca.commit_id = $1
         GROUP BY ca.file_path, ca.session_id, s.session_id
         ORDER BY ca.file_path, MIN(ca.line_start) NULLS LAST",
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
