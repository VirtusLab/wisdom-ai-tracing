use crate::llm::StoryLlm;
use crate::service::chat_chunking::extract_text_from_chunk;

const MAX_TRANSCRIPT_CONTEXT: usize = 60_000;
const SEGMENT_SIZE: usize = 15_000;

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
            let end = chunk_text.floor_char_boundary(remaining.min(chunk_text.len()));
            text.push_str(&chunk_text[..end]);
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

    // Small transcript: single-pass summary
    if transcript_text.len() <= SEGMENT_SIZE {
        let prompt = build_summary_prompt(&transcript_text, metadata);
        return llm.generate(&prompt, 1024).await;
    }

    // Large transcript: summarize segments, then combine
    let mut segment_summaries = Vec::new();
    let mut offset = 0;
    let mut segment_num = 1;

    while offset < transcript_text.len() {
        let end = transcript_text.floor_char_boundary((offset + SEGMENT_SIZE).min(transcript_text.len()));
        let segment = &transcript_text[offset..end];

        let prompt = format!(
            "Summarize this segment ({segment_num}) of an AI coding session transcript in 100-150 words. \
             Focus on what was done, key decisions, and files touched.\n\nTranscript segment:\n{segment}"
        );
        match llm.generate(&prompt, 512).await {
            Ok(summary) => segment_summaries.push(summary),
            Err(e) => {
                tracing::warn!("Segment {segment_num} summarization failed: {e}");
            }
        }

        offset = end;
        segment_num += 1;
    }

    if segment_summaries.is_empty() {
        return Err("All segment summarizations failed".to_string());
    }

    // Combine segment summaries into final summary
    let combined = segment_summaries.join("\n\n");
    let prompt = build_summary_prompt(&format!(
        "[Combined from {count} segment summaries]\n\n{combined}",
        count = segment_summaries.len()
    ), metadata);
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
