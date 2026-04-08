use serde_json::Value;

/// A sliding window over transcript chunks, containing concatenated text
/// and metadata about which chunk indices it spans.
pub struct ChunkWindow {
    pub chunk_start: i32,
    pub chunk_end: i32,
    pub text: String,
    pub content_preview: String,
}

/// Extract searchable text from a single JSONB transcript chunk.
///
/// Handles the following shapes:
/// - `{"type": "human", "message": {"content": "..."}}`
/// - `{"type": "assistant", "message": {"content": [{"type": "text", "text": "..."}, {"type": "tool_use", "name": "Edit", "input": {...}}]}}`
///
/// Tool inputs are truncated to 500 characters.
pub fn extract_text_from_chunk(data: &Value) -> String {
    let mut parts: Vec<String> = Vec::new();

    // Add the type as a prefix, e.g. [human] or [assistant]
    if let Some(typ) = data.get("type").and_then(|v| v.as_str()) {
        parts.push(format!("[{typ}]"));
    }

    let content = match data.get("message").and_then(|m| m.get("content")) {
        Some(c) => c,
        None => return parts.join(" "),
    };

    match content {
        Value::String(s) => {
            parts.push(s.clone());
        }
        Value::Array(arr) => {
            for block in arr {
                match block.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            parts.push(text.to_string());
                        }
                    }
                    Some("tool_use") => {
                        if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                            parts.push(format!("[tool:{name}]"));
                        }
                        if let Some(input) = block.get("input") {
                            let serialized = input.to_string();
                            if serialized.len() > 500 {
                                let end = serialized.floor_char_boundary(500);
                                parts.push(serialized[..end].to_string());
                            } else {
                                parts.push(serialized);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    parts.join(" ")
}

/// Build sliding windows over a list of chunk texts.
///
/// - `chunks`: ordered list of (chunk_index, chunk_data) pairs
/// - `window_size`: how many chunks per window
/// - `overlap`: how many chunks overlap between consecutive windows
/// - `max_text_len`: maximum character length for the concatenated window text
pub fn build_chunk_windows(
    chunks: &[(i32, &Value)],
    window_size: usize,
    overlap: usize,
    max_text_len: usize,
) -> Vec<ChunkWindow> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let step = window_size.saturating_sub(overlap).max(1);
    let mut windows = Vec::new();
    let mut start = 0;

    while start < chunks.len() {
        let end = (start + window_size).min(chunks.len());
        let window_chunks = &chunks[start..end];

        let text: String = window_chunks
            .iter()
            .map(|(_, data)| extract_text_from_chunk(data))
            .collect::<Vec<_>>()
            .join("\n");

        let truncated = if text.len() > max_text_len {
            let end = text.floor_char_boundary(max_text_len);
            text[..end].to_string()
        } else {
            text
        };

        let preview = if truncated.len() > 200 {
            let end = truncated.floor_char_boundary(200);
            truncated[..end].to_string()
        } else {
            truncated.clone()
        };

        windows.push(ChunkWindow {
            chunk_start: window_chunks.first().unwrap().0,
            chunk_end: window_chunks.last().unwrap().0,
            text: truncated,
            content_preview: preview,
        });

        start += step;
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_text_user_message() {
        let data = json!({
            "type": "human",
            "message": {
                "content": "Fix the login bug"
            }
        });

        let text = extract_text_from_chunk(&data);
        assert!(text.contains("[human]"));
        assert!(text.contains("Fix the login bug"));
    }

    #[test]
    fn extract_text_assistant_with_content_array() {
        let data = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "I'll fix the login bug"},
                    {"type": "tool_use", "name": "Edit", "input": {"file": "login.rs", "content": "fixed code"}}
                ]
            }
        });

        let text = extract_text_from_chunk(&data);
        assert!(text.contains("[assistant]"));
        assert!(text.contains("I'll fix the login bug"));
        assert!(text.contains("[tool:Edit]"));
        assert!(text.contains("login.rs"));
    }

    #[test]
    fn build_windows_basic() {
        let chunks_data: Vec<Value> = (0..10)
            .map(|i| {
                json!({
                    "type": "human",
                    "message": {"content": format!("message {i}")}
                })
            })
            .collect();

        let chunks: Vec<(i32, &Value)> = chunks_data
            .iter()
            .enumerate()
            .map(|(i, v)| (i as i32, v))
            .collect();

        let windows = build_chunk_windows(&chunks, 3, 1, 10000);

        // step = 3 - 1 = 2, so windows start at 0, 2, 4, 6, 8
        assert_eq!(windows.len(), 5);

        assert_eq!(windows[0].chunk_start, 0);
        assert_eq!(windows[0].chunk_end, 2);

        assert_eq!(windows[1].chunk_start, 2);
        assert_eq!(windows[1].chunk_end, 4);

        assert_eq!(windows[2].chunk_start, 4);
        assert_eq!(windows[2].chunk_end, 6);

        assert_eq!(windows[3].chunk_start, 6);
        assert_eq!(windows[3].chunk_end, 8);

        assert_eq!(windows[4].chunk_start, 8);
        assert_eq!(windows[4].chunk_end, 9);
    }

    #[test]
    fn build_windows_empty() {
        let chunks: Vec<(i32, &Value)> = Vec::new();
        let windows = build_chunk_windows(&chunks, 3, 1, 10000);
        assert!(windows.is_empty());
    }

    #[test]
    fn build_windows_fewer_than_window() {
        let data = json!({
            "type": "human",
            "message": {"content": "only message"}
        });
        let chunks: Vec<(i32, &Value)> = vec![(0, &data)];

        let windows = build_chunk_windows(&chunks, 3, 1, 10000);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].chunk_start, 0);
        assert_eq!(windows[0].chunk_end, 0);
        assert!(windows[0].text.contains("only message"));
    }
}
