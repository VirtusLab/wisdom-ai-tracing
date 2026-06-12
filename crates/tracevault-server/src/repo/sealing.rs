use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

pub struct SealingRepo;

impl SealingRepo {
    /// Atomically get prev chain hash, compute chain hash, and insert commit seal.
    /// Uses an advisory lock to prevent concurrent commits for the same org from
    /// reading the same prev_chain_hash.
    pub async fn insert_commit_seal_with_chain(
        pool: &PgPool,
        commit_id: Uuid,
        org_id: Uuid,
        record_hash: &str,
        signature: &str,
    ) -> Result<(Uuid, String), AppError> {
        let mut tx = pool.begin().await?;

        // Advisory lock scoped to this transaction, keyed on org_id
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(org_id.as_u128() as i64)
            .execute(&mut *tx)
            .await?;

        // Get previous chain hash
        let prev_chain_hash = sqlx::query_scalar::<_, Option<String>>(
            "SELECT cs.chain_hash
             FROM commit_seals cs
             JOIN commits c ON c.id = cs.commit_id
             JOIN repos r ON r.id = c.repo_id
             WHERE r.org_id = $1
             ORDER BY cs.sealed_at DESC
             LIMIT 1",
        )
        .bind(org_id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();

        // Compute chain hash
        let chain_hash = {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            if let Some(ref prev) = prev_chain_hash {
                hasher.update(prev.as_bytes());
            }
            hasher.update(record_hash.as_bytes());
            hex::encode(hasher.finalize())
        };

        // Insert commit seal
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO commit_seals (commit_id, record_hash, chain_hash, prev_chain_hash, signature)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (commit_id) DO UPDATE SET
               record_hash = EXCLUDED.record_hash,
               chain_hash = EXCLUDED.chain_hash,
               prev_chain_hash = EXCLUDED.prev_chain_hash,
               signature = EXCLUDED.signature,
               sealed_at = NOW()
             RETURNING id",
        )
        .bind(commit_id)
        .bind(record_hash)
        .bind(&chain_hash)
        .bind(prev_chain_hash.as_deref())
        .bind(signature)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok((id, chain_hash))
    }

    /// Insert a session seal. Returns the seal UUID.
    /// Multiple seals per session are allowed (commit snapshots + final seal).
    pub async fn insert_session_seal(
        pool: &PgPool,
        session_id: Uuid,
        record_hash: &str,
        signature: &str,
        seal_type: &str,
        commit_seal_id: Option<Uuid>,
        sealed_at: DateTime<Utc>,
    ) -> Result<Uuid, AppError> {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO session_seals (session_id, record_hash, signature, seal_type, commit_seal_id, sealed_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING id",
        )
        .bind(session_id)
        .bind(record_hash)
        .bind(signature)
        .bind(seal_type)
        .bind(commit_seal_id)
        .bind(sealed_at)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    /// Load commit data for record hash computation.
    #[allow(clippy::type_complexity)]
    pub async fn get_commit_for_sealing(
        pool: &PgPool,
        commit_id: Uuid,
    ) -> Result<
        Option<(
            String,                    // commit_sha
            Option<String>,            // branch
            Option<String>,            // author
            Option<String>,            // message
            Option<serde_json::Value>, // diff_data
            Option<DateTime<Utc>>,     // committed_at
            Option<serde_json::Value>, // attribution
        )>,
        AppError,
    > {
        let row = sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<serde_json::Value>,
                Option<DateTime<Utc>>,
                Option<serde_json::Value>,
            ),
        >(
            "SELECT commit_sha, branch, author, message, diff_data, committed_at, attribution
             FROM commits WHERE id = $1",
        )
        .bind(commit_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Resolve org_id from a commit via its repo.
    pub async fn get_org_id_for_commit(
        pool: &PgPool,
        commit_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        let row = sqlx::query_scalar(
            "SELECT r.org_id
             FROM commits c
             JOIN repos r ON r.id = c.repo_id
             WHERE c.id = $1",
        )
        .bind(commit_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get distinct session IDs attributed to a commit.
    pub async fn get_attributed_sessions(
        pool: &PgPool,
        commit_id: Uuid,
    ) -> Result<Vec<Uuid>, AppError> {
        let rows: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT session_id FROM commit_attributions WHERE commit_id = $1",
        )
        .bind(commit_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Load session data for record hash computation.
    #[allow(clippy::type_complexity)]
    pub async fn get_session_for_sealing(
        pool: &PgPool,
        session_id: Uuid,
    ) -> Result<Option<(String, Uuid, i64, i64, i64, i64, f64)>, AppError> {
        let row = sqlx::query_as::<_, (String, Uuid, i64, i64, i64, i64, f64)>(
            "SELECT session_id, repo_id, input_tokens, output_tokens,
                    cache_read_tokens, cache_write_tokens, estimated_cost_usd
             FROM sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Load all events for a session, ordered by event_index.
    #[allow(clippy::type_complexity)]
    pub async fn get_session_events_for_sealing(
        pool: &PgPool,
        session_id: Uuid,
    ) -> Result<
        Vec<(
            i32,                       // event_index
            String,                    // event_type
            Option<String>,            // tool_name
            Option<serde_json::Value>, // tool_input
            Option<serde_json::Value>, // tool_response
        )>,
        AppError,
    > {
        let rows = sqlx::query_as::<
            _,
            (
                i32,
                String,
                Option<String>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
            ),
        >(
            // `, id ASC` keeps the seal order deterministic now that two raced
            // parallel-tool events can share an event_index (see migration 035).
            // Forward-safe: sessions sealed before 035 have unique indices, so
            // the tiebreak never changes their order.
            "SELECT event_index, event_type, tool_name, tool_input, tool_response
             FROM events WHERE session_id = $1
             ORDER BY event_index ASC, id ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Load file changes for a session, ordered by file_path and id.
    pub async fn get_session_file_changes_for_sealing(
        pool: &PgPool,
        session_id: Uuid,
    ) -> Result<Vec<(String, String, Option<String>)>, AppError> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
            "SELECT file_path, change_type, content_hash
             FROM file_changes WHERE session_id = $1
             ORDER BY file_path ASC, id ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Check if signing is enabled for an org.
    pub async fn is_signing_enabled(pool: &PgPool, org_id: Uuid) -> Result<bool, AppError> {
        let row: Option<bool> = sqlx::query_scalar(
            "SELECT signing_enabled FROM org_compliance_settings WHERE org_id = $1",
        )
        .bind(org_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.unwrap_or(false))
    }

    /// Find sessions inactive for > N minutes with no final seal.
    /// Returns (session_db_id, org_id).
    pub async fn find_stale_sessions(
        pool: &PgPool,
        inactive_minutes: i64,
    ) -> Result<Vec<(Uuid, Uuid)>, AppError> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT s.id, r.org_id
             FROM sessions s
             JOIN repos r ON r.id = s.repo_id
             JOIN org_compliance_settings ocs ON ocs.org_id = r.org_id
             WHERE ocs.signing_enabled = true
               AND s.status = 'active'
               AND s.updated_at < NOW() - ($1::text || ' minutes')::interval
               AND NOT EXISTS (
                   SELECT 1 FROM session_seals ss
                   WHERE ss.session_id = s.id
                     AND ss.seal_type IN ('session_end', 'timeout')
               )",
        )
        .bind(inactive_minutes)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Check if a session already has a final seal (session_end or timeout).
    pub async fn has_final_seal(pool: &PgPool, session_id: Uuid) -> Result<bool, AppError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM session_seals
                WHERE session_id = $1
                  AND seal_type IN ('session_end', 'timeout')
             )",
        )
        .bind(session_id)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    /// Get all session seals for an org, ordered by sealed_at for chain verification.
    #[allow(clippy::type_complexity)]
    pub async fn get_session_seals_for_verification(
        pool: &PgPool,
        org_id: Uuid,
    ) -> Result<
        Vec<(
            Uuid,           // seal_id
            Uuid,           // session_id
            String,         // record_hash
            Option<String>, // signature
            String,         // seal_type
            DateTime<Utc>,  // sealed_at
        )>,
        AppError,
    > {
        let rows = sqlx::query_as::<
            _,
            (Uuid, Uuid, String, Option<String>, String, DateTime<Utc>),
        >(
            "SELECT ss.id, ss.session_id, ss.record_hash, ss.signature, ss.seal_type, ss.sealed_at
             FROM session_seals ss
             JOIN sessions s ON s.id = ss.session_id
             JOIN repos r ON r.id = s.repo_id
             WHERE r.org_id = $1
             ORDER BY ss.sealed_at ASC",
        )
        .bind(org_id)
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }
}
