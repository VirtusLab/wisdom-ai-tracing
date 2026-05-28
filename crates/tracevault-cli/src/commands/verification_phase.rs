use std::fs;
use std::path::Path;

use tracevault_core::streaming::{StreamEventRequest, StreamEventType};

use crate::api_client::ApiClient;
use crate::commands::stream::next_event_index;
use crate::config::TracevaultConfig;
use crate::credentials::Credentials;

/// Send a VerificationPhaseStart event to the server, recording the current
/// timestamp as the start of the verification phase for this session.
///
/// Only the most recent call per session matters — calling this again simply
/// moves the phase cursor forward, discarding events from the previous phase.
///
/// `explicit_session_id` — when Some, targets that session directly.
/// When None, the most recently modified session directory is used (suitable
/// for single-agent setups; pass `--session-id` in multi-agent setups).
pub async fn open_verification_phase(
    project_root: &Path,
    explicit_session_id: Option<&str>,
) -> Result<(), String> {
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

    let sessions_dir = project_root.join(".tracevault").join("sessions");

    let session_id = if let Some(id) = explicit_session_id {
        // Verify the session directory exists when an explicit ID is given.
        let dir = sessions_dir.join(id);
        if !dir.is_dir() {
            return Err(format!(
                "Session directory not found: {}. Check the session ID.",
                dir.display()
            ));
        }
        id.to_string()
    } else {
        // Auto-detect: use the most recently modified session directory.
        let session_dir = find_latest_session(&sessions_dir).ok_or(
            "No active session found. Start a session by running an AI coding agent first, \
             or pass --session-id to target a specific session.",
        )?;
        session_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Could not determine session ID")?
            .to_string()
    };

    let session_dir = sessions_dir.join(&session_id);
    let counter_path = session_dir.join(".event_counter");
    let event_index = next_event_index(&counter_path)
        .map_err(|e| format!("Failed to read event counter: {e}"))?;

    let event = StreamEventRequest {
        protocol_version: 2,
        // Carry the tool like the hook stream path does — the server's
        // `sessions.tool` column is NOT NULL, so sending None makes the
        // session upsert fail. (The server also defends against this, but
        // there's no reason to send a null here.)
        tool: Some("claude-code".to_string()),
        event_type: StreamEventType::VerificationPhaseStart,
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
        .map_err(|e| format!("Failed to send verification phase event: {e}"))?;

    println!("✓ Verification phase opened for session {session_id}");
    println!("  Tool calls from this point are evaluated by verification_phase-scoped policies.");
    println!("  Run `tracevault verify-start` again to reset the phase if needed.");

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
