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
            let end = crate::floor_char_boundary(&chunk_text, remaining.min(chunk_text.len()));
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
    let total_segments = transcript_text.len().div_ceil(SEGMENT_SIZE);

    tracing::info!(
        "Summarizing session: repo={}, user={}, transcript_len={}, chunks={}, segments={}",
        metadata.repo_name,
        metadata.user_email.as_deref().unwrap_or("unknown"),
        transcript_text.len(),
        chunks.len(),
        total_segments,
    );

    // Small transcript: single-pass summary
    if transcript_text.len() <= SEGMENT_SIZE {
        tracing::info!("Single-pass summarization (small transcript)");
        let start = std::time::Instant::now();
        let result = llm
            .generate(&build_summary_prompt(&transcript_text, metadata), 1024)
            .await;
        tracing::info!(
            "Single-pass summarization completed in {:?}",
            start.elapsed()
        );
        return result;
    }

    // Large transcript: summarize segments, then combine
    tracing::info!("Segmented summarization: {total_segments} segments of ~{SEGMENT_SIZE} chars");
    let mut segment_summaries = Vec::new();
    let mut offset = 0;
    let mut segment_num = 1;

    while offset < transcript_text.len() {
        let end = crate::floor_char_boundary(
            &transcript_text,
            (offset + SEGMENT_SIZE).min(transcript_text.len()),
        );
        let segment = &transcript_text[offset..end];

        tracing::info!(
            "Summarizing segment {segment_num}/{total_segments} ({} chars)",
            segment.len()
        );
        let start = std::time::Instant::now();

        let prompt = format!(
            "Summarize this segment ({segment_num}) of an AI coding session transcript in 100-150 words. \
             Focus on what was done, key decisions, and files touched.\n\nTranscript segment:\n{segment}"
        );
        match llm.generate(&prompt, 512).await {
            Ok(summary) => {
                tracing::info!(
                    "Segment {segment_num}/{total_segments} completed in {:?}",
                    start.elapsed()
                );
                segment_summaries.push(summary);
            }
            Err(e) => {
                tracing::warn!(
                    "Segment {segment_num}/{total_segments} failed in {:?}: {e}",
                    start.elapsed()
                );
            }
        }

        offset = end;
        segment_num += 1;
    }

    if segment_summaries.is_empty() {
        return Err("All segment summarizations failed".to_string());
    }

    // Combine segment summaries into final summary
    tracing::info!(
        "Combining {}/{total_segments} segment summaries into final summary",
        segment_summaries.len()
    );
    let start = std::time::Instant::now();
    let combined = segment_summaries.join("\n\n");
    let prompt = build_summary_prompt(
        &format!(
            "[Combined from {count} segment summaries]\n\n{combined}",
            count = segment_summaries.len()
        ),
        metadata,
    );
    let result = llm.generate(&prompt, 1024).await;
    tracing::info!("Final summary completed in {:?}", start.elapsed());
    result
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
