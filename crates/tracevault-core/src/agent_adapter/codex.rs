use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::Path;

use crate::streaming::{ExtractedFileChange, StreamEventType};

use super::{AgentAdapter, ParsedTranscriptRecord, TokenUsage};

/// Adapter for OpenAI Codex CLI.
///
/// Codex file modifications come exclusively through transcript chunks
/// (custom_tool_call with apply_patch), NOT through hook ToolUse events.
/// The hook events only carry shell commands like `pwd`, `git status`, etc.
pub struct CodexAdapter;

fn hooks_json() -> serde_json::Value {
    serde_json::json!({
        "SessionStart": [{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "tracevault stream --agent codex --event session-start",
                "timeout": 10,
                "statusMessage": "TraceVault: streaming session start"
            }]
        }],
        "PreToolUse": [{
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": "tracevault stream --agent codex --event pre-tool-use",
                "timeout": 10,
                "statusMessage": "TraceVault: streaming pre-tool event"
            }]
        }],
        "PostToolUse": [{
            "matcher": "Bash",
            "hooks": [{
                "type": "command",
                "command": "tracevault stream --agent codex --event post-tool-use",
                "timeout": 10,
                "statusMessage": "TraceVault: streaming post-tool event"
            }]
        }],
        "Stop": [{
            "hooks": [{
                "type": "command",
                "command": "tracevault stream --agent codex --event stop",
                "timeout": 10,
                "statusMessage": "TraceVault: finalizing session"
            }]
        }]
    })
}

impl AgentAdapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn map_event_type(&self, hook_event_name: &str) -> StreamEventType {
        match hook_event_name {
            "SessionStart" => StreamEventType::SessionStart,
            "Stop" => StreamEventType::SessionEnd,
            _ => StreamEventType::ToolUse,
        }
    }

    /// Codex hook events never carry file-modifying tool calls.
    /// File changes are extracted from transcript via `extract_file_changes_from_transcript`.
    fn is_file_modifying(&self, _tool_name: &str) -> bool {
        false
    }

    /// Not used for Codex — file changes come from transcript, not hook events.
    fn extract_file_changes(
        &self,
        _tool_name: &str,
        _tool_input: &serde_json::Value,
    ) -> Vec<ExtractedFileChange> {
        vec![]
    }

    /// Extract file changes from Codex transcript chunks.
    /// Handles `response_item` with `payload.type: "custom_tool_call"` and `name: "apply_patch"`.
    fn extract_file_changes_from_transcript(
        &self,
        chunk: &serde_json::Value,
    ) -> Vec<ExtractedFileChange> {
        let payload = match chunk.get("payload") {
            Some(p) => p,
            None => return vec![],
        };

        let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if payload_type != "custom_tool_call" {
            return vec![];
        }

        let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name != "apply_patch" {
            return vec![];
        }

        let input = match payload.get("input").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return vec![],
        };

        parse_codex_patch(input)
    }

    fn extract_token_usage(&self, chunk: &serde_json::Value) -> Option<TokenUsage> {
        let top_type = chunk.get("type")?.as_str()?;
        if top_type != "event_msg" {
            return None;
        }
        let payload = chunk.get("payload")?;
        let payload_type = payload.get("type")?.as_str()?;
        if payload_type != "token_count" {
            return None;
        }
        let usage = payload.get("info")?.get("last_token_usage")?;
        Some(TokenUsage {
            input_tokens: usage.get("input_tokens")?.as_i64()?,
            output_tokens: usage.get("output_tokens")?.as_i64()?,
            cache_read_tokens: usage
                .get("cached_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            cache_write_tokens: 0,
        })
    }

    fn extract_model(&self, chunk: &serde_json::Value) -> Option<String> {
        let top_type = chunk.get("type")?.as_str()?;
        if top_type != "turn_context" {
            return None;
        }
        chunk
            .get("payload")?
            .get("model")?
            .as_str()
            .map(|s| s.to_string())
    }

    fn install_hooks(&self, project_root: &Path) -> io::Result<()> {
        let codex_dir = project_root.join(".codex");
        fs::create_dir_all(&codex_dir)?;

        let hooks_path = codex_dir.join("hooks.json");
        let mut config: serde_json::Value = if hooks_path.exists() {
            let content = fs::read_to_string(&hooks_path)?;
            serde_json::from_str(&content).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to parse .codex/hooks.json: {e}"),
                )
            })?
        } else {
            serde_json::json!({})
        };

        let config_obj = config.as_object_mut().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                ".codex/hooks.json is not a JSON object",
            )
        })?;
        config_obj.insert("hooks".to_string(), hooks_json());

        let formatted = serde_json::to_string_pretty(&config)
            .map_err(|e| io::Error::other(format!("Failed to serialize hooks: {e}")))?;
        fs::write(&hooks_path, formatted)?;
        Ok(())
    }

    fn parse_transcript_record(&self, chunk: &serde_json::Value) -> Option<ParsedTranscriptRecord> {
        let top_type = chunk.get("type")?.as_str()?;
        let timestamp = chunk
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(String::from);

        match top_type {
            "event_msg" => self.parse_event_msg(chunk, &timestamp),
            "response_item" => self.parse_response_item(chunk, &timestamp),
            // turn_context, session_meta — ingestion-only, not for display
            _ => None,
        }
    }
}

impl CodexAdapter {
    fn parse_event_msg(
        &self,
        chunk: &serde_json::Value,
        timestamp: &Option<String>,
    ) -> Option<ParsedTranscriptRecord> {
        let payload = chunk.get("payload")?;
        let payload_type = payload.get("type")?.as_str()?;

        match payload_type {
            "agent_message" => {
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some(ParsedTranscriptRecord {
                    record_type: "assistant".to_string(),
                    timestamp: timestamp.clone(),
                    content_types: vec!["text".to_string()],
                    tool_name: None,
                    text: content,
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            "user_message" => {
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                Some(ParsedTranscriptRecord {
                    record_type: "user".to_string(),
                    timestamp: timestamp.clone(),
                    content_types: vec!["text".to_string()],
                    tool_name: None,
                    text: content,
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            // token_count, task_started — ingestion-only
            _ => None,
        }
    }

    fn parse_response_item(
        &self,
        chunk: &serde_json::Value,
        timestamp: &Option<String>,
    ) -> Option<ParsedTranscriptRecord> {
        let payload = chunk.get("payload")?;
        let payload_type = payload.get("type")?.as_str()?;

        match payload_type {
            "local_shell_call" => {
                let command = payload
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let output = payload.get("output").and_then(|v| v.as_str()).unwrap_or("");
                let text = format!("$ {}\n{}", command, output);
                Some(ParsedTranscriptRecord {
                    record_type: "assistant".to_string(),
                    timestamp: timestamp.clone(),
                    content_types: vec!["tool_use".to_string()],
                    tool_name: Some("Bash".to_string()),
                    text: Some(text),
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            "message" => {
                let role = payload.get("role")?.as_str()?;
                // Skip system/developer messages (permissions, instructions)
                if role == "developer" {
                    return None;
                }
                let record_type = if role == "assistant" {
                    "assistant"
                } else {
                    "user"
                };
                let text = payload
                    .get("content")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|block| {
                                let block_type = block.get("type").and_then(|v| v.as_str())?;
                                if block_type == "input_text" || block_type == "output_text" {
                                    let t = block.get("text").and_then(|v| v.as_str())?;
                                    // Codex injects system context into the user role wrapped
                                    // in known XML tags (see openai/codex protocol.rs).
                                    // Skip only those — a blunt `starts_with('<')` would also
                                    // drop legitimate user questions about HTML/JSX/XML snippets.
                                    if role == "user" && is_codex_system_prompt(t) {
                                        return None;
                                    }
                                    Some(t.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    })
                    .filter(|s| !s.is_empty());
                // Skip if no meaningful text
                text.as_ref()?;
                Some(ParsedTranscriptRecord {
                    record_type: record_type.to_string(),
                    timestamp: timestamp.clone(),
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
            "custom_tool_call" => {
                let name = payload
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool");
                let input = payload.get("input").and_then(|v| v.as_str()).unwrap_or("");
                // Truncate long patches for display (char-safe to avoid UTF-8 panic)
                let display_input = if input.len() > 500 {
                    let truncated: String = input.chars().take(500).collect();
                    format!("{}...", truncated)
                } else {
                    input.to_string()
                };
                Some(ParsedTranscriptRecord {
                    record_type: "assistant".to_string(),
                    timestamp: timestamp.clone(),
                    content_types: vec!["tool_use".to_string()],
                    tool_name: Some(name.to_string()),
                    text: Some(display_input),
                    raw_input_tokens: None,
                    raw_output_tokens: None,
                    raw_cache_read_tokens: None,
                    raw_cache_write_tokens: None,
                    model: None,
                })
            }
            // reasoning — encrypted, skip
            _ => None,
        }
    }
}

/// Known opening tags Codex uses to inject system context into the user role.
/// Sourced from openai/codex `codex-rs/protocol/src/protocol.rs`.
const CODEX_SYSTEM_PROMPT_TAGS: &[&str] = &[
    "<user_instructions>",
    "<environment_context>",
    "<apps_instructions>",
    "<skills_instructions>",
    "<plugins_instructions>",
    "<collaboration_mode>",
    "<realtime_conversation>",
];

/// Returns true if `text` starts with one of the known Codex system-prompt tags
/// (after trimming leading whitespace).
fn is_codex_system_prompt(text: &str) -> bool {
    let trimmed = text.trim_start();
    CODEX_SYSTEM_PROMPT_TAGS
        .iter()
        .any(|tag| trimmed.starts_with(tag))
}

/// Parse Codex's custom apply_patch format into file changes.
pub fn parse_codex_patch(patch: &str) -> Vec<ExtractedFileChange> {
    let mut changes = Vec::new();
    let mut current_file: Option<String> = None;
    let mut current_type: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in patch.lines() {
        if line == "*** Begin Patch" || line == "*** End Patch" {
            flush_pending(
                &mut changes,
                &mut current_file,
                &mut current_type,
                &mut current_lines,
            );
            continue;
        }

        if let Some(path) = line.strip_prefix("*** Add File: ") {
            flush_pending(
                &mut changes,
                &mut current_file,
                &mut current_type,
                &mut current_lines,
            );
            current_file = Some(path.to_string());
            current_type = Some("create".to_string());
        } else if let Some(path) = line.strip_prefix("*** Update File: ") {
            flush_pending(
                &mut changes,
                &mut current_file,
                &mut current_type,
                &mut current_lines,
            );
            current_file = Some(path.to_string());
            current_type = Some("edit".to_string());
        } else if let Some(path) = line.strip_prefix("*** Delete File: ") {
            flush_pending(
                &mut changes,
                &mut current_file,
                &mut current_type,
                &mut current_lines,
            );
            current_file = Some(path.to_string());
            current_type = Some("delete".to_string());
        } else if current_file.is_some() {
            current_lines.push(line.to_string());
        }
    }

    flush_pending(
        &mut changes,
        &mut current_file,
        &mut current_type,
        &mut current_lines,
    );
    changes
}

fn flush_pending(
    changes: &mut Vec<ExtractedFileChange>,
    file: &mut Option<String>,
    kind: &mut Option<String>,
    lines: &mut Vec<String>,
) {
    if let (Some(file_path), Some(change_type)) = (file.take(), kind.take()) {
        changes.push(build_file_change(&file_path, &change_type, lines));
        lines.clear();
    }
}

fn build_file_change(file_path: &str, change_type: &str, lines: &[String]) -> ExtractedFileChange {
    match change_type {
        "create" => {
            let content: String = lines
                .iter()
                .map(|l| l.strip_prefix('+').unwrap_or(l))
                .collect::<Vec<_>>()
                .join("\n");
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let hash = hex::encode(hasher.finalize());
            let diff_text = lines.join("\n");
            ExtractedFileChange {
                file_path: file_path.to_string(),
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
            let diff_text = lines.join("\n");
            ExtractedFileChange {
                file_path: file_path.to_string(),
                change_type: "edit".to_string(),
                diff_text: if diff_text.is_empty() {
                    None
                } else {
                    Some(diff_text)
                },
                content_hash: None,
            }
        }
        "delete" => ExtractedFileChange {
            file_path: file_path.to_string(),
            change_type: "delete".to_string(),
            diff_text: None,
            content_hash: None,
        },
        _ => ExtractedFileChange {
            file_path: file_path.to_string(),
            change_type: change_type.to_string(),
            diff_text: None,
            content_hash: None,
        },
    }
}
