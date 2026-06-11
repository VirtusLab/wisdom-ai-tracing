use tracevault_core::software::extract_software;
use tracevault_core::streaming::{
    extract_file_change, is_file_modifying_tool, StreamEventRequest, StreamEventResponse,
    StreamEventType,
};
use uuid::Uuid;

use crate::error::AppError;
use crate::repo::events::{
    EventRepo, InsertFileChange, InsertTranscriptChunk, UpsertAiToolUsage, UpsertSoftwareUsage,
};
use crate::repo::sessions::{SessionRepo, TokenBatch, UpsertSession};
use crate::AppState;

pub struct StreamService;

impl StreamService {
    pub async fn process(
        state: &AppState,
        org_id: Uuid,
        repo_id: Uuid,
        user_id: Uuid,
        req: StreamEventRequest,
    ) -> Result<StreamEventResponse, AppError> {
        // 1. Validate protocol version
        if req.protocol_version != 1 && req.protocol_version != 2 {
            return Err(AppError::BadRequest(format!(
                "Unsupported protocol_version: {}, expected 1 or 2",
                req.protocol_version
            )));
        }

        // 2. Determine tool name: v2 uses req.tool, v1 defaults to "claude-code"
        let tool = if req.protocol_version >= 2 {
            req.tool.clone()
        } else {
            Some("claude-code".to_string())
        };

        // 3. Upsert session
        let session_db_id = SessionRepo::upsert(
            &state.pool,
            &UpsertSession {
                org_id,
                repo_id,
                user_id,
                session_id: req.session_id.clone(),
                model: req.model.clone(),
                cwd: req.cwd.clone(),
                tool,
                timestamp: Some(req.timestamp),
            },
        )
        .await?;

        let mut event_db_id: Option<Uuid> = None;

        // 4. Process piggybacked transcript lines on any event type
        if let Some(ref lines) = req.transcript_lines {
            if !lines.is_empty() {
                let offset = req.transcript_offset.unwrap_or(0);
                let mut batch_input: i64 = 0;
                let mut batch_output: i64 = 0;
                let mut batch_cache_read: i64 = 0;
                let mut batch_cache_write: i64 = 0;
                let mut detected_model: Option<String> = None;
                let mut message_ids: Vec<String> = Vec::new();

                for (i, line) in lines.iter().enumerate() {
                    let chunk_index = offset as i32 + i as i32;
                    let was_inserted = EventRepo::insert_transcript_chunk(
                        &state.pool,
                        &InsertTranscriptChunk {
                            session_id: session_db_id,
                            chunk_index,
                            data: line.clone(),
                        },
                    )
                    .await?;

                    // Only count tokens from newly inserted chunks to avoid
                    // double-counting when overlapping batches are sent
                    if !was_inserted {
                        continue;
                    }

                    // Extract token usage from assistant messages
                    if let Some(msg) = line.get("message") {
                        if let Some(usage) = msg.get("usage") {
                            batch_input += usage
                                .get("input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            batch_output += usage
                                .get("output_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            batch_cache_read += usage
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            batch_cache_write += usage
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                        }
                        if detected_model.is_none() {
                            detected_model =
                                msg.get("model").and_then(|v| v.as_str()).map(String::from);
                        }
                        if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
                            message_ids.push(id.to_string());
                        }
                    }
                }

                // Record assistant message ids for hook/proxy usage dedup.
                // Bulk-insert in one round trip; dedup in-memory first so a
                // batch repeating an id doesn't bloat the array.
                if !message_ids.is_empty() {
                    message_ids.sort();
                    message_ids.dedup();
                    sqlx::query(
                        "INSERT INTO session_message_ids (anthropic_message_id, org_id, session_id) \
                         SELECT mid, $2, $3 FROM UNNEST($1::text[]) AS mid \
                         ON CONFLICT (org_id, anthropic_message_id) DO NOTHING",
                    )
                    .bind(&message_ids)
                    .bind(org_id)
                    .bind(session_db_id)
                    .execute(&state.pool)
                    .await?;
                }

                // Update session token counts and cost if we found usage data
                let has_tokens = batch_input > 0
                    || batch_output > 0
                    || batch_cache_read > 0
                    || batch_cache_write > 0;
                if has_tokens {
                    let model_name = detected_model.as_deref().unwrap_or("unknown");
                    // input_tokens from the API includes cache_read and cache_write,
                    // subtract to get fresh (non-cached) input only
                    let fresh_input = (batch_input - batch_cache_read - batch_cache_write).max(0);
                    let pricing =
                        crate::pricing::fetch_pricing_for_model(&state.pool, model_name, None)
                            .await;
                    let batch_cost = crate::pricing::estimate_cost_with_pricing(
                        &pricing,
                        fresh_input,
                        batch_output,
                        batch_cache_read,
                        batch_cache_write,
                    );

                    SessionRepo::update_tokens(
                        &state.pool,
                        session_db_id,
                        &TokenBatch {
                            input_tokens: fresh_input,
                            output_tokens: batch_output,
                            cache_read_tokens: batch_cache_read,
                            cache_write_tokens: batch_cache_write,
                            estimated_cost_usd: batch_cost,
                            model: detected_model,
                        },
                    )
                    .await?;
                }
            }
        }

        match req.event_type {
            // 5. ToolUse events
            StreamEventType::ToolUse => {
                // Need an ordering key: a UUIDv7 (current clients) or a legacy
                // event_index (older clients). Reject events that carry neither.
                if req.event_index.is_none() && req.event_uuid.is_none() {
                    return Err(AppError::BadRequest(
                        "ToolUse events require event_uuid (or legacy event_index)".to_string(),
                    ));
                }

                let tool_name = req.tool_name.as_deref().unwrap_or("");
                let store_response = is_file_modifying_tool(tool_name);

                let inserted_id = EventRepo::insert_tool_event(
                    &state.pool,
                    &crate::repo::events::InsertToolEvent {
                        session_id: session_db_id,
                        event_index: req.event_index,
                        event_uuid: req.event_uuid,
                        tool_name: req.tool_name.clone(),
                        tool_input: req.tool_input.clone(),
                        tool_response: if store_response {
                            req.tool_response.clone()
                        } else {
                            None
                        },
                        tool_is_error: req.tool_is_error,
                        timestamp: Some(req.timestamp),
                        hook_event_name: req.hook_event_name.clone(),
                        tool_use_id: req.tool_use_id.clone(),
                    },
                )
                .await?;

                if let Some(eid) = inserted_id {
                    event_db_id = Some(eid);

                    // Extract file changes for file-modifying tools
                    if is_file_modifying_tool(tool_name) {
                        if let Some(ref tool_input) = req.tool_input {
                            if let Some(change) = extract_file_change(tool_name, tool_input) {
                                EventRepo::insert_file_change(
                                    &state.pool,
                                    &InsertFileChange {
                                        session_id: session_db_id,
                                        event_id: eid,
                                        file_path: change.file_path,
                                        change_type: change.change_type,
                                        diff_text: change.diff_text,
                                        content_hash: change.content_hash,
                                        timestamp: Some(req.timestamp),
                                    },
                                )
                                .await?;
                            }
                        }
                    }

                    // Increment total_tool_calls
                    SessionRepo::increment_tool_calls(&state.pool, session_db_id).await?;

                    // Extract software usage from Bash commands
                    if tool_name == "Bash" {
                        if let Some(ref tool_input) = req.tool_input {
                            if let Some(command) =
                                tool_input.get("command").and_then(|v| v.as_str())
                            {
                                let software = extract_software(command);
                                for sw in software {
                                    if let Err(e) = EventRepo::upsert_software_usage(
                                        &state.pool,
                                        &UpsertSoftwareUsage {
                                            org_id,
                                            user_id,
                                            session_id: session_db_id,
                                            software_name: sw.clone(),
                                            timestamp: Some(req.timestamp),
                                        },
                                    )
                                    .await
                                    {
                                        tracing::warn!(
                                            "Failed to upsert software usage for '{}': {}",
                                            sw,
                                            e
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // Extract AI tool usage (MCP servers and skill groups)
                    if let Some(ai_tool) = extract_ai_tool(tool_name, req.tool_input.as_ref()) {
                        if let Err(e) = EventRepo::upsert_ai_tool_usage(
                            &state.pool,
                            &UpsertAiToolUsage {
                                org_id,
                                user_id,
                                session_id: session_db_id,
                                tool_category: ai_tool.0.clone(),
                                tool_name: ai_tool.1.clone(),
                                timestamp: Some(req.timestamp),
                            },
                        )
                        .await
                        {
                            tracing::warn!(
                                "Failed to upsert AI tool usage for '{}/{}': {}",
                                ai_tool.0,
                                ai_tool.1,
                                e
                            );
                        }
                    }
                }
            }

            // 6. Transcript events -- lines already processed above via piggybacking
            StreamEventType::Transcript => {}

            // 7. SessionEnd
            StreamEventType::SessionEnd => {
                if let Some(ref stats) = req.final_stats {
                    SessionRepo::complete_with_stats(
                        &state.pool,
                        session_db_id,
                        Some(req.timestamp),
                        stats,
                    )
                    .await?;
                } else {
                    SessionRepo::complete_minimal(&state.pool, session_db_id, Some(req.timestamp))
                        .await?;
                }

                // Seal session if signing is enabled
                if let Err(e) = crate::service::sealing::SealingService::seal_session_final(
                    &state.pool,
                    session_db_id,
                    org_id,
                    state.encryption_key.as_deref(),
                    "session_end",
                )
                .await
                {
                    tracing::warn!("Failed to seal session {session_db_id} on end: {e}");
                }

                // Notify ingest hooks that the session is finalized.
                let fin_ctx = crate::plugins::SessionFinalizedContext {
                    org_id,
                    repo_id,
                    user_id,
                    session_db_id,
                    model: req.model.clone(),
                };
                for hook in state.plugins.ingest_hooks.clone() {
                    hook.on_session_finalized(state, &fin_ctx).await;
                }

                // Trigger chat indexing if enabled
                if state.extensions.features.chat_search {
                    if let Some(ref emb_svc) = state.embedding_service {
                        let pool = state.pool.clone();
                        let emb = emb_svc.clone();
                        let encryption = state.extensions.encryption.clone();
                        tokio::spawn(async move {
                            let llm = crate::api::orgs::resolve_chat_llm_from_pool(
                                &pool,
                                org_id,
                                encryption.as_ref(),
                            )
                            .await;
                            if let Some(llm) = llm {
                                if let Err(e) =
                                    crate::service::chat_indexing::ChatIndexingService::index_session(
                                        &pool,
                                        &emb,
                                        llm.as_ref(),
                                        session_db_id,
                                    )
                                    .await
                                {
                                    tracing::warn!(
                                        "Background chat indexing failed for session {session_db_id}: {e}"
                                    );
                                }
                            }
                        });
                    }
                }
            }

            // 8. SessionStart -- session already upserted above
            StreamEventType::SessionStart => {}

            // 9. VerificationPhaseStart — record timestamp; only the latest matters
            StreamEventType::VerificationPhaseStart => {
                sqlx::query(include_str!(
                    "../repo/sql/update_session_verification_phase.sql"
                ))
                .bind(req.timestamp)
                .bind(session_db_id)
                .execute(&state.pool)
                .await
                .map_err(AppError::from)?;
            }
        }

        Ok(StreamEventResponse {
            session_db_id,
            event_db_id,
            status: "ok".to_string(),
        })
    }
}

/// Extract AI tool category and name from a tool call.
/// Returns Some(("mcp_server", "server_name")) or Some(("skill_group", "namespace")).
/// Returns None if the tool is not an MCP tool or skill.
pub fn extract_ai_tool(
    tool_name: &str,
    tool_input: Option<&serde_json::Value>,
) -> Option<(String, String)> {
    // MCP tools: tool_name starts with "mcp__", second segment is server name
    if let Some(rest) = tool_name.strip_prefix("mcp__") {
        if let Some(server_name) = rest.split("__").next() {
            if !server_name.is_empty() {
                return Some(("mcp_server".to_string(), server_name.to_string()));
            }
        }
        return None;
    }

    // Skills: tool_name is "Skill", skill name is in tool_input.skill
    if tool_name == "Skill" {
        if let Some(input) = tool_input {
            if let Some(skill) = input.get("skill").and_then(|v| v.as_str()) {
                let group = skill.split(':').next().unwrap_or(skill);
                if !group.is_empty() {
                    return Some(("skill_group".to_string(), group.to_string()));
                }
            }
        }
        return None;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ai_tool_mcp_valid() {
        let result = extract_ai_tool("mcp__postgres__query", None);
        assert_eq!(result, Some(("mcp_server".into(), "postgres".into())));
    }

    #[test]
    fn extract_ai_tool_mcp_empty_server() {
        let result = extract_ai_tool("mcp____query", None);
        assert!(result.is_none());
    }

    #[test]
    fn extract_ai_tool_skill_with_namespace() {
        let input = serde_json::json!({"skill": "superpowers:brainstorm"});
        let result = extract_ai_tool("Skill", Some(&input));
        assert_eq!(result, Some(("skill_group".into(), "superpowers".into())));
    }

    #[test]
    fn extract_ai_tool_skill_no_colon() {
        let input = serde_json::json!({"skill": "commit"});
        let result = extract_ai_tool("Skill", Some(&input));
        assert_eq!(result, Some(("skill_group".into(), "commit".into())));
    }

    #[test]
    fn extract_ai_tool_skill_no_input() {
        assert!(extract_ai_tool("Skill", None).is_none());
    }

    #[test]
    fn extract_ai_tool_read_returns_none() {
        assert!(extract_ai_tool("Read", None).is_none());
    }

    #[test]
    fn extract_ai_tool_bash_returns_none() {
        assert!(extract_ai_tool("Bash", None).is_none());
    }
}
