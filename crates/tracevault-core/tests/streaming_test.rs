use serde_json::json;
use tracevault_core::streaming::*;

#[test]
fn test_stream_event_request_serialization() {
    let eid = uuid::Uuid::now_v7();
    let req = StreamEventRequest {
        protocol_version: 1,
        tool: None,
        event_type: StreamEventType::ToolUse,
        session_id: "sess-123".to_string(),
        timestamp: chrono::Utc::now(),
        hook_event_name: Some("PostToolUse".to_string()),
        tool_name: Some("Edit".to_string()),
        tool_use_id: None,
        tool_input: Some(
            json!({"file_path": "src/main.rs", "old_string": "old", "new_string": "new"}),
        ),
        tool_response: Some(json!({"success": true})),
        tool_is_error: None,
        event_index: Some(42),
        event_uuid: Some(eid),
        transcript_lines: None,
        transcript_offset: None,
        model: None,
        cwd: None,
        final_stats: None,
    };
    let json_str = serde_json::to_string(&req).unwrap();
    let parsed: StreamEventRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.session_id, "sess-123");
    assert_eq!(parsed.event_index, Some(42));
    assert_eq!(parsed.event_uuid, Some(eid));
}

#[test]
fn test_event_uuid_defaults_to_none_for_legacy_payloads() {
    // A payload from an older client omits event_uuid entirely; it must
    // deserialize as None rather than failing.
    let legacy = json!({
        "protocol_version": 1,
        "event_type": "tool_use",
        "session_id": "sess-legacy",
        "timestamp": chrono::Utc::now(),
        "hook_event_name": "PostToolUse",
        "tool_name": "Edit",
        "tool_use_id": "toolu_legacy",
        "tool_input": null,
        "tool_response": null,
        "tool_is_error": null,
        "event_index": 7,
        "transcript_lines": null,
        "transcript_offset": null,
        "model": null,
        "cwd": null,
        "final_stats": null
    });
    let parsed: StreamEventRequest = serde_json::from_value(legacy).unwrap();
    assert_eq!(parsed.event_uuid, None);
    assert_eq!(parsed.event_index, Some(7));
}

#[test]
fn test_commit_push_request_serialization() {
    let req = CommitPushRequest {
        commit_sha: "abc123".to_string(),
        branch: Some("main".to_string()),
        author: "dev@example.com".to_string(),
        message: Some("feat: add new feature".to_string()),
        diff_data: Some(json!({"files": []})),
        committed_at: Some(chrono::Utc::now()),
    };
    let json_str = serde_json::to_string(&req).unwrap();
    let parsed: CommitPushRequest = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.commit_sha, "abc123");
}
