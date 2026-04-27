pub mod claude_code;
pub mod codex;
mod default;

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::hooks::HookResponse;
use crate::streaming::{ExtractedFileChange, StreamEventType};

use self::default::DefaultAdapter;

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
    fn map_event_type(&self, hook_event_name: &str) -> StreamEventType;
    fn is_file_modifying(&self, tool_name: &str) -> bool;
    /// Extract file changes from a hook tool event (tool_name + tool_input)
    fn extract_file_changes(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Vec<ExtractedFileChange>;
    /// Extract file changes from a transcript chunk (e.g. Codex custom_tool_call with apply_patch).
    /// Default: no extraction. Override for agents whose file ops appear in transcript, not hook events.
    fn extract_file_changes_from_transcript(
        &self,
        _chunk: &serde_json::Value,
    ) -> Vec<ExtractedFileChange> {
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
