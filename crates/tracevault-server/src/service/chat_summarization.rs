use crate::llm::StoryLlm;
use crate::service::chat_chunking::extract_text_from_chunk;

const MAX_TRANSCRIPT_CONTEXT: usize = 60_000;

pub struct SessionMetadataForSummary {
    pub repo_name: String,
    pub user_email: Option<String>,
    pub model: Option<String>,
    pub duration_ms: i64,
    pub total_tool_calls: i32,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub fn build_summary_prompt(
    transcript_text: &str,
    session_metadata: &SessionMetadataForSummary,
) -> String {
    format!(
        r#"Summarize this AI coding session in 200-300 words. Include:
- What was worked on (the main task/goal)
- Key decisions made
- Files and components touched
- Outcome (completed, in-progress, blocked)

Session metadata:
- Repository: {repo}
- User: {user}
- Model: {model}
- Duration: {duration}
- Tool calls: {tool_calls}
- Started: {started}

Transcript (may be truncated):
{transcript}

Write a clear, factual summary. Do not include greetings or filler."#,
        repo = session_metadata.repo_name,
        user = session_metadata.user_email.as_deref().unwrap_or("unknown"),
        model = session_metadata.model.as_deref().unwrap_or("unknown"),
        duration = format_duration_ms(session_metadata.duration_ms),
        tool_calls = session_metadata.total_tool_calls,
        started = session_metadata
            .started_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string()),
        transcript = transcript_text,
    )
}

pub fn flatten_transcript(chunks: &[(i32, serde_json::Value)]) -> String {
    let mut text = String::new();
    for (_, data) in chunks {
        let chunk_text = extract_text_from_chunk(data);
        if text.len() + chunk_text.len() > MAX_TRANSCRIPT_CONTEXT {
            let remaining = MAX_TRANSCRIPT_CONTEXT - text.len();
            text.push_str(&chunk_text[..remaining.min(chunk_text.len())]);
            text.push_str("\n[...truncated]");
            break;
        }
        text.push_str(&chunk_text);
        text.push('\n');
    }
    text
}

pub async fn generate_summary(
    llm: &dyn StoryLlm,
    chunks: &[(i32, serde_json::Value)],
    metadata: &SessionMetadataForSummary,
) -> Result<String, String> {
    let transcript_text = flatten_transcript(chunks);
    let prompt = build_summary_prompt(&transcript_text, metadata);
    llm.generate(&prompt, 1024).await
}

fn format_duration_ms(ms: i64) -> String {
    let secs = ms / 1000;
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
