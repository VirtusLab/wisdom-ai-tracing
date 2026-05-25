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
    >(
        "SELECT * FROM (
            SELECT 'event'::text AS item_type,
                   e.id AS event_id,
                   e.session_id,
                   LEFT(s.session_id, 8) AS session_short_id,
                   e.event_type,
                   e.tool_name,
                   e.tool_input->>'file_path' AS file_path,
                   NULL::text AS commit_sha,
                   NULL::text AS branch,
                   NULL::text AS author,
                   e.timestamp
            FROM events e
            JOIN sessions s ON e.session_id = s.id
            JOIN repos r ON s.repo_id = r.id
            WHERE r.org_id = $1
              AND ($2::uuid IS NULL OR s.repo_id = $2)
              AND ($3::text IS NULL OR e.tool_name = $3)
              AND ($4::uuid IS NULL OR e.session_id = $4)
              AND ($5::timestamptz IS NULL OR e.timestamp >= $5)
              AND ($6::timestamptz IS NULL OR e.timestamp <= $6)

            UNION ALL

            SELECT 'commit'::text AS item_type,
                   NULL::uuid AS event_id,
                   NULL::uuid AS session_id,
                   NULL::text AS session_short_id,
                   NULL::text AS event_type,
                   NULL::text AS tool_name,
                   NULL::text AS file_path,
                   c.commit_sha,
                   c.branch,
                   c.author,
                   c.committed_at AS timestamp
            FROM commits c
            JOIN repos r ON c.repo_id = r.id
            WHERE r.org_id = $1
              AND ($2::uuid IS NULL OR c.repo_id = $2)
              AND ($5::timestamptz IS NULL OR c.committed_at >= $5)
              AND ($6::timestamptz IS NULL OR c.committed_at <= $6)
              AND $3::text IS NULL
              AND $4::uuid IS NULL
        ) combined
        ORDER BY timestamp DESC
        LIMIT $7 OFFSET $8",
    )
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
