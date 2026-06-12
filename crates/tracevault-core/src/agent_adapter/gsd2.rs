use chrono::{DateTime, Utc};

use crate::streaming::{ExtractedFileChange, StreamEventType};

use super::{AgentAdapter, FileChangeRecord, ParsedTranscriptRecord, TokenUsage};

/// Adapter for GSD2 (pi/GSD-2).
///
/// GSD2 integrates via a TypeScript extension loaded in-process, not shell hooks.
/// Events are POSTed directly to the TraceVault HTTP API by the extension.
///
/// Transcript chunks from GSD2 carry assistant/user messages from the
/// AgentEndEvent.messages array, plus tool_execution_end events.
///
/// File changes come from Write/Edit tool results (content available in
/// ToolExecutionEndEvent.result for Write, and diff via Edit details).
pub struct Gsd2Adapter;

impl AgentAdapter for Gsd2Adapter {
    fn name(&self) -> &str {
        "gsd2"
    }

    fn display_name(&self) -> &str {
        "GSD 2"
    }

    fn wire_protocol_version(&self) -> u32 {
        2
    }

    fn map_event_type(&self, hook_event_name: &str) -> StreamEventType {
        match hook_event_name {
            "session_start" => StreamEventType::SessionStart,
            "stop" | "session_end" | "session_shutdown" => StreamEventType::SessionEnd,
            _ => StreamEventType::ToolUse,
        }
    }

    /// GSD2 extension sends file changes via transcript chunks (tool_execution_end
    /// events for Write/Edit tools). Hook events don't carry file content.
    fn is_file_modifying(&self, _tool_name: &str) -> bool {
        false
    }

    fn provides_transcript_file_changes(&self) -> bool {
        true
    }

    /// Extract file changes from GSD2 transcript chunks.
    ///
    /// GSD2 extension sends tool_execution_end events as transcript chunks:
    /// ```json
    /// {
    ///   "type": "tool_execution_end",
    ///   "toolCallId": "...",
    ///   "toolName": "write",
    ///   "result": { "filePath": "src/main.rs", "content": "..." },
    ///   "isError": false,
    ///   "timestamp": "2026-05-19T..."
    /// }
    /// ```
    fn file_changes_from_transcript(
        &self,
        chunk: &serde_json::Value,
        fallback_timestamp: DateTime<Utc>,
    ) -> Vec<FileChangeRecord> {
        let chunk_type = chunk.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if chunk_type != "tool_execution_end" {
            return vec![];
        }

        let is_error = chunk
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_error {
            return vec![];
        }

        let tool_name = chunk.get("toolName").and_then(|v| v.as_str()).unwrap_or("");
        let result = match chunk.get("result") {
            Some(r) => r,
            None => return vec![],
        };

        let timestamp = chunk
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(fallback_timestamp);

        let change = match tool_name {
            "write" => {
                let file_path = match result.get("filePath").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => return vec![],
                };
                let content = result.get("content").and_then(|v| v.as_str()).unwrap_or("");
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(content.as_bytes());
                let hash = hex::encode(hasher.finalize());
                let diff_text = content
                    .lines()
                    .map(|l| format!("+{}", l))
                    .collect::<Vec<_>>()
                    .join("\n");
                ExtractedFileChange {
                    file_path,
                    change_type: "create".to_string(),
                    diff_text: if diff_text.is_empty() {
                        None
                    } else {
                        Some(diff_text)
                    },
                    content_hash: Some(hash),
                }
            }
            "edit" => {
                let file_path = match result.get("filePath").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => return vec![],
                };
                // GSD2 edit result carries oldString/newString
                let old = result
                    .get("oldString")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let new = result
                    .get("newString")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let diff_text = format!("--- {}\n+++ {}", old, new);
                ExtractedFileChange {
                    file_path,
                    change_type: "edit".to_string(),
                    diff_text: Some(diff_text),
                    content_hash: None,
                }
            }
            _ => return vec![],
        };

        vec![FileChangeRecord {
            change,
            tool_name: tool_name.to_string(),
            tool_input: chunk.get("args").cloned(),
            timestamp,
        }]
    }

    /// Extract token usage from GSD2 transcript chunks.
    ///
    /// GSD2 extension sends agent_end chunks containing the last assistant
    /// message's usage stats:
    /// ```json
    /// {
    ///   "type": "agent_end",
    ///   "usage": {
    ///     "input": 1234, "output": 456,
    ///     "cacheRead": 789, "cacheWrite": 100
    ///   },
    ///   "model": "claude-sonnet-4-5",
    ///   "timestamp": "..."
    /// }
    /// ```
    fn extract_token_usage(&self, chunk: &serde_json::Value) -> Option<TokenUsage> {
        let chunk_type = chunk.get("type")?.as_str()?;
        if chunk_type != "agent_end" {
            return None;
        }
        let usage = chunk.get("usage")?;
        Some(TokenUsage {
            input_tokens: usage.get("input").and_then(|v| v.as_i64()).unwrap_or(0),
            output_tokens: usage.get("output").and_then(|v| v.as_i64()).unwrap_or(0),
            cache_read_tokens: usage.get("cacheRead").and_then(|v| v.as_i64()).unwrap_or(0),
            cache_write_tokens: usage
                .get("cacheWrite")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        })
    }

    fn extract_model(&self, chunk: &serde_json::Value) -> Option<String> {
        let chunk_type = chunk.get("type")?.as_str()?;
        if chunk_type != "agent_end" && chunk_type != "session_start" {
            return None;
        }
        chunk.get("model")?.as_str().map(|s| s.to_string())
    }

    /// GSD2 transcript records from agent_end message arrays and tool executions.
    fn parse_transcript_record(&self, chunk: &serde_json::Value) -> Option<ParsedTranscriptRecord> {
        let chunk_type = chunk.get("type")?.as_str()?;
        let timestamp = chunk
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);

        match chunk_type {
            "assistant_message" => {
                let text = chunk.get("text").and_then(|v| v.as_str()).map(String::from);
                let model = chunk
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                Some(ParsedTranscriptRecord {
                    record_type: "assistant".to_string(),
                    timestamp,
                    content_types: vec!["text".to_string()],
                    tool_name: None,
                    text,
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model,
                })
            }
            "user_message" => {
                let text = chunk.get("text").and_then(|v| v.as_str()).map(String::from);
                Some(ParsedTranscriptRecord {
                    record_type: "user".to_string(),
                    timestamp,
                    content_types: vec!["text".to_string()],
                    tool_name: None,
                    text,
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            "tool_execution_end" => {
                let tool_name = chunk
                    .get("toolName")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let is_error = chunk
                    .get("isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // Show a brief summary — truncate large results
                let result_text = chunk.get("result").map(|r| {
                    let s = serde_json::to_string(r).unwrap_or_default();
                    if s.len() > 500 {
                        let truncated: String = s.chars().take(500).collect();
                        format!("{}...", truncated)
                    } else {
                        s
                    }
                });
                let display = if is_error {
                    result_text.map(|t| format!("[error] {}", t))
                } else {
                    result_text
                };
                Some(ParsedTranscriptRecord {
                    record_type: "assistant".to_string(),
                    timestamp,
                    content_types: vec!["tool_use".to_string()],
                    tool_name,
                    text: display,
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            "agent_end" => {
                // Summary record with token usage
                let usage = chunk.get("usage");
                let input = usage.and_then(|u| u.get("input")).and_then(|v| v.as_i64());
                let output = usage.and_then(|u| u.get("output")).and_then(|v| v.as_i64());
                let cache_read = usage
                    .and_then(|u| u.get("cacheRead"))
                    .and_then(|v| v.as_i64());
                let cache_write = usage
                    .and_then(|u| u.get("cacheWrite"))
                    .and_then(|v| v.as_i64());
                let model = chunk
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                Some(ParsedTranscriptRecord {
                    record_type: "system".to_string(),
                    timestamp,
                    content_types: vec!["agent_end".to_string()],
                    tool_name: None,
                    text: model.as_ref().map(|m| format!("turn complete ({})", m)),
                    raw_input_tokens: input,
                    raw_output_tokens: output,
                    raw_cache_read_tokens: cache_read,
                    raw_cache_write_tokens: cache_write,
                    model,
                })
            }
            _ => None,
        }
    }

    /// GSD2 does not install shell hooks — integration is via a TypeScript extension.
    fn install_hooks(&self, _project_root: &std::path::Path) -> std::io::Result<()> {
        Ok(())
    }
}
