use std::fs;
use std::path::Path;

use tracevault_core::streaming::{StreamEventRequest, StreamEventType};

use crate::api_client::ApiClient;
use crate::commands::stream::next_event_index;
use crate::config::TracevaultConfig;
use crate::credentials::Credentials;

/// Send a ValidationWindowStart event to the server, recording the current
/// timestamp as the start of the validation window for this session.
///
/// Only the most recent call per session matters — calling this again simply
/// moves the window cursor forward, discarding events from the previous window.
pub async fn open_validation_window(project_root: &Path) -> Result<(), String> {
    let config = TracevaultConfig::load(project_root)
        .ok_or("TraceVault not initialized. Run `tracevault init` first.")?;

    let org_slug = config
        .org_slug
        .as_deref()
        .ok_or("No org_slug configured. Run `tracevault init`.")?;
    let repo_id = config
        .repo_id
        .as_deref()
        .ok_or("No repo_id configured. Run `tracevault init`.")?;

    // Find the most recently modified session directory
    let sessions_dir = project_root.join(".tracevault").join("sessions");
    let session_dir = find_latest_session(&sessions_dir)
        .ok_or("No active session found. Start a session by running an AI coding agent first.")?;

    let session_id = session_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Could not determine session ID")?
        .to_string();

    let counter_path = session_dir.join(".event_counter");
    let event_index = next_event_index(&counter_path)
        .map_err(|e| format!("Failed to read event counter: {e}"))?;

    let event = StreamEventRequest {
        protocol_version: 2,
        tool: None,
        event_type: StreamEventType::ValidationWindowStart,
        session_id: session_id.clone(),
        timestamp: chrono::Utc::now(),
        hook_event_name: None,
        tool_name: None,
        tool_use_id: None,
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        event_index: Some(event_index),
        transcript_lines: None,
        transcript_offset: None,
        model: None,
        cwd: Some(project_root.to_string_lossy().into_owned()),
        final_stats: None,
    };

    let creds = Credentials::load().ok_or("Not logged in. Run `tracevault login` first.")?;
    let server_url = config
        .server_url
        .as_deref()
        .unwrap_or("https://tracevault.softwaremill.com");
    let client = ApiClient::new(server_url, Some(&creds.token));

    client
        .stream_event(org_slug, repo_id, &event)
        .await
        .map_err(|e| format!("Failed to send validation window event: {e}"))?;

    println!("✓ Validation window opened for session {session_id}");
    println!("  Tool calls from this point are evaluated by validation_window-scoped policies.");
    println!("  Run `tracevault validation-start` again to reset the window if needed.");

    Ok(())
}

/// Return the most recently modified session directory under `sessions_dir`.
fn find_latest_session(sessions_dir: &Path) -> Option<std::path::PathBuf> {
    let entries = fs::read_dir(sessions_dir).ok()?;
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .max_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
        .map(|e| e.path())
}
