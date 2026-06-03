use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedUsage {
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyKind {
    Sse,
    Json,
}

pub struct UsageCapture {
    kind: BodyKind,
    buf: Vec<u8>,
}

impl UsageCapture {
    pub fn new(content_type: Option<&str>) -> Self {
        let kind = if content_type.unwrap_or("").contains("text/event-stream") {
            BodyKind::Sse
        } else {
            BodyKind::Json
        };
        UsageCapture {
            kind,
            buf: Vec::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    pub fn finish(self) -> Option<ParsedUsage> {
        match self.kind {
            BodyKind::Json => parse_json(&self.buf),
            BodyKind::Sse => parse_sse(&self.buf),
        }
    }
}

/// Apply the `usage` JSON object fields onto a `ParsedUsage`, respecting verbatim input_tokens.
fn apply_usage(parsed: &mut ParsedUsage, usage: &Value) {
    if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_i64()) {
        parsed.input_tokens = Some(v);
    }
    if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_i64()) {
        parsed.output_tokens = Some(v);
    }
    if let Some(v) = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
    {
        parsed.cache_read_tokens = Some(v);
    }
    if let Some(v) = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_i64())
    {
        parsed.cache_write_tokens = Some(v);
    }
}

fn parse_json(buf: &[u8]) -> Option<ParsedUsage> {
    let v: Value = serde_json::from_slice(buf).ok()?;
    let mut parsed = ParsedUsage::default();

    if let Some(model) = v.get("model").and_then(|v| v.as_str()) {
        parsed.model = Some(model.to_string());
    }
    if let Some(stop_reason) = v.get("stop_reason").and_then(|v| v.as_str()) {
        parsed.stop_reason = Some(stop_reason.to_string());
    }
    if let Some(usage) = v.get("usage") {
        apply_usage(&mut parsed, usage);
    }

    if parsed == ParsedUsage::default() {
        None
    } else {
        Some(parsed)
    }
}

fn parse_sse(buf: &[u8]) -> Option<ParsedUsage> {
    let text = std::str::from_utf8(buf).ok()?;
    let mut parsed = ParsedUsage::default();
    let mut seen_usage = false;

    for line in text.lines() {
        let trimmed = line.trim_start();
        let data = if let Some(rest) = trimmed.strip_prefix("data:") {
            rest.trim()
        } else {
            continue;
        };

        let v: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match v.get("type").and_then(|t| t.as_str()) {
            Some("message_start") => {
                if let Some(message) = v.get("message") {
                    if parsed.model.is_none() {
                        if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
                            parsed.model = Some(model.to_string());
                            seen_usage = true;
                        }
                    }
                    if let Some(usage) = message.get("usage") {
                        apply_usage(&mut parsed, usage);
                        seen_usage = true;
                    }
                }
            }
            Some("message_delta") => {
                if let Some(usage) = v.get("usage") {
                    // message_delta usage only carries output_tokens (final cumulative value)
                    if let Some(output) = usage.get("output_tokens").and_then(|v| v.as_i64()) {
                        parsed.output_tokens = Some(output);
                        seen_usage = true;
                    }
                }
                if let Some(delta) = v.get("delta") {
                    if let Some(stop_reason) = delta.get("stop_reason").and_then(|v| v.as_str()) {
                        parsed.stop_reason = Some(stop_reason.to_string());
                        seen_usage = true;
                    }
                }
            }
            _ => {}
        }
    }

    if seen_usage {
        Some(parsed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(cap: &mut UsageCapture, parts: &[&str]) {
        for p in parts {
            cap.feed(p.as_bytes());
        }
    }

    #[test]
    fn parses_non_streaming_json() {
        let body = r#"{"id":"msg_1","type":"message","model":"claude-opus-4-6",
            "stop_reason":"end_turn",
            "usage":{"input_tokens":50,"output_tokens":12,
                     "cache_read_input_tokens":1000,"cache_creation_input_tokens":500}}"#;
        let mut cap = UsageCapture::new(Some("application/json"));
        cap.feed(body.as_bytes());
        let p = cap.finish().unwrap();
        assert_eq!(p.input_tokens, Some(50)); // VERBATIM, no subtraction
        assert_eq!(p.output_tokens, Some(12));
        assert_eq!(p.cache_read_tokens, Some(1000));
        assert_eq!(p.cache_write_tokens, Some(500));
        assert_eq!(p.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(p.stop_reason.as_deref(), Some("end_turn"));
    }

    #[test]
    fn parses_streaming_sse_split_across_chunks() {
        let parts = [
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-sonnet-4-6\",\"usage\":{\"input_tokens\":42,\"cache_read_input_tokens\":900,\"cache_creation_input_tokens\":100,\"output_tokens\":1}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        ];
        let mut cap = UsageCapture::new(Some("text/event-stream; charset=utf-8"));
        feed_all(&mut cap, &parts);
        let p = cap.finish().unwrap();
        assert_eq!(p.input_tokens, Some(42));
        assert_eq!(p.cache_read_tokens, Some(900));
        assert_eq!(p.cache_write_tokens, Some(100));
        assert_eq!(p.output_tokens, Some(7)); // final from message_delta, not the initial 1
        assert_eq!(p.stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(p.model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn returns_none_on_garbage() {
        let mut cap = UsageCapture::new(Some("application/json"));
        cap.feed(b"not json at all");
        assert!(cap.finish().is_none());
    }

    #[test]
    fn partial_sse_stream_returns_what_it_saw() {
        let parts = [
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"model\":\"m\",\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}\n\n",
        ];
        let mut cap = UsageCapture::new(Some("text/event-stream"));
        feed_all(&mut cap, &parts);
        let p = cap.finish().unwrap();
        assert_eq!(p.input_tokens, Some(10));
        assert_eq!(p.stop_reason, None);
    }
}
