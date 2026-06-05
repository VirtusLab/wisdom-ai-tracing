use serde_json::Value;

/// Upper bound on bytes retained for capture (request/response bodies are
/// queued for later projection). Beyond this the body is marked `truncated`
/// and further bytes are dropped — usage parsing for SSE is incremental and
/// does NOT depend on retaining the whole body, so token accounting stays
/// correct even when the captured body is truncated.
const CAPTURE_CAP_BYTES: usize = 5 * 1024 * 1024; // 5 MB

/// Defensive bound on a single in-progress SSE line. Real Anthropic SSE lines
/// are small JSON objects; a stream that never emits a newline must not grow
/// the line assembler without bound.
const SSE_LINE_CAP: usize = 1024 * 1024; // 1 MB

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedUsage {
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub stop_reason: Option<String>,
}

/// The (bounded) raw response body retained for capture, plus whether it was
/// truncated at `CAPTURE_CAP_BYTES`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CapturedBody {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BodyKind {
    Sse,
    Json,
}

/// Taps a proxied response stream for two independent purposes:
///   1. **Usage accounting** — for SSE this is parsed *incrementally* as bytes
///      arrive (O(1) memory: a small line assembler + a running `ParsedUsage`),
///      so the final `message_delta` usage is captured regardless of stream
///      size. For JSON it is parsed once from the (bounded) captured body.
///   2. **Capture** — a bounded copy of the body (`CAPTURE_CAP_BYTES`) retained
///      for later projection into the capture model.
pub struct UsageCapture {
    kind: BodyKind,
    cap: usize,
    // Bounded capture buffer. For JSON this is also the usage-parse source.
    capture_buf: Vec<u8>,
    truncated: bool,
    // Incremental SSE state.
    line_buf: Vec<u8>,
    sse_parsed: ParsedUsage,
    sse_seen: bool,
}

impl UsageCapture {
    pub fn new(content_type: Option<&str>) -> Self {
        Self::with_cap(content_type, CAPTURE_CAP_BYTES)
    }

    fn with_cap(content_type: Option<&str>, cap: usize) -> Self {
        let kind = if content_type.unwrap_or("").contains("text/event-stream") {
            BodyKind::Sse
        } else {
            BodyKind::Json
        };
        UsageCapture {
            kind,
            cap,
            capture_buf: Vec::new(),
            truncated: false,
            line_buf: Vec::new(),
            sse_parsed: ParsedUsage::default(),
            sse_seen: false,
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) {
        // 1. Bounded capture: append up to the cap, then mark truncated.
        if self.capture_buf.len() < self.cap {
            let remaining = self.cap - self.capture_buf.len();
            if chunk.len() <= remaining {
                self.capture_buf.extend_from_slice(chunk);
            } else {
                self.capture_buf.extend_from_slice(&chunk[..remaining]);
                self.truncated = true;
            }
        } else if !chunk.is_empty() {
            self.truncated = true;
        }

        // 2. Incremental SSE usage parsing — independent of the capture cap.
        if self.kind == BodyKind::Sse {
            self.feed_sse(chunk);
        }
    }

    fn feed_sse(&mut self, chunk: &[u8]) {
        self.line_buf.extend_from_slice(chunk);
        while let Some(pos) = self.line_buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.line_buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line).into_owned();
            self.parse_sse_line(&line);
        }
        // Discard a pathologically large partial line (can't be valid SSE).
        if self.line_buf.len() > SSE_LINE_CAP {
            self.line_buf.clear();
        }
    }

    fn parse_sse_line(&mut self, line: &str) {
        let data = match line.trim_start().strip_prefix("data:") {
            Some(rest) => rest.trim(),
            None => return,
        };
        let v: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return,
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("message_start") => self.apply_message_start(&v),
            Some("message_delta") => self.apply_message_delta(&v),
            _ => {}
        }
    }

    fn apply_message_start(&mut self, v: &Value) {
        let Some(message) = v.get("message") else {
            return;
        };
        // Keep the first model name seen; a resumed stream might repeat message_start.
        if self.sse_parsed.model.is_none() {
            if let Some(model) = message.get("model").and_then(|m| m.as_str()) {
                self.sse_parsed.model = Some(model.to_string());
                self.sse_seen = true;
            }
        }
        if let Some(usage) = message.get("usage") {
            apply_usage(&mut self.sse_parsed, usage);
            self.sse_seen = true;
        }
    }

    fn apply_message_delta(&mut self, v: &Value) {
        // message_delta usage carries the final cumulative output_tokens.
        if let Some(output) = v
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_i64())
        {
            self.sse_parsed.output_tokens = Some(output);
            self.sse_seen = true;
        }
        if let Some(stop_reason) = v
            .get("delta")
            .and_then(|d| d.get("stop_reason"))
            .and_then(|v| v.as_str())
        {
            self.sse_parsed.stop_reason = Some(stop_reason.to_string());
            self.sse_seen = true;
        }
    }

    /// Consume the capture, returning parsed usage AND the bounded captured body.
    pub fn into_parts(mut self) -> (Option<ParsedUsage>, CapturedBody) {
        // Flush a trailing SSE line that never received a newline.
        if self.kind == BodyKind::Sse && !self.line_buf.is_empty() {
            let line = String::from_utf8_lossy(&self.line_buf).into_owned();
            self.parse_sse_line(&line);
            self.line_buf.clear();
        }

        let parsed = match self.kind {
            BodyKind::Sse => self.sse_seen.then(|| std::mem::take(&mut self.sse_parsed)),
            BodyKind::Json => parse_json(&self.capture_buf),
        };
        let captured = CapturedBody {
            bytes: std::mem::take(&mut self.capture_buf),
            truncated: self.truncated,
        };
        (parsed, captured)
    }

    /// Usage-only convenience for callers that don't need the captured body.
    pub fn finish(self) -> Option<ParsedUsage> {
        self.into_parts().0
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
    let root_json: Value = serde_json::from_slice(buf).ok()?;
    let mut parsed = ParsedUsage::default();
    let mut seen = false;

    if let Some(model) = root_json.get("model").and_then(|v| v.as_str()) {
        parsed.model = Some(model.to_string());
        seen = true;
    }
    if let Some(stop_reason) = root_json.get("stop_reason").and_then(|v| v.as_str()) {
        parsed.stop_reason = Some(stop_reason.to_string());
        seen = true;
    }
    if let Some(usage) = root_json.get("usage") {
        apply_usage(&mut parsed, usage);
        seen = true;
    }

    seen.then_some(parsed)
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
    fn parses_sse_when_data_line_split_mid_chunk() {
        // A single data line delivered in two byte-level pieces (no newline in
        // the first) must still parse once the newline arrives.
        let mut cap = UsageCapture::new(Some("text/event-stream"));
        cap.feed(b"data: {\"type\":\"message_start\",\"message\":{\"model\":\"m\",");
        cap.feed(b"\"usage\":{\"input_tokens\":11,\"output_tokens\":1}}}\n\n");
        let p = cap.finish().unwrap();
        assert_eq!(p.input_tokens, Some(11));
        assert_eq!(p.model.as_deref(), Some("m"));
    }

    #[test]
    fn returns_none_on_garbage() {
        let mut cap = UsageCapture::new(Some("application/json"));
        cap.feed(b"not json at all");
        assert!(cap.finish().is_none());
    }

    #[test]
    fn defaults_to_json_when_no_content_type() {
        let mut cap = UsageCapture::new(None);
        cap.feed(br#"{"model":"m","usage":{"input_tokens":1,"output_tokens":2}}"#);
        assert!(cap.finish().is_some());
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

    #[test]
    fn into_parts_returns_usage_and_untruncated_body() {
        let body = br#"{"id":"msg_1","type":"message","model":"claude","stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":2}}"#;
        let mut cap = UsageCapture::new(Some("application/json"));
        cap.feed(body);
        let (parsed, captured) = cap.into_parts();
        assert_eq!(captured.bytes, body.to_vec(), "raw body returned verbatim");
        assert!(!captured.truncated);
        let parsed = parsed.expect("usage should parse");
        assert_eq!(parsed.input_tokens, Some(5));
        assert_eq!(parsed.output_tokens, Some(2));
    }

    #[test]
    fn capture_body_is_bounded_and_marked_truncated() {
        let mut cap = UsageCapture::with_cap(Some("application/json"), 8);
        cap.feed(b"0123456789ABCDEF"); // 16 bytes, cap is 8
        let (_, captured) = cap.into_parts();
        assert_eq!(captured.bytes.len(), 8);
        assert_eq!(&captured.bytes, b"01234567");
        assert!(captured.truncated);
    }

    #[test]
    fn sse_usage_parsed_even_when_capture_truncated() {
        // Tiny cap so the captured body truncates, but incremental SSE parsing
        // must still recover input (from message_start) and final output (from
        // message_delta) — they bracket the truncated middle.
        let mut cap = UsageCapture::with_cap(Some("text/event-stream"), 16);
        cap.feed(b"data: {\"type\":\"message_start\",\"message\":{\"model\":\"m\",\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}\n\n");
        cap.feed(b"data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n\n");
        let (parsed, captured) = cap.into_parts();
        assert!(captured.truncated, "body should be truncated at the tiny cap");
        let parsed = parsed.expect("usage parsed incrementally despite truncation");
        assert_eq!(parsed.input_tokens, Some(10));
        assert_eq!(parsed.output_tokens, Some(7));
        assert_eq!(parsed.stop_reason.as_deref(), Some("end_turn"));
    }
}
