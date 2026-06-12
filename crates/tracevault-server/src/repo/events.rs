use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct InsertToolEvent {
    pub session_id: Uuid,
    /// Legacy ordering counter from older clients; `None` for clients that send
    /// `event_uuid` instead.
    pub event_index: Option<i32>,
    /// Client-minted UUIDv7 ordering key; `None` for legacy clients.
    pub event_uuid: Option<Uuid>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_response: Option<serde_json::Value>,
    pub tool_is_error: Option<bool>,
    pub timestamp: Option<DateTime<Utc>>,
    pub hook_event_name: Option<String>,
    pub tool_use_id: Option<String>,
}

pub struct InsertFileChange {
    pub session_id: Uuid,
    pub event_id: Uuid,
    pub file_path: String,
    pub change_type: String,
    pub diff_text: Option<String>,
    pub content_hash: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
}

pub struct InsertTranscriptChunk {
    pub session_id: Uuid,
    pub chunk_index: i32,
    pub data: serde_json::Value,
}

pub struct UpsertSoftwareUsage {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub software_name: String,
    pub timestamp: Option<DateTime<Utc>>,
}

pub struct UpsertAiToolUsage {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub tool_category: String,
    pub tool_name: String,
    pub timestamp: Option<DateTime<Utc>>,
}

pub struct EventRepo;

impl EventRepo {
    /// INSERT INTO events ... ON CONFLICT DO NOTHING RETURNING id.
    /// Returns None if the row already existed (conflict).
    ///
    /// Dedup is keyed on the event's intrinsic identity when a `tool_use_id` is
    /// present — `(session_id, tool_use_id, hook_event_name)` — so a re-fired or
    /// re-delivered hook collapses regardless of its client-assigned
    /// `event_index`, and two concurrently-raced parallel-tool events (which can
    /// share an `event_index` because the CLI counter is not atomic) both persist
    /// because their `tool_use_id`s differ. Legacy rows without a `tool_use_id`
    /// fall back to the historical `(session_id, event_index)` dedup.
    pub async fn insert_tool_event(
        pool: &PgPool,
        req: &InsertToolEvent,
    ) -> Result<Option<Uuid>, AppError> {
        let sql = if req.tool_use_id.is_some() {
            include_str!("sql/insert_tool_event_by_identity.sql")
        } else {
            include_str!("sql/insert_tool_event.sql")
        };
        let id: Option<Uuid> = sqlx::query_scalar(sql)
            .bind(req.session_id)
            .bind(req.event_index)
            .bind(&req.tool_name)
            .bind(&req.tool_input)
            .bind(&req.tool_response)
            .bind(req.tool_is_error)
            .bind(req.timestamp)
            .bind(&req.hook_event_name)
            .bind(&req.tool_use_id)
            .bind(req.event_uuid)
            .fetch_optional(pool)
            .await?;
        Ok(id)
    }

    /// INSERT INTO file_changes ... ON CONFLICT DO NOTHING.
    pub async fn insert_file_change(pool: &PgPool, req: &InsertFileChange) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO file_changes (session_id, event_id, file_path, change_type, diff_text, content_hash, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (event_id, file_path) DO NOTHING",
        )
        .bind(req.session_id)
        .bind(req.event_id)
        .bind(&req.file_path)
        .bind(&req.change_type)
        .bind(&req.diff_text)
        .bind(&req.content_hash)
        .bind(req.timestamp)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// INSERT INTO transcript_chunks ... ON CONFLICT DO NOTHING.
    /// Returns true if the row was actually inserted (not a duplicate).
    pub async fn insert_transcript_chunk(
        pool: &PgPool,
        req: &InsertTranscriptChunk,
    ) -> Result<bool, AppError> {
        let result = sqlx::query(
            "INSERT INTO transcript_chunks (session_id, chunk_index, data)
             VALUES ($1, $2, $3)
             ON CONFLICT (session_id, chunk_index) DO NOTHING",
        )
        .bind(req.session_id)
        .bind(req.chunk_index)
        .bind(&req.data)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// INSERT INTO user_software_usage ... ON CONFLICT DO UPDATE.
    pub async fn upsert_software_usage(
        pool: &PgPool,
        req: &UpsertSoftwareUsage,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_software_usage (org_id, user_id, session_id, software_name, first_seen_at, last_seen_at)
             VALUES ($1, $2, $3, $4, $5, $5)
             ON CONFLICT (session_id, software_name) DO UPDATE SET
                 usage_count = user_software_usage.usage_count + 1,
                 last_seen_at = EXCLUDED.last_seen_at",
        )
        .bind(req.org_id)
        .bind(req.user_id)
        .bind(req.session_id)
        .bind(&req.software_name)
        .bind(req.timestamp)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Aggregate tool call stats for a session that occurred *after* a given
    /// timestamp (i.e. inside the verification window).
    ///
    /// Only `PostToolUse` rows are counted (completed calls). The phase-opening
    /// `tracevault verify-start` Bash call is excluded even though its Post
    /// fires inside the window — it is infrastructure, not work.
    ///
    /// Returns a map of tool_name → ToolCallStats.
    pub async fn get_verification_phase_tool_call_stats(
        pool: &PgPool,
        session_db_id: Uuid,
        window_started_at: DateTime<Utc>,
    ) -> Result<
        std::collections::HashMap<String, tracevault_core::policy_eval::ToolCallStats>,
        AppError,
    > {
        let rows: Vec<(String, Option<String>, Option<bool>)> = sqlx::query_as(include_str!(
            "sql/get_verification_phase_tool_call_stats.sql"
        ))
        .bind(session_db_id)
        .bind(window_started_at)
        .fetch_all(pool)
        .await?;

        let mut map: std::collections::HashMap<
            String,
            tracevault_core::policy_eval::ToolCallStats,
        > = std::collections::HashMap::new();

        for (tool_name, command, is_error) in rows {
            // Exclude the phase-opening command itself: a standalone
            // `tracevault verify-start` Bash call legitimately lands in-window
            // (its Post fires after the marker) but is infrastructure, not work.
            if tool_name == "Bash" {
                if let Some(cmd) = command.as_deref() {
                    if tracevault_core::bash_command::is_standalone_tracevault_subcommand(
                        cmd,
                        "verify-start",
                    ) {
                        continue;
                    }
                }
            }
            let entry = map.entry(tool_name).or_default();
            entry.total += 1;
            if is_error == Some(false) {
                entry.successful += 1;
            }
        }
        Ok(map)
    }

    /// INSERT INTO user_ai_tool_usage ... ON CONFLICT DO UPDATE.
    pub async fn upsert_ai_tool_usage(
        pool: &PgPool,
        req: &UpsertAiToolUsage,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_ai_tool_usage (org_id, user_id, session_id, tool_category, tool_name, first_seen_at, last_seen_at)
             VALUES ($1, $2, $3, $4, $5, $6, $6)
             ON CONFLICT (session_id, tool_category, tool_name) DO UPDATE SET
                 usage_count = user_ai_tool_usage.usage_count + 1,
                 last_seen_at = EXCLUDED.last_seen_at",
        )
        .bind(req.org_id)
        .bind(req.user_id)
        .bind(req.session_id)
        .bind(&req.tool_category)
        .bind(&req.tool_name)
        .bind(req.timestamp)
        .execute(pool)
        .await?;
        Ok(())
    }
}
