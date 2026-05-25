use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::pricing;
use crate::{extractors::OrgAuth, AppState};

use super::session_detail::{parse_transcript, TranscriptRecord};

// ── Query param types ───────────────────────────────────────────────

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

// ── Response types ──────────────────────────────────────────────────

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

// ── Helpers ─────────────────────────────────────────────────────────

async fn verify_session_access(
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

// ── Handlers ────────────────────────────────────────────────────────

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

/// GET /api/v1/orgs/{slug}/traces/sessions
pub async fn list_sessions(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(params): Query<SessionListQuery>,
) -> Result<Json<PaginatedResponse<SessionListItem>>, AppError> {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    // Map status filter: "stale" is a virtual status
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

    // Fetch attributions grouped by file
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

    // Group attributions by file_path
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

// ── Timeline ────────────────────────────────────────────────────────

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

    // Build the UNION ALL query
    let rows = sqlx::query_as::<
        _,
        (
            String,         // item_type
            Option<Uuid>,   // event_id
            Option<Uuid>,   // session_id
            Option<String>, // session_short_id
            Option<String>, // event_type
            Option<String>, // tool_name
            Option<String>, // file_path
            Option<String>, // commit_sha
            Option<String>, // branch
            Option<String>, // author
            DateTime<Utc>,  // timestamp
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

    let items: Vec<TimelineItem> = rows
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

// ── Attribution (Code Blame) ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AttributionResponse {
    pub file_path: String,
    pub commit_sha: String,
    pub lines: Vec<AttributionLine>,
}

#[derive(Debug, Serialize)]
pub struct AttributionLine {
    pub line_number: usize,
    pub content: String,
    pub git_author: Option<String>,
    pub session_id: Option<String>,
    pub session_short_id: Option<String>,
    pub confidence: Option<f32>,
}

/// GET /api/v1/orgs/{slug}/traces/attribution/{commit_id}/{*file_path}
pub async fn get_attribution(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, commit_id, file_path)): Path<(String, Uuid, String)>,
) -> Result<Json<AttributionResponse>, AppError> {
    // Get commit + repo info
    let row = sqlx::query_as::<_, (String, Uuid)>(
        "SELECT c.commit_sha, c.repo_id
         FROM commits c
         JOIN repos r ON c.repo_id = r.id
         WHERE c.id = $1 AND r.org_id = $2",
    )
    .bind(commit_id)
    .bind(auth.org_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Commit not found".into()))?;

    let (commit_sha, repo_id) = row;

    // Get clone_path
    let clone_path =
        sqlx::query_scalar::<_, Option<String>>("SELECT clone_path FROM repos WHERE id = $1")
            .bind(repo_id)
            .fetch_one(&state.pool)
            .await?
            .ok_or_else(|| AppError::BadRequest("Repo not cloned".into()))?;

    // git show {sha}:{path} for file content
    let file_content = std::process::Command::new("git")
        .args(["show", &format!("{commit_sha}:{file_path}")])
        .current_dir(&clone_path)
        .output()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !file_content.status.success() {
        return Err(AppError::NotFound("File not found at this commit".into()));
    }

    let content = String::from_utf8_lossy(&file_content.stdout);
    let content_lines: Vec<&str> = content.lines().collect();

    // git blame --porcelain
    let blame_output = std::process::Command::new("git")
        .args(["blame", "--porcelain", &commit_sha, "--", &file_path])
        .current_dir(&clone_path)
        .output()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let blame_text = String::from_utf8_lossy(&blame_output.stdout);
    let blame_map = parse_porcelain_blame(&blame_text);

    // Collect all unique commit SHAs from blame (these are the commits that actually
    // last touched each line). We look up attributions for ALL of them, not just the
    // viewed commit, so lines from older AI sessions also show attribution.
    let blame_shas: Vec<String> = blame_map
        .values()
        .map(|b| b.commit_sha.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Resolve blame SHAs to commit IDs in our DB
    let sha_to_commit_id: std::collections::HashMap<String, Uuid> = if !blame_shas.is_empty() {
        sqlx::query_as::<_, (String, Uuid)>(
            "SELECT commit_sha, id FROM commits WHERE repo_id = $1 AND commit_sha = ANY($2)",
        )
        .bind(repo_id)
        .bind(&blame_shas)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect()
    } else {
        std::collections::HashMap::new()
    };

    let all_commit_ids: Vec<Uuid> = sha_to_commit_id.values().copied().collect();

    // Load attributions for ALL commits that touched this file
    let attributions = sqlx::query_as::<_, (Uuid, Option<Uuid>, i32, i32, f32)>(
        "SELECT ca.commit_id, ca.session_id, ca.line_start, ca.line_end, ca.confidence
         FROM commit_attributions ca
         JOIN sessions s ON ca.session_id = s.id
         WHERE ca.commit_id = ANY($1) AND ca.file_path = $2",
    )
    .bind(&all_commit_ids)
    .bind(&file_path)
    .fetch_all(&state.pool)
    .await?;

    // Load session short IDs
    let session_ids: Vec<Uuid> = attributions.iter().filter_map(|a| a.1).collect();
    let session_short_ids: std::collections::HashMap<Uuid, String> = if !session_ids.is_empty() {
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id, LEFT(session_id, 8) FROM sessions WHERE id = ANY($1)",
        )
        .bind(&session_ids)
        .fetch_all(&state.pool)
        .await?
        .into_iter()
        .collect()
    } else {
        std::collections::HashMap::new()
    };

    // Build line-by-line output
    let mut lines = Vec::new();
    for (i, line_content) in content_lines.iter().enumerate() {
        let line_num = i + 1;
        let blame_info = blame_map.get(&line_num);
        let git_author = blame_info.map(|b| b.author.clone());

        // Find the commit ID for this line's blame SHA
        let line_commit_id = blame_info
            .and_then(|b| sha_to_commit_id.get(&b.commit_sha))
            .copied();

        // Check attributions for this line.
        // Strategy:
        // 1. First try: exact line range match from any commit's attribution
        // 2. Fallback: if git blame points to a commit that has ANY attribution for
        //    this file, use the best one. This handles lines that shifted due to later
        //    edits (stored line ranges become stale after insertions/deletions).
        let mut best_session: Option<Uuid> = None;
        let mut best_confidence: Option<f32> = None;

        // Pass 1: exact line range match
        for (cid, sid, start, end, conf) in &attributions {
            if line_num as i32 >= *start && line_num as i32 <= *end {
                let is_blame_commit = line_commit_id == Some(*cid);
                let is_better = match best_confidence {
                    None => true,
                    Some(bc) => is_blame_commit || *conf > bc,
                };
                if is_better {
                    best_session = *sid;
                    best_confidence = Some(*conf);
                }
            }
        }

        // Pass 2: if no exact match, check if blame commit has any attribution for this file
        if best_session.is_none() {
            if let Some(blame_cid) = line_commit_id {
                for (cid, sid, _start, _end, conf) in &attributions {
                    if *cid == blame_cid {
                        let is_better = match best_confidence {
                            None => true,
                            Some(bc) => *conf > bc,
                        };
                        if is_better {
                            best_session = *sid;
                            best_confidence = Some(*conf);
                        }
                    }
                }
            }
        }

        lines.push(AttributionLine {
            line_number: line_num,
            content: line_content.to_string(),
            git_author,
            session_id: best_session.map(|s| s.to_string()),
            session_short_id: best_session.and_then(|s| session_short_ids.get(&s).cloned()),
            confidence: best_confidence,
        });
    }

    Ok(Json(AttributionResponse {
        file_path,
        commit_sha,
        lines,
    }))
}

struct BlameInfo {
    author: String,
    commit_sha: String,
}

fn parse_porcelain_blame(text: &str) -> std::collections::HashMap<usize, BlameInfo> {
    let mut map = std::collections::HashMap::new();
    let mut current_line: usize = 0;
    let mut current_author = String::new();
    let mut current_sha = String::new();

    for line in text.lines() {
        // Header line: {sha} {orig_line} {final_line} [num_lines]
        if line.len() >= 40 && line.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                current_sha = parts[0].to_string();
                current_line = parts[2].parse().unwrap_or(0);
            }
        } else if let Some(author) = line.strip_prefix("author ") {
            current_author = author.to_string();
        } else if line.starts_with('\t') {
            // Content line — record the author and commit sha for this line number
            if current_line > 0 {
                map.insert(
                    current_line,
                    BlameInfo {
                        author: current_author.clone(),
                        commit_sha: current_sha.clone(),
                    },
                );
            }
        }
    }

    map
}

// ── Branches ────────────────────────────────────────────────────────

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
    >(
        "SELECT
            bt.branch,
            MAX(bt.tag) AS tag,
            COUNT(DISTINCT bt.commit_id) AS commits_count,
            COUNT(DISTINCT ca.session_id) AS sessions_count,
            COALESCE(SUM(DISTINCT s.estimated_cost_usd), 0) AS total_cost,
            MAX(bt.tracking_type) AS status,
            MAX(bt.tracked_at) AS last_activity
         FROM branch_tracking bt
         JOIN commits c ON bt.commit_id = c.id
         JOIN repos r ON c.repo_id = r.id
         LEFT JOIN commit_attributions ca ON ca.commit_id = c.id
         LEFT JOIN sessions s ON ca.session_id = s.id
         WHERE r.org_id = $1
           AND ($2::uuid IS NULL OR c.repo_id = $2)
         GROUP BY bt.branch
         ORDER BY last_activity DESC NULLS LAST",
    )
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
