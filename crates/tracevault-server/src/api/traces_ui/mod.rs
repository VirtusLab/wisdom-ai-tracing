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

    let (active_sessions, total_sessions, total_commits, total_events) = tokio::try_join!(
        sqlx::query_scalar(include_str!("sql/stats_active_sessions.sql"))
            .bind(auth.org_id)
            .bind(repo_filter)
            .fetch_one(&state.pool),
        sqlx::query_scalar(include_str!("sql/stats_total_sessions.sql"))
            .bind(auth.org_id)
            .bind(repo_filter)
            .fetch_one(&state.pool),
        sqlx::query_scalar(include_str!("sql/stats_total_commits.sql"))
            .bind(auth.org_id)
            .bind(repo_filter)
            .fetch_one(&state.pool),
        sqlx::query_scalar(include_str!("sql/stats_total_events.sql"))
            .bind(auth.org_id)
            .bind(repo_filter)
            .fetch_one(&state.pool),
    )?;

    Ok(Json(StatsResponse {
        active_sessions,
        total_sessions,
        total_commits,
        total_events,
    }))
}
