use crate::api_client::{resolve_credentials, ApiClient, CheckPoliciesRequest, SessionCheckData};
use crate::config::TracevaultConfig;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git_repo_name(project_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .as_deref()
        .and_then(|p| p.rsplit('/').next())
        .map(String::from)
        .unwrap_or_else(|| "unknown".into())
}

fn git_head_sha(project_root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

fn collect_session_data(session_dir: &Path) -> Option<SessionCheckData> {
    let session_id = session_dir.file_name()?.to_string_lossy().to_string();

    // Read events.jsonl for files_modified
    let events_path = session_dir.join("events.jsonl");
    let mut files_modified = Vec::new();
    let mut files_seen = HashSet::new();

    if events_path.exists() {
        if let Ok(content) = fs::read_to_string(&events_path) {
            for line in content.lines() {
                let event: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if let Some(path) = event
                    .get("tool_input")
                    .and_then(|v| v.get("file_path"))
                    .and_then(|v| v.as_str())
                {
                    if files_seen.insert(path.to_string()) {
                        files_modified.push(path.to_string());
                    }
                }
            }
        }
    }

    // Read transcript for tool_calls
    let meta_path = session_dir.join("metadata.json");
    let metadata: Option<serde_json::Value> = meta_path
        .exists()
        .then(|| fs::read_to_string(&meta_path).ok())
        .flatten()
        .and_then(|c| serde_json::from_str(&c).ok());

    let transcript_path = metadata
        .as_ref()
        .and_then(|m| m.get("transcript_path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mut tool_calls_map: std::collections::HashMap<String, i32> =
        std::collections::HashMap::new();
    let mut total_tool_calls: i32 = 0;

    if let Some(path) = &transcript_path {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let entry: serde_json::Value = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if entry.get("type").and_then(|v| v.as_str()) == Some("assistant") {
                    if let Some(content_arr) = entry
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                    {
                        for block in content_arr {
                            if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                                if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                                    *tool_calls_map.entry(name.to_string()).or_insert(0) += 1;
                                    total_tool_calls += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let tool_calls = if tool_calls_map.is_empty() {
        None
    } else {
        serde_json::to_value(&tool_calls_map).ok()
    };

    Some(SessionCheckData {
        session_id,
        tool_calls,
        files_modified: if files_modified.is_empty() {
            None
        } else {
            Some(files_modified)
        },
        total_tool_calls: if total_tool_calls > 0 {
            Some(total_tool_calls)
        } else {
            None
        },
    })
}

pub async fn check_policies(project_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (server_url, token) = resolve_credentials(project_root);

    let server_url = match server_url {
        Some(url) => url,
        None => {
            return Err("No server URL configured. Run 'tracevault login' first.".into());
        }
    };

    if token.is_none() {
        return Err("Not logged in. Run 'tracevault login' to check policies.".into());
    }

    let org_slug = TracevaultConfig::load(project_root)
        .and_then(|c| c.org_slug)
        .ok_or("No org_slug in config. Run 'tracevault init' first.")?;

    let client = ApiClient::new(&server_url, token.as_deref());

    // Resolve repo_id by name
    let repo_name = git_repo_name(project_root);
    let repos = client.list_repos(&org_slug).await?;
    let repo = repos.iter().find(|r| r.name == repo_name).ok_or_else(|| {
        format!(
            "Repo '{}' not found on server. Run 'tracevault sync' first.",
            repo_name
        )
    })?;

    // Collect session data from unpushed sessions
    let sessions_dir = project_root.join(".tracevault").join("sessions");
    let mut sessions = Vec::new();

    if sessions_dir.exists() {
        for entry in fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let session_dir = entry.path();
            let pushed_marker = session_dir.join(".pushed");
            if pushed_marker.exists() {
                continue;
            }
            if let Some(data) = collect_session_data(&session_dir) {
                sessions.push(data);
            }
        }
    }

    if sessions.is_empty() {
        println!("No unpushed sessions to check.");
        return Ok(());
    }

    println!("Checking {} session(s) against policies...", sessions.len());

    let commit_sha = git_head_sha(project_root);
    let result = client
        .check_policies(
            &org_slug,
            &repo.id,
            CheckPoliciesRequest {
                sessions,
                commit_sha,
            },
        )
        .await?;

    // Print results
    for r in &result.results {
        let icon = match r.result.as_str() {
            "pass" => "\x1b[32m✓\x1b[0m",                             // green
            "fail" if r.action == "block_push" => "\x1b[31m✗\x1b[0m", // red
            "fail" => "\x1b[33m!\x1b[0m",                             // yellow
            _ => " ",
        };
        println!(
            "  {} [{}] {} — {}",
            icon, r.severity, r.rule_name, r.details
        );
    }

    if result.blocked {
        eprintln!("\n\x1b[31mPolicy check failed: push blocked.\x1b[0m");
        std::process::exit(1);
    } else if result.passed {
        println!("\n\x1b[32mAll policy checks passed.\x1b[0m");
    } else {
        println!("\n\x1b[33mPolicy warnings found (push not blocked).\x1b[0m");
    }

    Ok(())
}
