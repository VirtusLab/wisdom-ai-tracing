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
pub struct BranchesQuery {
    pub repo_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BranchItem {
    pub branch: String,
    pub tag: Option<String>,
    pub commits_count: i64,
    pub sessions_count: i64,
    pub total_cost: f64,
    pub status: String,
    pub last_activity: Option<DateTime<Utc>>,
}

/// GET /api/v1/orgs/{slug}/traces/branches
pub async fn get_branches(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(q): Query<BranchesQuery>,
) -> Result<Json<Vec<BranchItem>>, AppError> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            i64,
            i64,
            Option<f64>,
            String,
            Option<DateTime<Utc>>,
        ),
    >(include_str!("sql/get_branches.sql"))
    .bind(auth.org_id)
    .bind(q.repo_id)
    .fetch_all(&state.pool)
    .await?;

    let items: Vec<BranchItem> = rows
        .into_iter()
        .map(|r| {
            let status = match r.5.as_str() {
                "merge" => "merged",
                "tag" => "tagged",
                _ => "tracked",
            };
            BranchItem {
                branch: r.0,
                tag: r.1,
                commits_count: r.2,
                sessions_count: r.3,
                total_cost: r.4.unwrap_or(0.0),
                status: status.to_string(),
                last_activity: r.6,
            }
        })
        .collect();

    Ok(Json(items))
}
