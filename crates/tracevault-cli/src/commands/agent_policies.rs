//! `tracevault agent-policies` — fetch active policies and render
//! agent-readable Markdown instructions.
//!
//! Output is consumed by an agent (Claude Code, Pi, etc.) at session start
//! so its behaviour matches the policies configured on the TraceVault server.

use crate::api_client::{resolve_credentials, ApiClient, PolicyListItem, RepoSettings};
use crate::config::TracevaultConfig;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// Subset of `tracevault_core::policy::PolicyCondition` — only the variants
/// that render into agent instructions. Other variants (TokenBudget,
/// ModelAllowlist, etc.) are server-evaluated without agent action and are
/// intentionally skipped.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum Condition {
    RequiredToolCall {
        tool_names: Vec<String>,
        #[serde(default)]
        must_succeed: bool,
    },
    ConditionalToolCall {
        tool_name: String,
        #[serde(default)]
        when_files_match: Option<Vec<String>>,
        #[serde(default)]
        must_succeed: bool,
    },
    /// Variant we don't render — catch-all for unknown / non-tool conditions.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Block,
    Warn,
    Allow,
}

impl Action {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "block_push" => Some(Action::Block),
            "warn" => Some(Action::Warn),
            "allow" => Some(Action::Allow),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Session,
    ValidationWindow,
    Both,
}

impl Scope {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "session" => Some(Scope::Session),
            "validation_window" => Some(Scope::ValidationWindow),
            "both" => Some(Scope::Both),
            _ => None,
        }
    }

    fn applies_to_session(self) -> bool {
        matches!(self, Scope::Session | Scope::Both)
    }

    fn applies_to_window(self) -> bool {
        matches!(self, Scope::ValidationWindow | Scope::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowMode {
    Disabled,
    Warn,
    Block,
}

impl WindowMode {
    fn from_str(s: &str) -> Self {
        match s {
            "warn" => WindowMode::Warn,
            "block" => WindowMode::Block,
            _ => WindowMode::Disabled,
        }
    }
}

/// One rendered line: a tool requirement under a given scope.
#[derive(Debug, Clone)]
struct ToolRequirement {
    tool: String,
    action: Action,
    must_succeed: bool,
    when_files_match: Option<Vec<String>>,
}

impl ToolRequirement {
    fn render_line(&self) -> String {
        // Allow-listed tools render bare — no "must/should be called" language.
        if self.action == Action::Allow {
            return format!("- `{}`", self.tool);
        }

        let (verb, succeed) = match self.action {
            Action::Block => ("must be called", "must succeed"),
            Action::Warn => ("should be called", "should succeed"),
            Action::Allow => unreachable!(),
        };

        let action_phrase = if self.must_succeed {
            format!("{verb} and {succeed}")
        } else {
            // For unconditional required tools we add "at least once" so it's
            // clear duplicates aren't required.
            if self.when_files_match.is_some() {
                verb.to_string()
            } else {
                format!("{verb} at least once")
            }
        };

        match &self.when_files_match {
            Some(patterns) if !patterns.is_empty() => {
                let joined = patterns
                    .iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "- `{}` — when files matching {joined} are changed, {action_phrase}",
                    self.tool
                )
            }
            _ => format!("- `{}` — {action_phrase}", self.tool),
        }
    }
}

/// Pure-function renderer. Takes the raw API data and produces the Markdown
/// output. Lives apart from the network layer so it can be unit-tested.
fn render_instructions(policies: &[PolicyListItem], settings: &RepoSettings) -> String {
    let window_mode = WindowMode::from_str(&settings.validation_window_mode);

    // Buckets per section.
    let mut session_reqs: Vec<ToolRequirement> = Vec::new();
    let mut window_required: Vec<ToolRequirement> = Vec::new();
    let mut window_allowed: Vec<ToolRequirement> = Vec::new();

    for p in policies.iter().filter(|p| p.enabled) {
        let Some(action) = Action::from_str(&p.action) else {
            continue;
        };
        let Some(scope) = Scope::from_str(&p.scope) else {
            continue;
        };

        let cond: Condition = match serde_json::from_value(p.condition.clone()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let reqs = condition_to_requirements(&cond, action);
        if reqs.is_empty() {
            continue;
        }

        if scope.applies_to_session() {
            session_reqs.extend(reqs.iter().cloned());
        }
        if scope.applies_to_window() && window_mode != WindowMode::Disabled {
            for r in &reqs {
                if r.action == Action::Allow {
                    window_allowed.push(r.clone());
                } else {
                    window_required.push(r.clone());
                }
            }
        }
    }

    // No actionable policies at all → terse message.
    let has_session_section = !session_reqs.is_empty();
    let has_window_section = (!window_required.is_empty() || !window_allowed.is_empty())
        && window_mode != WindowMode::Disabled;

    if !has_session_section && !has_window_section {
        return "## Visdom Trace — agent policy instructions\n\n\
                No active policies for this repository.\n"
            .into();
    }

    let mut out = String::new();
    out.push_str("## Visdom Trace — agent policy instructions\n\n");
    out.push_str(
        "These instructions reflect the active policies for this repository. \
        They take precedence over any manual instructions elsewhere.\n",
    );

    if has_session_section {
        out.push_str("\n### Before push\n");
        out.push_str("The following pre-push checks apply to this repository:\n");
        for r in &session_reqs {
            out.push_str(&r.render_line());
            out.push('\n');
        }
    }

    if has_window_section {
        out.push_str("\n### Validation window (pre-push gating)\n");
        out.push_str(
            "A validation window restricts which tools can be called before push, \
            gating the push on a clean validation run. Before pushing, open a \
            validation window:\n\n    tracevault validation-start\n\n\
            The window stays open until you push, or until you open a new window. \
            Opening a new window invalidates the prior one.\n",
        );

        if !window_required.is_empty() {
            out.push_str("\nRequired tools (must be called inside the window):\n");
            for r in &window_required {
                out.push_str(&r.render_line());
                out.push('\n');
            }
        }

        if !window_allowed.is_empty() {
            out.push_str("\nAllowed tools (may be called freely inside the window):\n");
            for r in &window_allowed {
                out.push_str(&r.render_line());
                out.push('\n');
            }
        }

        out.push_str(
            "\nIf you need to call additional tools after opening the window, open a new window \
            afterwards and rerun all required tools.\n",
        );
    }

    out
}

fn condition_to_requirements(cond: &Condition, action: Action) -> Vec<ToolRequirement> {
    match cond {
        Condition::RequiredToolCall {
            tool_names,
            must_succeed,
        } => tool_names
            .iter()
            .map(|t| ToolRequirement {
                tool: t.clone(),
                action,
                must_succeed: *must_succeed,
                when_files_match: None,
            })
            .collect(),
        Condition::ConditionalToolCall {
            tool_name,
            when_files_match,
            must_succeed,
        } => vec![ToolRequirement {
            tool: tool_name.clone(),
            action,
            must_succeed: *must_succeed,
            when_files_match: when_files_match.clone(),
        }],
        Condition::Other => Vec::new(),
    }
}

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

pub async fn run(project_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let (server_url, token) = resolve_credentials(project_root);

    let server_url = server_url.ok_or("No server URL configured. Run 'tracevault login' first.")?;
    let token = token.ok_or("Not logged in. Run 'tracevault login' first.")?;

    let org_slug = TracevaultConfig::load(project_root)
        .and_then(|c| c.org_slug)
        .ok_or("No org_slug in .tracevault/config.toml. Run 'tracevault init' first.")?;

    let client = ApiClient::new(&server_url, Some(&token));

    // Resolve repo_id from the local git repo name.
    let repo_name = git_repo_name(project_root);
    let repos = client.list_repos(&org_slug).await?;
    let repo = repos.iter().find(|r| r.name == repo_name).ok_or_else(|| {
        format!("Repo '{repo_name}' not found on server. Run 'tracevault sync' first.")
    })?;

    let (policies, settings) = tokio::try_join!(
        client.list_policies(&org_slug, &repo.id),
        client.get_repo_settings(&org_slug, &repo.id),
    )?;

    print!("{}", render_instructions(&policies, &settings));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(mode: &str) -> RepoSettings {
        RepoSettings {
            validation_window_mode: mode.into(),
        }
    }

    fn policy(
        name: &str,
        condition: serde_json::Value,
        action: &str,
        scope: &str,
        enabled: bool,
    ) -> PolicyListItem {
        PolicyListItem {
            name: name.into(),
            condition,
            action: action.into(),
            scope: scope.into(),
            enabled,
        }
    }

    #[test]
    fn no_policies_renders_terse_message() {
        let out = render_instructions(&[], &settings("disabled"));
        assert!(out.contains("No active policies"));
        assert!(!out.contains("Before push"));
        assert!(!out.contains("Validation window"));
    }

    #[test]
    fn disabled_policies_are_ignored() {
        let p = policy(
            "fmt",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_fmt"]}),
            "block_push",
            "session",
            false, // disabled
        );
        let out = render_instructions(&[p], &settings("disabled"));
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn session_required_tool_block_renders_must_be_called() {
        let p = policy(
            "fmt",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_fmt"]}),
            "block_push",
            "session",
            true,
        );
        let out = render_instructions(&[p], &settings("disabled"));
        assert!(out.contains("### Before push"));
        assert!(out.contains("`cargo_fmt`"));
        assert!(out.contains("must be called"));
    }

    #[test]
    fn session_required_tool_warn_renders_should_be_called() {
        let p = policy(
            "fmt",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_fmt"]}),
            "warn",
            "session",
            true,
        );
        let out = render_instructions(&[p], &settings("disabled"));
        // The rendered tool line should use "should" language, not "must".
        let tool_line = out
            .lines()
            .find(|l| l.starts_with("- `cargo_fmt`"))
            .expect("tool line missing");
        assert!(tool_line.contains("should be called"));
        assert!(!tool_line.contains("must be called"));
    }

    #[test]
    fn must_succeed_modifies_clause() {
        let p_block = policy(
            "check",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_check"], "must_succeed": true}),
            "block_push",
            "session",
            true,
        );
        let out_block = render_instructions(&[p_block], &settings("disabled"));
        assert!(out_block.contains("must be called and must succeed"));

        let p_warn = policy(
            "check",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_check"], "must_succeed": true}),
            "warn",
            "session",
            true,
        );
        let out_warn = render_instructions(&[p_warn], &settings("disabled"));
        assert!(out_warn.contains("should be called and should succeed"));
    }

    #[test]
    fn conditional_tool_call_renders_file_clause() {
        let p = policy(
            "audit",
            json!({
                "type": "ConditionalToolCall",
                "tool_name": "cargo_audit",
                "when_files_match": ["Cargo.lock"],
                "must_succeed": true
            }),
            "block_push",
            "session",
            true,
        );
        let out = render_instructions(&[p], &settings("disabled"));
        assert!(out.contains("when files matching `Cargo.lock` are changed"));
        assert!(out.contains("must be called and must succeed"));
    }

    #[test]
    fn validation_window_required_section_renders() {
        let p = policy(
            "review",
            json!({
                "type": "RequiredToolCall",
                "tool_names": ["agent_review"],
                "must_succeed": true
            }),
            "block_push",
            "validation_window",
            true,
        );
        let out = render_instructions(&[p], &settings("block"));
        assert!(out.contains("### Validation window"));
        assert!(out.contains("Required tools"));
        assert!(out.contains("`agent_review`"));
    }

    #[test]
    fn validation_window_allowed_section_renders() {
        let p = policy(
            "read-ok",
            json!({"type": "RequiredToolCall", "tool_names": ["Read", "Grep"]}),
            "allow",
            "validation_window",
            true,
        );
        let out = render_instructions(&[p], &settings("block"));
        assert!(out.contains("Allowed tools"));
        assert!(out.contains("`Read`"));
        assert!(out.contains("`Grep`"));
        assert!(!out.contains("must be called"));
    }

    #[test]
    fn validation_window_section_hidden_when_mode_disabled() {
        let p = policy(
            "review",
            json!({"type": "RequiredToolCall", "tool_names": ["agent_review"]}),
            "block_push",
            "validation_window",
            true,
        );
        let out = render_instructions(&[p], &settings("disabled"));
        assert!(!out.contains("### Validation window"));
        // No session section either since only validation_window-scoped policy → terse output.
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn unknown_condition_type_is_skipped_silently() {
        let p = policy(
            "budget",
            json!({"type": "TokenBudget", "max_tokens": 1000}),
            "block_push",
            "session",
            true,
        );
        let out = render_instructions(&[p], &settings("disabled"));
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn scope_both_applies_to_both_sections() {
        let p = policy(
            "fmt",
            json!({"type": "RequiredToolCall", "tool_names": ["cargo_fmt"]}),
            "block_push",
            "both",
            true,
        );
        let out = render_instructions(&[p], &settings("block"));
        assert!(out.contains("### Before push"));
        assert!(out.contains("### Validation window"));
        // Tool listed in both sections.
        let count = out.matches("`cargo_fmt`").count();
        assert_eq!(count, 2, "expected cargo_fmt in both sections, got: {out}");
    }

    #[test]
    fn warn_mode_does_not_advertise_blocking_in_window() {
        // The spec says: do NOT emit any "this will block the push" statement
        // (we never emit one anyway). Just verify the window section renders
        // without making false claims.
        let p = policy(
            "review",
            json!({"type": "RequiredToolCall", "tool_names": ["agent_review"]}),
            "warn",
            "validation_window",
            true,
        );
        let out = render_instructions(&[p], &settings("warn"));
        assert!(out.contains("### Validation window"));
        assert!(!out.contains("will block the push"));
        assert!(!out.contains("blocked"));
        // Should use 'should be called' (warn action).
        assert!(out.contains("should be called"));
    }
}
