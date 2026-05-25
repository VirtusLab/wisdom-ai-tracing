use axum::{
    extract::{Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::{extractors::OrgAuth, AppState};

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    pub repo_id: Option<Uuid>,
    pub tool_name: Option<String>,
    pub session_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TimelineItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub event_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub session_short_id: Option<String>,
    pub event_type: Option<String>,
    pub tool_name: Option<String>,
    pub file_path: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub author: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// GET /api/v1/orgs/{slug}/traces/timeline
pub async fn get_timeline(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(q): Query<TimelineQuery>,
) -> Result<Json<Vec<TimelineItem>>, AppError> {
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);

    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<Uuid>,
            Option<Uuid>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            DateTime<Utc>,
        ),
    >(include_str!("sql/get_timeline.sql"))
    .bind(auth.org_id)
    .bind(q.repo_id)
    .bind(&q.tool_name)
    .bind(q.session_id)
    .bind(q.from)
    .bind(q.to)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|r| TimelineItem {
            item_type: r.0,
            event_id: r.1,
            session_id: r.2,
            session_short_id: r.3,
            event_type: r.4,
            tool_name: r.5,
            file_path: r.6,
            commit_sha: r.7,
            branch: r.8,
            author: r.9,
            timestamp: r.10,
        })
        .collect();

    Ok(Json(items))
}
