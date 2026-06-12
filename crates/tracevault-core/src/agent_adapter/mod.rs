pub mod claude_code;
pub mod codex;
mod default;

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::hooks::HookResponse;
use crate::streaming::{ExtractedFileChange, StreamEventType};

use self::default::DefaultAdapter;

/// File change with all metadata `stream.rs` needs to persist it. Both
/// hook-sourced and transcript-sourced extractions return this same shape so
/// the persistence layer doesn't need to know which mechanism produced it.
#[derive(Debug, Clone)]
pub struct FileChangeRecord {
    pub change: ExtractedFileChange,
    pub tool_name: String,
    pub tool_input: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedTranscriptRecord {
    pub record_type: String,
    pub timestamp: Option<String>,
    pub content_types: Vec<String>,
    pub tool_name: Option<String>,
    pub text: Option<String>,
    pub raw_input_tokens: Option<i64>,
    pub raw_output_tokens: Option<i64>,
    pub raw_cache_read_tokens: Option<i64>,
    pub raw_cache_write_tokens: Option<i64>,
    pub model: Option<String>,
}

pub trait AgentAdapter: Send + Sync {
    fn name(&self) -> &str;
    /// Human-readable label shown in CLI/UI (e.g. "Claude Code", "Codex").
    /// Defaults to `name()` for adapters that don't override.
    fn display_name(&self) -> &str {
        self.name()
    }
    /// Repo-relative path of the file `install_hooks` writes to
    /// (e.g. ".claude/settings.json"). Empty for adapters that don't install
    /// hooks (the default adapter).
    fn hooks_install_path(&self) -> &str {
        ""
    }
    /// Wire protocol version the CLI should send for this adapter.
    /// Claude Code stays on v1 to keep its request bytes identical to the
    /// pre-multi-agent main; new adapters use v2 (which carries `tool` over
    /// the wire instead of the server hardcoding "claude-code").
    fn wire_protocol_version(&self) -> u32 {
        2
    }
    /// Capability flag: should the server fire `update_tokens` when a
    /// transcript batch contained a model but zero token usage? Defaults to
    /// `false` to preserve main's Claude path bit-for-bit (where the gate was
    /// solely on token presence). Codex sets this to `true` because its
    /// model-only chunks can legitimately precede usage.
    fn persists_model_without_usage(&self) -> bool {
        false
    }
    fn map_event_type(&self, hook_event_name: &str) -> StreamEventType;
    fn is_file_modifying(&self, tool_name: &str) -> bool;
    /// File changes derived from a hook ToolUse event (Claude Write/Edit).
    /// Default: none. Override for adapters whose file ops appear in the hook's
    /// `tool_input` payload itself.
    fn file_changes_from_hook(
        &self,
        _tool_name: &str,
        _tool_input: &serde_json::Value,
        _timestamp: DateTime<Utc>,
    ) -> Vec<FileChangeRecord> {
        vec![]
    }
    /// Capability flag: does this adapter source file changes from transcript
    /// chunks? When `false` (default), `stream.rs` skips
    /// `file_changes_from_transcript` entirely — preserving the pre-multi-agent
    /// code path for adapters like Claude Code that have no transcript-side
    /// file extraction.
    fn provides_transcript_file_changes(&self) -> bool {
        false
    }
    /// File changes discovered inside a transcript chunk (Codex apply_patch).
    /// Only called when `provides_transcript_file_changes()` returns `true`.
    /// `fallback_timestamp` is used when the chunk itself has no parseable
    /// timestamp.
    fn file_changes_from_transcript(
        &self,
        _chunk: &serde_json::Value,
        _fallback_timestamp: DateTime<Utc>,
    ) -> Vec<FileChangeRecord> {
        vec![]
    }
    fn extract_token_usage(&self, chunk: &serde_json::Value) -> Option<TokenUsage>;
    fn extract_model(&self, chunk: &serde_json::Value) -> Option<String>;
    fn parse_transcript_record(&self, chunk: &serde_json::Value) -> Option<ParsedTranscriptRecord>;
    /// Install agent-specific hooks into `project_root`. Default: no-op.
    fn install_hooks(&self, _project_root: &Path) -> std::io::Result<()> {
        Ok(())
    }
    /// Response to print on stdout after the hook stream finishes.
    /// Default: empty `{}` (e.g. Codex). Claude Code overrides with `suppress_output: true`.
    fn hook_response(&self) -> HookResponse {
        HookResponse::empty()
    }
}

pub struct AgentAdapterRegistry {
    adapters: HashMap<String, Arc<dyn AgentAdapter>>,
    default: Arc<dyn AgentAdapter>,
}

impl AgentAdapterRegistry {
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Arc<dyn AgentAdapter>> = HashMap::new();
        let claude: Arc<dyn AgentAdapter> = Arc::new(claude_code::ClaudeCodeAdapter);
        adapters.insert("claude-code".to_string(), Arc::clone(&claude));
        adapters.insert("claude".to_string(), claude);
        adapters.insert("codex".to_string(), Arc::new(codex::CodexAdapter));
        Self {
            adapters,
            default: Arc::new(DefaultAdapter),
        }
    }

    pub fn get(&self, name: &str) -> &dyn AgentAdapter {
        self.adapters
            .get(name)
            .map(|a| a.as_ref())
            .unwrap_or(self.default.as_ref())
    }

    /// Returns Some only for explicitly registered agents — None for unknown.
    pub fn try_get(&self, name: &str) -> Option<&dyn AgentAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }
}

impl Default for AgentAdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
