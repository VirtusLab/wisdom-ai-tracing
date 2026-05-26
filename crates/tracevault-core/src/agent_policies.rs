//! Render agent-readable Markdown instructions from a set of policies.
//!
//! Consumed by `tracevault agent-policies`, the `agent_policies` MCP tool, and
//! the dashboard preview. Lives in `tracevault-core` so there is exactly one
//! rendering implementation regardless of which frontend asks for it.

use crate::policy::{PolicyAction, PolicyCondition, PolicyRule, PolicyScope, ValidationWindowMode};

/// One rendered tool-requirement line.
#[derive(Debug, Clone)]
struct ToolRequirement {
    tool: String,
    action: ActionTag,
    must_succeed: bool,
    when_files_match: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionTag {
    Block,
    Warn,
    Allow,
}

impl ActionTag {
    fn from(action: &PolicyAction) -> Self {
        match action {
            PolicyAction::BlockPush => ActionTag::Block,
            PolicyAction::Warn => ActionTag::Warn,
            PolicyAction::Allow => ActionTag::Allow,
        }
    }
}

impl ToolRequirement {
    fn render_line(&self) -> String {
        if self.action == ActionTag::Allow {
            return format!("- `{}`", self.tool);
        }

        let (verb, succeed) = match self.action {
            ActionTag::Block => ("must be called", "must succeed"),
            ActionTag::Warn => ("should be called", "should succeed"),
            ActionTag::Allow => unreachable!(),
        };

        let action_phrase = if self.must_succeed {
            format!("{verb} and {succeed}")
        } else if self.when_files_match.is_some() {
            verb.to_string()
        } else {
            format!("{verb} at least once")
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

fn condition_to_requirements(cond: &PolicyCondition, action: ActionTag) -> Vec<ToolRequirement> {
    match cond {
        PolicyCondition::RequiredToolCall {
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
        PolicyCondition::ConditionalToolCall {
            tool_name,
            when_files_match,
            must_succeed,
            ..
        } => vec![ToolRequirement {
            tool: tool_name.clone(),
            action,
            must_succeed: *must_succeed,
            when_files_match: when_files_match.clone(),
        }],
        // Other conditions (TokenBudget, ModelAllowlist, etc.) are
        // server-evaluated and not actionable by the agent — skip.
        _ => Vec::new(),
    }
}

fn scope_applies_to_session(scope: &PolicyScope) -> bool {
    matches!(scope, PolicyScope::Session | PolicyScope::Both)
}

fn scope_applies_to_window(scope: &PolicyScope) -> bool {
    matches!(scope, PolicyScope::ValidationWindow | PolicyScope::Both)
}

/// Render agent instructions as Markdown.
///
/// `policies` may include disabled entries — they're filtered out here.
/// `validation_window_mode` controls whether the validation window section is
/// rendered at all (it is hidden entirely when mode is `Disabled`).
pub fn render_markdown(
    policies: &[PolicyRule],
    validation_window_mode: &ValidationWindowMode,
) -> String {
    let mut session_reqs: Vec<ToolRequirement> = Vec::new();
    let mut window_required: Vec<ToolRequirement> = Vec::new();
    let mut window_allowed: Vec<ToolRequirement> = Vec::new();

    for p in policies.iter().filter(|p| p.enabled) {
        let action = ActionTag::from(&p.action);
        let reqs = condition_to_requirements(&p.condition, action);
        if reqs.is_empty() {
            continue;
        }

        if scope_applies_to_session(&p.scope) {
            session_reqs.extend(reqs.iter().cloned());
        }
        if scope_applies_to_window(&p.scope)
            && !matches!(validation_window_mode, ValidationWindowMode::Disabled)
        {
            for r in &reqs {
                if r.action == ActionTag::Allow {
                    window_allowed.push(r.clone());
                } else {
                    window_required.push(r.clone());
                }
            }
        }
    }

    let has_session = !session_reqs.is_empty();
    let has_window = (!window_required.is_empty() || !window_allowed.is_empty())
        && !matches!(validation_window_mode, ValidationWindowMode::Disabled);

    if !has_session && !has_window {
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

    if has_session {
        out.push_str("\n### Before push\n");
        out.push_str("The following pre-push checks apply to this repository:\n");
        for r in &session_reqs {
            out.push_str(&r.render_line());
            out.push('\n');
        }
    }

    if has_window {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::PolicySeverity;
    use uuid::Uuid;

    fn rule(
        condition: PolicyCondition,
        action: PolicyAction,
        scope: PolicyScope,
        enabled: bool,
    ) -> PolicyRule {
        PolicyRule {
            id: Uuid::new_v4(),
            org_id: None,
            name: "test".into(),
            description: "test".into(),
            condition,
            action,
            severity: PolicySeverity::High,
            enabled,
            scope,
        }
    }

    fn required(tools: &[&str], must_succeed: bool) -> PolicyCondition {
        PolicyCondition::RequiredToolCall {
            tool_names: tools.iter().map(|s| s.to_string()).collect(),
            must_succeed,
        }
    }

    fn conditional(tool: &str, files: &[&str], must_succeed: bool) -> PolicyCondition {
        PolicyCondition::ConditionalToolCall {
            tool_name: tool.into(),
            min_count: None,
            when_files_match: Some(files.iter().map(|s| s.to_string()).collect()),
            must_succeed,
        }
    }

    #[test]
    fn no_policies_renders_terse_message() {
        let out = render_markdown(&[], &ValidationWindowMode::Disabled);
        assert!(out.contains("No active policies"));
        assert!(!out.contains("Before push"));
        assert!(!out.contains("Validation window"));
    }

    #[test]
    fn disabled_policies_are_ignored() {
        let p = rule(
            required(&["cargo_fmt"], false),
            PolicyAction::BlockPush,
            PolicyScope::Session,
            false,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn session_required_tool_block_renders_must_be_called() {
        let p = rule(
            required(&["cargo_fmt"], false),
            PolicyAction::BlockPush,
            PolicyScope::Session,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        assert!(out.contains("### Before push"));
        assert!(out.contains("`cargo_fmt`"));
        assert!(out.contains("must be called"));
    }

    #[test]
    fn session_required_tool_warn_uses_should_language() {
        let p = rule(
            required(&["cargo_fmt"], false),
            PolicyAction::Warn,
            PolicyScope::Session,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        let tool_line = out
            .lines()
            .find(|l| l.starts_with("- `cargo_fmt`"))
            .expect("tool line missing");
        assert!(tool_line.contains("should be called"));
        assert!(!tool_line.contains("must be called"));
    }

    #[test]
    fn must_succeed_modifies_clause() {
        let p_block = rule(
            required(&["cargo_check"], true),
            PolicyAction::BlockPush,
            PolicyScope::Session,
            true,
        );
        let out_block = render_markdown(&[p_block], &ValidationWindowMode::Disabled);
        assert!(out_block.contains("must be called and must succeed"));

        let p_warn = rule(
            required(&["cargo_check"], true),
            PolicyAction::Warn,
            PolicyScope::Session,
            true,
        );
        let out_warn = render_markdown(&[p_warn], &ValidationWindowMode::Disabled);
        assert!(out_warn.contains("should be called and should succeed"));
    }

    #[test]
    fn conditional_tool_call_renders_file_clause() {
        let p = rule(
            conditional("cargo_audit", &["Cargo.lock"], true),
            PolicyAction::BlockPush,
            PolicyScope::Session,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        assert!(out.contains("when files matching `Cargo.lock` are changed"));
        assert!(out.contains("must be called and must succeed"));
    }

    #[test]
    fn validation_window_required_section_renders() {
        let p = rule(
            required(&["agent_review"], true),
            PolicyAction::BlockPush,
            PolicyScope::ValidationWindow,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Block);
        assert!(out.contains("### Validation window"));
        assert!(out.contains("Required tools"));
        assert!(out.contains("`agent_review`"));
    }

    #[test]
    fn validation_window_allowed_section_renders() {
        let p = rule(
            required(&["Read", "Grep"], false),
            PolicyAction::Allow,
            PolicyScope::ValidationWindow,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Block);
        assert!(out.contains("Allowed tools"));
        assert!(out.contains("`Read`"));
        assert!(out.contains("`Grep`"));
        // Allow-listed tools render bare — no must/should language on those lines.
        let read_line = out.lines().find(|l| l.contains("`Read`")).unwrap();
        assert!(!read_line.contains("must be called"));
        assert!(!read_line.contains("should be called"));
    }

    #[test]
    fn validation_window_section_hidden_when_mode_disabled() {
        let p = rule(
            required(&["agent_review"], false),
            PolicyAction::BlockPush,
            PolicyScope::ValidationWindow,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        assert!(!out.contains("### Validation window"));
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn non_actionable_conditions_are_skipped() {
        let p = rule(
            PolicyCondition::TokenBudget {
                max_tokens: Some(1000),
                max_cost_usd: None,
            },
            PolicyAction::BlockPush,
            PolicyScope::Session,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Disabled);
        assert!(out.contains("No active policies"));
    }

    #[test]
    fn scope_both_applies_to_both_sections() {
        let p = rule(
            required(&["cargo_fmt"], false),
            PolicyAction::BlockPush,
            PolicyScope::Both,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Block);
        assert!(out.contains("### Before push"));
        assert!(out.contains("### Validation window"));
        assert_eq!(out.matches("`cargo_fmt`").count(), 2);
    }

    #[test]
    fn warn_mode_does_not_advertise_blocking_in_window() {
        let p = rule(
            required(&["agent_review"], false),
            PolicyAction::Warn,
            PolicyScope::ValidationWindow,
            true,
        );
        let out = render_markdown(&[p], &ValidationWindowMode::Warn);
        assert!(out.contains("### Validation window"));
        assert!(!out.contains("will block the push"));
        assert!(!out.contains("blocked"));
        assert!(out.contains("should be called"));
    }
}
