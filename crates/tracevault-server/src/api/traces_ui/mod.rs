pub mod attribution;
pub mod branches;
pub mod commits;
pub mod sessions;
pub mod timeline;

pub use attribution::get_attribution;
pub use branches::get_branches;
pub use commits::{get_commit, list_commits};
pub use sessions::{
    get_session, get_session_events, get_session_file_changes, get_session_linked_commits,
    get_session_transcript, list_sessions,
};
pub use timeline::get_timeline;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::{extractors::OrgAuth, AppState};

// ── Shared query param types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub repo_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SessionListQuery {
    pub repo_id: Option<Uuid>,
    pub status: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CommitListQuery {
    pub repo_id: Option<Uuid>,
    pub branch: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── Shared response types ────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub active_sessions: i64,
    pub total_sessions: i64,
    pub total_commits: i64,
    pub total_events: i64,
}

// ── Stats handler ────────────────────────────────────────────────────

/// GET /api/v1/orgs/{slug}/traces/stats
pub async fn get_stats(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(params): Query<StatsQuery>,
) -> Result<Json<StatsResponse>, AppError> {
    let repo_filter = params.repo_id;

    let active_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions s
         JOIN repos r ON s.repo_id = r.id
         WHERE r.org_id = $1
           AND s.status = 'active'
           AND s.updated_at >= now() - interval '30 minutes'
           AND ($2::UUID IS NULL OR s.repo_id = $2)",
    )
    .bind(auth.org_id)
    .bind(repo_filter)
    .fetch_one(&state.pool)
    .await?;

    let total_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions s
         JOIN repos r ON s.repo_id = r.id
         WHERE r.org_id = $1
           AND ($2::UUID IS NULL OR s.repo_id = $2)",
    )
    .bind(auth.org_id)
    .bind(repo_filter)
    .fetch_one(&state.pool)
    .await?;

    let total_commits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM commits c
         JOIN repos r ON c.repo_id = r.id
         WHERE r.org_id = $1
           AND ($2::UUID IS NULL OR c.repo_id = $2)",
    )
    .bind(auth.org_id)
    .bind(repo_filter)
    .fetch_one(&state.pool)
    .await?;

    let total_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM events e
         JOIN sessions s ON e.session_id = s.id
         JOIN repos r ON s.repo_id = r.id
         WHERE r.org_id = $1
           AND ($2::UUID IS NULL OR s.repo_id = $2)",
    )
    .bind(auth.org_id)
    .bind(repo_filter)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(StatsResponse {
        active_sessions,
        total_sessions,
        total_commits,
        total_events,
    }))
}
