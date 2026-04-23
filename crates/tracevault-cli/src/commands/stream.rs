use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};
use std::path::Path;

use tracevault_core::hooks::{parse_hook_event, HookResponse};
use tracevault_core::streaming::{StreamEventRequest, StreamEventType};

pub fn next_event_index(counter_path: &Path) -> Result<i32, io::Error> {
    let current = if counter_path.exists() {
        let content = fs::read_to_string(counter_path)?;
        content
            .trim()
            .parse::<i32>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        0
    };
    fs::write(counter_path, (current + 1).to_string())?;
    Ok(current)
}

pub fn read_new_transcript_lines(
    transcript_path: &Path,
    offset_path: &Path,
) -> Result<(Vec<serde_json::Value>, i64), io::Error> {
    if !transcript_path.exists() {
        return Ok((vec![], 0));
    }

    let offset: i64 = if offset_path.exists() {
        let content = fs::read_to_string(offset_path)?;
        content
            .trim()
            .parse::<i64>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
    } else {
        0
    };

    let mut file = fs::File::open(transcript_path)?;
    file.seek(SeekFrom::Start(offset as u64))?;

    let reader = io::BufReader::new(file);
    let mut lines = Vec::new();
    let mut bytes_read = offset;

    for line_result in reader.lines() {
        let line = line_result?;
        // +1 for the newline character
        bytes_read += line.len() as i64 + 1;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) {
            lines.push(value);
        }
    }

    Ok((lines, bytes_read))
}

pub fn append_pending(pending_path: &Path, json: &str) -> Result<(), io::Error> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(pending_path)?;
    writeln!(file, "{json}")?;
    Ok(())
}

pub fn drain_pending(pending_path: &Path) -> Result<Vec<String>, io::Error> {
    if !pending_path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(pending_path)?;
    let lines: Vec<String> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect();
    fs::remove_file(pending_path)?;
    Ok(lines)
}

pub async fn run_stream(
    project_root: &Path,
    event_type: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Read HookEvent from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let hook_event = parse_hook_event(&input)?;

    // 2. Create session dir
    let session_dir = project_root
        .join(".tracevault")
        .join("sessions")
        .join(&hook_event.session_id);
    fs::create_dir_all(&session_dir)?;

    // 3. Get event_index
    let counter_path = session_dir.join(".event_counter");
    let event_index = next_event_index(&counter_path)?;

    // 4. Read new transcript lines
    let transcript_path = Path::new(&hook_event.transcript_path);
    let offset_path = session_dir.join(".stream_offset");
    let (transcript_lines, new_offset) = read_new_transcript_lines(transcript_path, &offset_path)?;

    // 5. Build StreamEventRequest
    let stream_event_type = match event_type {
        "notification" => StreamEventType::SessionStart,
        "stop" => StreamEventType::SessionEnd,
        _ => StreamEventType::ToolUse,
    };

    let mut req = StreamEventRequest {
        protocol_version: 1,
        tool: Some("claude-code".to_string()),
        event_type: stream_event_type,
        session_id: hook_event.session_id.clone(),
        timestamp: chrono::Utc::now(),
        hook_event_name: Some(hook_event.hook_event_name.clone()),
        tool_name: hook_event.tool_name.clone(),
        tool_input: hook_event.tool_input.clone(),
        tool_response: hook_event.tool_response.clone(),
        event_index: Some(event_index),
        transcript_lines: if transcript_lines.is_empty() {
            None
        } else {
            Some(transcript_lines)
        },
        transcript_offset: Some(new_offset),
        model: None,
        cwd: Some(hook_event.cwd.clone()),
        final_stats: None,
    };

    req.truncate_large_fields();

    // 6. Resolve credentials
    let (server_url, token) = crate::api_client::resolve_credentials(project_root);

    // 7. Load config for org_slug and repo_id
    let config =
        crate::config::TracevaultConfig::load(project_root).ok_or("TracevaultConfig not found")?;
    let org_slug = config
        .org_slug
        .as_deref()
        .ok_or("org_slug not configured")?;
    let repo_id = config.repo_id.as_deref().ok_or("repo_id not configured")?;

    // 8. Create ApiClient
    let server_url = server_url.ok_or("server_url not configured")?;
    let client = crate::api_client::ApiClient::new(&server_url, token.as_deref());

    // 9. Try drain pending queue and send
    let pending_path = session_dir.join("pending.jsonl");
    let pending_events = drain_pending(&pending_path)?;

    let mut send_failed = false;

    // Send pending events first
    for pending_json in &pending_events {
        if let Ok(pending_req) = serde_json::from_str::<StreamEventRequest>(pending_json) {
            if client
                .stream_event(org_slug, repo_id, &pending_req)
                .await
                .is_err()
            {
                // Re-queue all remaining pending events
                for evt in &pending_events {
                    append_pending(&pending_path, evt)?;
                }
                send_failed = true;
                break;
            }
        }
    }

    // Send current event
    let req_json = serde_json::to_string(&req)?;
    if send_failed {
        append_pending(&pending_path, &req_json)?;
    } else {
        match client.stream_event(org_slug, repo_id, &req).await {
            Ok(_) => {
                // 10. On success update .stream_offset
                fs::write(&offset_path, new_offset.to_string())?;
            }
            Err(_) => {
                // 11. On failure append to pending.jsonl
                append_pending(&pending_path, &req_json)?;
            }
        }
    }

    // 12. Always print HookResponse::allow() to stdout
    let response = HookResponse::allow();
    println!("{}", serde_json::to_string(&response)?);

    Ok(())
}
