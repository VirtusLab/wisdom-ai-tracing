use chrono::{TimeZone, Utc};
use serde_json::json;
use tracevault_core::agent_adapter::AgentAdapterRegistry;
use tracevault_core::streaming::StreamEventType;

fn ts() -> chrono::DateTime<chrono::Utc> {
    Utc.with_ymd_and_hms(2026, 4, 29, 10, 0, 0).unwrap()
}

#[test]
fn registry_unknown_agent_returns_default() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("unknown-agent");
    assert_eq!(adapter.name(), "default");
}

#[test]
fn default_adapter_extract_token_usage_returns_none() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("nope");
    let chunk =
        serde_json::json!({"type": "assistant", "message": {"usage": {"input_tokens": 100}}});
    assert!(adapter.extract_token_usage(&chunk).is_none());
}

#[test]
fn registry_dispatches_to_claude_code() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    assert_eq!(adapter.name(), "claude-code");
}

#[test]
fn claude_code_map_event_types() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    assert!(matches!(
        adapter.map_event_type("SessionStart"),
        StreamEventType::SessionStart
    ));
    assert!(matches!(
        adapter.map_event_type("Notification"),
        StreamEventType::SessionStart
    ));
    assert!(matches!(
        adapter.map_event_type("Stop"),
        StreamEventType::SessionEnd
    ));
    assert!(matches!(
        adapter.map_event_type("PostToolUse"),
        StreamEventType::ToolUse
    ));
}

#[test]
fn claude_code_file_changes_from_hook_write() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let input = json!({"file_path": "src/main.rs", "content": "fn main() {}"});
    let records = adapter.file_changes_from_hook("Write", &input, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].change.file_path, "src/main.rs");
    assert_eq!(records[0].change.change_type, "create");
    assert!(records[0].change.content_hash.is_some());
    assert_eq!(records[0].tool_name, "Write");
    assert_eq!(records[0].timestamp, ts());
}

#[test]
fn claude_code_file_changes_from_hook_edit() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let input = json!({"file_path": "src/lib.rs", "old_string": "old", "new_string": "new"});
    let records = adapter.file_changes_from_hook("Edit", &input, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].change.change_type, "edit");
    assert_eq!(records[0].tool_name, "Edit");
}

#[test]
fn claude_code_read_returns_empty() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let input = json!({"file_path": "src/lib.rs"});
    assert!(adapter
        .file_changes_from_hook("Read", &input, ts())
        .is_empty());
}

#[test]
fn claude_code_is_file_modifying() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    assert!(adapter.is_file_modifying("Write"));
    assert!(adapter.is_file_modifying("Edit"));
    assert!(adapter.is_file_modifying("Bash"));
    assert!(!adapter.is_file_modifying("Read"));
}

#[test]
fn claude_code_extract_token_usage() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "assistant",
        "message": {
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 200,
                "cache_read_input_tokens": 500,
                "cache_creation_input_tokens": 100
            }
        }
    });
    let usage = adapter.extract_token_usage(&chunk).unwrap();
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 200);
    assert_eq!(usage.cache_read_tokens, 500);
    assert_eq!(usage.cache_write_tokens, 100);
}

#[test]
fn claude_code_extract_model() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "assistant", "message": {"model": "claude-opus-4-6"}});
    assert_eq!(
        adapter.extract_model(&chunk).as_deref(),
        Some("claude-opus-4-6")
    );
}

#[test]
fn claude_code_parse_assistant_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "assistant",
        "timestamp": "2026-03-23T13:17:16Z",
        "message": {
            "model": "claude-opus-4-6",
            "content": [
                {"type": "text", "text": "Hello world"},
                {"type": "tool_use", "name": "Write", "input": {}}
            ],
            "usage": {
                "input_tokens": 100, "output_tokens": 50,
                "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0
            }
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert_eq!(record.model.as_deref(), Some("claude-opus-4-6"));
    assert!(record.text.as_ref().unwrap().contains("Hello world"));
    assert!(record.content_types.contains(&"text".to_string()));
    assert!(record.content_types.contains(&"tool_use".to_string()));
    assert_eq!(record.tool_name.as_deref(), Some("Write"));
    assert_eq!(record.raw_input_tokens, Some(100));
}

#[test]
fn claude_code_parse_user_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "user", "timestamp": "2026-03-23T13:17:00Z", "message": {"content": "Fix the bug"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "user");
    assert_eq!(record.text.as_deref(), Some("Fix the bug"));
}

#[test]
fn claude_code_parse_user_tool_result_read() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "user", "toolUseResult": {"file": {"filePath": "src/main.rs"}}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.tool_name.as_deref(), Some("Read: src/main.rs"));
}

#[test]
fn claude_code_parse_user_tool_result_bash_uses_top_level_stdout() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "user",
        "toolUseResult": {
            "stdout": "ok\n",
            "stderr": "",
            "interrupted": false
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.tool_name.as_deref(), Some("Bash"));
}

#[test]
fn claude_code_parse_user_tool_result_glob_uses_top_level_filenames() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "user",
        "toolUseResult": {
            "filenames": ["src/main.rs", "src/lib.rs"]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.tool_name.as_deref(), Some("Glob"));
}

#[test]
fn claude_code_parse_user_tool_result_block_reads_content_field() {
    // tool_result blocks store the body under `content`, not `text`.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "user",
        "message": {
            "content": [
                {"type": "tool_result", "tool_use_id": "abc", "content": "command output"}
            ]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.text.as_deref(), Some("command output"));
    assert!(record.content_types.contains(&"tool_result".to_string()));
}

#[test]
fn claude_code_parse_user_text_block() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "user",
        "message": {
            "content": [{"type": "text", "text": "follow up"}]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.text.as_deref(), Some("follow up"));
}

#[test]
fn claude_code_parse_assistant_thinking_uses_prefix_and_double_newline() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({
        "type": "assistant",
        "message": {
            "model": "claude-opus-4-6",
            "content": [
                {"type": "thinking", "thinking": "let me think"},
                {"type": "text", "text": "the answer"}
            ]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(
        record.text.as_deref(),
        Some("[thinking] let me think\n\nthe answer")
    );
}

#[test]
fn claude_code_parse_assistant_missing_message_returns_empty_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "assistant", "timestamp": "2026-04-29T10:00:00Z"});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert!(record.text.is_none());
    assert!(record.content_types.is_empty());
    assert!(record.model.is_none());
}

#[test]
fn claude_code_parse_progress_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk =
        json!({"type": "progress", "data": {"hookName": "tracevault", "hookEvent": "PostToolUse"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "progress");
    assert_eq!(record.text.as_deref(), Some("PostToolUse: tracevault"));
    assert_eq!(record.tool_name.as_deref(), Some("tracevault"));
}

#[test]
fn claude_code_parse_progress_record_event_only() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "progress", "data": {"hookEvent": "PostToolUse"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.text.as_deref(), Some("PostToolUse"));
    assert!(record.tool_name.is_none());
}

#[test]
fn claude_code_parse_progress_record_missing_event_yields_no_text() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "progress", "data": {"hookName": "tracevault"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert!(record.text.is_none());
    assert_eq!(record.tool_name.as_deref(), Some("tracevault"));
}

#[test]
fn claude_code_parse_system_record_turn_duration() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "system", "subtype": "turn_duration", "durationMs": 5000.0});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "system");
    assert_eq!(record.text.as_deref(), Some("turn_duration: 5.0s"));
    assert_eq!(record.content_types, vec!["turn_duration".to_string()]);
}

#[test]
fn claude_code_parse_system_record_stop_hook_summary() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "system", "subtype": "stop_hook_summary", "hookCount": 3});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.text.as_deref(), Some("stop_hook_summary: 3 hooks"));
}

#[test]
fn claude_code_parse_system_record_unknown_subtype_keeps_subtype_in_content_types() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "system", "subtype": "init"});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.text.as_deref(), Some("init"));
    assert_eq!(record.content_types, vec!["init".to_string()]);
}

#[test]
fn codex_map_event_types() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    assert!(matches!(
        adapter.map_event_type("SessionStart"),
        StreamEventType::SessionStart
    ));
    assert!(matches!(
        adapter.map_event_type("Stop"),
        StreamEventType::SessionEnd
    ));
    assert!(matches!(
        adapter.map_event_type("PostToolUse"),
        StreamEventType::ToolUse
    ));
}

#[test]
fn codex_extract_token_usage() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 2000, "output_tokens": 300, "cached_input_tokens": 1500}}}});
    let usage = adapter.extract_token_usage(&chunk).unwrap();
    assert_eq!(usage.input_tokens, 2000);
    assert_eq!(usage.output_tokens, 300);
    assert_eq!(usage.cache_read_tokens, 1500);
    assert_eq!(usage.cache_write_tokens, 0);
}

#[test]
fn codex_extract_token_usage_non_token_chunk_returns_none() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "agent_message"}});
    assert!(adapter.extract_token_usage(&chunk).is_none());
}

#[test]
fn codex_extract_token_usage_token_count_without_info_returns_none() {
    // token_count event with no `info` field (e.g. early/empty payload).
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "token_count"}});
    assert!(adapter.extract_token_usage(&chunk).is_none());
}

#[test]
fn codex_extract_token_usage_token_count_without_last_token_usage_returns_none() {
    // token_count event with `info` but no `last_token_usage` (metadata-only).
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "event_msg",
        "payload": {"type": "token_count", "info": {"total_tokens": 0}}
    });
    assert!(adapter.extract_token_usage(&chunk).is_none());
}

#[test]
fn codex_extract_model() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "turn_context", "payload": {"model": "codex-mini-latest"}});
    assert_eq!(
        adapter.extract_model(&chunk).as_deref(),
        Some("codex-mini-latest")
    );
}

#[test]
fn codex_extract_model_non_turn_context_returns_none() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "agent_message"}});
    assert!(adapter.extract_model(&chunk).is_none());
}

#[test]
fn codex_parse_agent_message() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "agent_message", "content": "I'll fix that bug now."}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert_eq!(record.text.as_deref(), Some("I'll fix that bug now."));
}

#[test]
fn codex_parse_user_message() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "user_message", "content": "Fix the login bug"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "user");
    assert_eq!(record.text.as_deref(), Some("Fix the login bug"));
}

#[test]
fn codex_user_message_with_html_snippet_is_kept() {
    // Legitimate user questions starting with `<` (HTML/JSX/XML) must not be
    // dropped by the system-prompt filter — only known Codex prompt tags are.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": "<div>fix this rendering</div>"}
            ]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(
        record.text.as_deref(),
        Some("<div>fix this rendering</div>")
    );
}

#[test]
fn codex_user_message_with_system_prompt_tag_is_dropped() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    for tag in [
        "<user_instructions>",
        "<environment_context>",
        "<apps_instructions>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<collaboration_mode>",
        "<realtime_conversation>",
    ] {
        let body = format!("{tag}some system context\n</tag>");
        let chunk = json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": body}]
            }
        });
        assert!(
            adapter.parse_transcript_record(&chunk).is_none(),
            "tag {tag} should be filtered out"
        );
    }
}

#[test]
fn codex_user_message_with_leading_whitespace_then_system_tag_is_dropped() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [
                {"type": "input_text", "text": "  \n<environment_context>cwd: /tmp</environment_context>"}
            ]
        }
    });
    assert!(adapter.parse_transcript_record(&chunk).is_none());
}

#[test]
fn codex_assistant_message_with_html_snippet_is_kept_regardless_of_prefix() {
    // The system-prompt filter only applies to the user role.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "assistant",
            "content": [
                {"type": "output_text", "text": "<environment_context>example</environment_context>"}
            ]
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert!(record.text.is_some());
}

#[test]
fn codex_parse_shell_call() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "response_item", "payload": {"type": "local_shell_call", "command": "cargo test", "output": "test result: ok. 5 passed"}});
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert_eq!(record.tool_name.as_deref(), Some("Bash"));
    assert!(record.text.as_ref().unwrap().contains("cargo test"));
}

#[test]
fn codex_parse_token_count_returns_none_for_display() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({"type": "event_msg", "payload": {"type": "token_count", "info": {"last_token_usage": {"input_tokens": 100, "output_tokens": 50}}}});
    assert!(adapter.parse_transcript_record(&chunk).is_none());
}

// Codex file changes are extracted from transcript, not hook events.
// These tests use parse_codex_patch directly.

#[test]
fn codex_patch_parse_add_file() {
    let changes = tracevault_core::agent_adapter::codex::parse_codex_patch(
        "*** Begin Patch\n*** Add File: src/new.rs\n+fn main() {}\n*** End Patch\n",
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file_path, "src/new.rs");
    assert_eq!(changes[0].change_type, "create");
    assert!(changes[0].content_hash.is_some());
}

#[test]
fn codex_patch_parse_update_file() {
    let changes = tracevault_core::agent_adapter::codex::parse_codex_patch(
        "*** Begin Patch\n*** Update File: src/lib.rs\n@@ fn old()\n-fn old()\n+fn new_func()\n*** End Patch\n",
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file_path, "src/lib.rs");
    assert_eq!(changes[0].change_type, "edit");
    assert!(changes[0].diff_text.is_some());
}

#[test]
fn codex_patch_parse_delete_file() {
    let changes = tracevault_core::agent_adapter::codex::parse_codex_patch(
        "*** Begin Patch\n*** Delete File: src/old.rs\n*** End Patch\n",
    );
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].file_path, "src/old.rs");
    assert_eq!(changes[0].change_type, "delete");
}

#[test]
fn codex_file_changes_from_hook_returns_empty() {
    // Codex hook events don't carry file modifications.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let input = json!({"command": "cargo build"});
    assert!(adapter
        .file_changes_from_hook("Bash", &input, ts())
        .is_empty());
}

#[test]
fn codex_is_file_modifying_always_false() {
    // Codex file changes come from transcript, not hook events.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    assert!(!adapter.is_file_modifying("Bash"));
    assert!(!adapter.is_file_modifying("Read"));
    assert!(!adapter.is_file_modifying("apply_patch"));
}

#[test]
fn codex_file_changes_from_transcript_apply_patch() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "timestamp": "2026-04-29T11:30:00Z",
        "payload": {
            "type": "custom_tool_call",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Update File: src/main.rs\n@@ fn old()\n-fn old()\n+fn new_func()\n*** End Patch\n"
        }
    });
    let records = adapter.file_changes_from_transcript(&chunk, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].change.file_path, "src/main.rs");
    assert_eq!(records[0].change.change_type, "edit");
    assert_eq!(records[0].tool_name, "apply_patch");
    assert!(records[0].tool_input.is_some());
    // chunk timestamp wins over fallback.
    assert_ne!(records[0].timestamp, ts());
}

#[test]
fn codex_file_changes_from_transcript_falls_back_when_chunk_has_no_timestamp() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {
            "type": "custom_tool_call",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Add File: x.rs\n+x\n*** End Patch\n"
        }
    });
    let records = adapter.file_changes_from_transcript(&chunk, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].timestamp, ts());
}

#[test]
fn codex_file_changes_from_transcript_non_patch_returns_empty() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {"type": "message", "role": "assistant", "content": []}
    });
    assert!(adapter
        .file_changes_from_transcript(&chunk, ts())
        .is_empty());
}

#[test]
fn codex_reasoning_record_returns_none() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "payload": {
            "type": "reasoning",
            "content": null,
            "summary": [],
            "encrypted_content": "gAAAAA..."
        }
    });
    assert!(adapter.parse_transcript_record(&chunk).is_none());
}

#[test]
fn codex_custom_tool_call_display() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("codex");
    let chunk = json!({
        "type": "response_item",
        "timestamp": "2026-04-03T17:52:42Z",
        "payload": {
            "type": "custom_tool_call",
            "name": "apply_patch",
            "input": "*** Begin Patch\n*** Update File: README.md\n@@\n old line\n+new line\n*** End Patch"
        }
    });
    let record = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(record.record_type, "assistant");
    assert_eq!(record.tool_name.as_deref(), Some("apply_patch"));
    assert!(record.text.as_ref().unwrap().contains("Update File"));
}

#[test]
fn claude_code_file_changes_from_transcript_returns_empty() {
    // Claude Code file changes come from hook events, not transcript.
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("claude-code");
    let chunk = json!({"type": "assistant", "message": {"content": []}});
    assert!(adapter
        .file_changes_from_transcript(&chunk, ts())
        .is_empty());
}

// ─── GSD2 adapter tests ─────────────────────────────────────────────────────

#[test]
fn gsd2_registry_dispatch() {
    let registry = AgentAdapterRegistry::new();
    let a = registry.get("gsd2");
    assert_eq!(a.name(), "gsd2");
    let b = registry.get("gsd-2");
    assert_eq!(b.name(), "gsd2");
}

#[test]
fn gsd2_map_event_types() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    assert!(matches!(
        adapter.map_event_type("session_start"),
        StreamEventType::SessionStart
    ));
    assert!(matches!(
        adapter.map_event_type("stop"),
        StreamEventType::SessionEnd
    ));
    assert!(matches!(
        adapter.map_event_type("session_end"),
        StreamEventType::SessionEnd
    ));
    assert!(matches!(
        adapter.map_event_type("session_shutdown"),
        StreamEventType::SessionEnd
    ));
    assert!(matches!(
        adapter.map_event_type("tool_execution_end"),
        StreamEventType::ToolUse
    ));
}

#[test]
fn gsd2_is_not_file_modifying_from_hooks() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    // GSD2 gets file changes from transcript, not hook events
    assert!(!adapter.is_file_modifying("write"));
    assert!(!adapter.is_file_modifying("edit"));
    assert!(!adapter.is_file_modifying("bash"));
}

#[test]
fn gsd2_provides_transcript_file_changes() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    assert!(adapter.provides_transcript_file_changes());
}

#[test]
fn gsd2_file_change_from_write_chunk() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "tool_execution_end",
        "toolCallId": "tc-001",
        "toolName": "write",
        "result": { "filePath": "src/lib.rs", "content": "pub fn hello() {}" },
        "isError": false,
        "timestamp": "2026-05-19T10:00:00Z"
    });
    let records = adapter.file_changes_from_transcript(&chunk, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].change.file_path, "src/lib.rs");
    assert_eq!(records[0].change.change_type, "create");
    assert!(records[0].change.content_hash.is_some());
    assert_eq!(records[0].tool_name, "write");
}

#[test]
fn gsd2_file_change_skipped_on_error() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "tool_execution_end",
        "toolCallId": "tc-002",
        "toolName": "write",
        "result": { "filePath": "src/lib.rs", "content": "pub fn hello() {}" },
        "isError": true
    });
    assert!(adapter
        .file_changes_from_transcript(&chunk, ts())
        .is_empty());
}

#[test]
fn gsd2_file_change_from_edit_chunk() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "tool_execution_end",
        "toolCallId": "tc-003",
        "toolName": "edit",
        "result": { "filePath": "src/main.rs", "oldString": "old", "newString": "new" },
        "isError": false
    });
    let records = adapter.file_changes_from_transcript(&chunk, ts());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].change.change_type, "edit");
    assert!(records[0]
        .change
        .diff_text
        .as_deref()
        .unwrap_or("")
        .contains("--- old"));
}

#[test]
fn gsd2_non_file_tool_returns_empty() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "tool_execution_end",
        "toolName": "bash",
        "result": { "output": "hello" },
        "isError": false
    });
    assert!(adapter
        .file_changes_from_transcript(&chunk, ts())
        .is_empty());
}

#[test]
fn gsd2_extract_token_usage_from_agent_end() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "agent_end",
        "usage": { "input": 1000, "output": 200, "cacheRead": 500, "cacheWrite": 50 },
        "model": "claude-sonnet-4-5"
    });
    let usage = adapter.extract_token_usage(&chunk).unwrap();
    assert_eq!(usage.input_tokens, 1000);
    assert_eq!(usage.output_tokens, 200);
    assert_eq!(usage.cache_read_tokens, 500);
    assert_eq!(usage.cache_write_tokens, 50);
}

#[test]
fn gsd2_extract_token_usage_wrong_type_returns_none() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({ "type": "tool_execution_end", "usage": { "input": 100 } });
    assert!(adapter.extract_token_usage(&chunk).is_none());
}

#[test]
fn gsd2_extract_model_from_agent_end() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({ "type": "agent_end", "model": "claude-opus-4-7" });
    assert_eq!(
        adapter.extract_model(&chunk),
        Some("claude-opus-4-7".to_string())
    );
}

#[test]
fn gsd2_extract_model_from_session_start() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({ "type": "session_start", "model": "gpt-5" });
    assert_eq!(adapter.extract_model(&chunk), Some("gpt-5".to_string()));
}

#[test]
fn gsd2_parse_assistant_message_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "assistant_message",
        "text": "Here is the fix.",
        "model": "claude-sonnet-4-5",
        "timestamp": "2026-05-19T10:00:00Z"
    });
    let rec = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(rec.record_type, "assistant");
    assert_eq!(rec.text.as_deref(), Some("Here is the fix."));
    assert_eq!(rec.model.as_deref(), Some("claude-sonnet-4-5"));
}

#[test]
fn gsd2_parse_tool_execution_record() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "tool_execution_end",
        "toolName": "bash",
        "result": { "output": "ok" },
        "isError": false,
        "timestamp": "2026-05-19T10:00:00Z"
    });
    let rec = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(rec.record_type, "assistant");
    assert_eq!(rec.content_types, vec!["tool_use"]);
    assert_eq!(rec.tool_name.as_deref(), Some("bash"));
}

#[test]
fn gsd2_parse_agent_end_record_with_usage() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let chunk = json!({
        "type": "agent_end",
        "usage": { "input": 100, "output": 50, "cacheRead": 0, "cacheWrite": 0 },
        "model": "claude-sonnet-4-5",
        "timestamp": "2026-05-19T10:00:00Z"
    });
    let rec = adapter.parse_transcript_record(&chunk).unwrap();
    assert_eq!(rec.record_type, "system");
    assert_eq!(rec.raw_input_tokens, Some(100));
    assert_eq!(rec.raw_output_tokens, Some(50));
}

#[test]
fn gsd2_install_hooks_is_noop() {
    let registry = AgentAdapterRegistry::new();
    let adapter = registry.get("gsd2");
    let dir = tempfile::tempdir().unwrap();
    // Should succeed silently — GSD2 uses an in-process extension, not shell hooks
    adapter.install_hooks(dir.path()).unwrap();
    // Nothing should be created
    assert!(!dir.path().join(".gsd2").exists());
}
