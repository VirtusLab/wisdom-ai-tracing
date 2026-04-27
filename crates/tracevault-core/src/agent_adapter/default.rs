use crate::streaming::{ExtractedFileChange, StreamEventType};

use super::{AgentAdapter, ParsedTranscriptRecord, TokenUsage};

pub struct DefaultAdapter;

impl AgentAdapter for DefaultAdapter {
    fn name(&self) -> &str {
        "default"
    }

    fn map_event_type(&self, hook_event_name: &str) -> StreamEventType {
        match hook_event_name {
            "SessionStart" => StreamEventType::SessionStart,
            "Stop" | "SessionEnd" => StreamEventType::SessionEnd,
            _ => StreamEventType::ToolUse,
        }
    }

    fn is_file_modifying(&self, _tool_name: &str) -> bool {
        false
    }

    fn extract_file_changes(
        &self,
        _tool_name: &str,
        _tool_input: &serde_json::Value,
    ) -> Vec<ExtractedFileChange> {
        Vec::new()
    }

    fn extract_token_usage(&self, _chunk: &serde_json::Value) -> Option<TokenUsage> {
        None
    }

    fn extract_model(&self, _chunk: &serde_json::Value) -> Option<String> {
        None
    }

    fn parse_transcript_record(
        &self,
        _chunk: &serde_json::Value,
    ) -> Option<ParsedTranscriptRecord> {
        None
    }
}
