//! Render agent-readable Markdown instructions from a set of policies.
//!
//! Consumed by `tracevault agent-policies`, the `agent_policies` MCP tool, and
//! the dashboard preview. Lives in `tracevault-core` so there is exactly one
//! rendering implementation regardless of which frontend asks for it.

use crate::policy::{
    PolicyAction, PolicyCondition, PolicyRule, PolicyScope, VerificationPhaseMode,
};

/// One rendered tool-requirement line.
#[derive(Debug, Clone)]
struct ToolRequirement {
    tool: String,
    action: PolicyAction,
    must_succeed: bool,
    when_files_match: Option<Vec<String>>,
}

impl ToolRequirement {
    fn render_line(&self) -> String {
        let (verb, succeed) = match self.action {
            PolicyAction::Allow => return format!("- `{}`", self.tool),
            PolicyAction::BlockPush => ("must be called", "must succeed"),
            PolicyAction::Warn => ("should be called", "should succeed"),
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

fn condition_to_requirements(cond: &PolicyCondition, action: PolicyAction) -> Vec<ToolRequirement> {
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

fn scope_applies_to_verification(scope: &PolicyScope) -> bool {
    matches!(scope, PolicyScope::VerificationPhase | PolicyScope::Both)
}

/// Render agent instructions as Markdown.
///
/// `policies` may include disabled entries — they're filtered out here.
/// `verification_phase_mode` controls whether the verification phase section is
/// rendered at all (it is hidden entirely when mode is `Disabled`).
pub fn render_markdown(
    policies: &[PolicyRule],
    verification_phase_mode: &VerificationPhaseMode,
) -> String {
    let mut session_reqs: Vec<ToolRequirement> = Vec::new();
    let mut verification_required: Vec<ToolRequirement> = Vec::new();
    let mut verification_allowed: Vec<ToolRequirement> = Vec::new();

    for p in policies.iter().filter(|p| p.enabled) {
        let reqs = condition_to_requirements(&p.condition, p.action);
        if reqs.is_empty() {
            continue;
        }

        if scope_applies_to_session(&p.scope) {
            session_reqs.extend(reqs.iter().cloned());
        }
        if scope_applies_to_verification(&p.scope)
            && !matches!(verification_phase_mode, VerificationPhaseMode::Disabled)
        {
            for r in &reqs {
                if r.action == PolicyAction::Allow {
                    verification_allowed.push(r.clone());
                } else {
                    verification_required.push(r.clone());
                }
            }
        }
    }

    let has_session = !session_reqs.is_empty();
    let has_verification = (!verification_required.is_empty() || !verification_allowed.is_empty())
        && !matches!(verification_phase_mode, VerificationPhaseMode::Disabled);

    if !has_session && !has_verification {
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

    if has_verification {
        // The consequence sentence depends on the enforcement mode. In
        // practice this section is only reached when mode is Warn or Block
        // (has_verification short-circuits Disabled above), but that is a
        // runtime invariant, not one the compiler enforces — so the Disabled
        // arm yields a neutral empty consequence rather than `unreachable!`.
        // If the gating logic ever drifts, we render a harmless instruction
        // instead of panicking in the middle of building agent output.
        let consequence = match verification_phase_mode {
            VerificationPhaseMode::Block => "Any other tool call will fail the push.",
            VerificationPhaseMode::Warn => {
                "Any other tool call will be recorded as a warning on the push."
            }
            VerificationPhaseMode::Disabled => "",
        };
        out.push_str("\n### Verification phase (pre-push gating)\n");
        out.push_str(&format!(
            "When you are done changing code and ready to push, you must enter a \
            **verification phase** by running:\n\n    tracevault verify-start\n\n\
            From that point until the push, you are only allowed to call the tools \
            listed below. {consequence}\n\n\
            The intent: catch agents that pretend to review while still editing. \
            If you need to make a code change after entering the phase, that is fine — \
            make the change, then run `tracevault verify-start` again to restart the \
            phase and rerun the required tools. The most recent `verify-start` is the \
            only one that counts.\n",
        ));

        if !verification_required.is_empty() {
            out.push_str("\n**Required** — must be called inside the phase:\n");
            for r in &verification_required {
                out.push_str(&r.render_line());
                out.push('\n');
            }
        }

        if !verification_allowed.is_empty() {
            out.push_str("\n**Allowed** — may be called inside the phase without restriction:\n");
            for r in &verification_allowed {
                out.push_str(&r.render_line());
                out.push('\n');
            }
        }
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
        let out = render_markdown(&[], &VerificationPhaseMode::Disabled);
        assert!(out.contains("No active policies"));
        assert!(!out.contains("Before push"));
        assert!(!out.contains("Verification phase"));
    }

    #[test]
    fn disabled_policies_are_ignored() {
        let p = rule(
            required(&["cargo_fmt"], false),
            PolicyAction::BlockPush,
            PolicyScope::Session,
            false,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
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
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
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
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
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
        let out_block = render_markdown(&[p_block], &VerificationPhaseMode::Disabled);
        assert!(out_block.contains("must be called and must succeed"));

        let p_warn = rule(
            required(&["cargo_check"], true),
            PolicyAction::Warn,
            PolicyScope::Session,
            true,
        );
        let out_warn = render_markdown(&[p_warn], &VerificationPhaseMode::Disabled);
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
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
        assert!(out.contains("when files matching `Cargo.lock` are changed"));
        assert!(out.contains("must be called and must succeed"));
    }

    #[test]
    fn verification_phase_required_section_renders() {
        let p = rule(
            required(&["agent_review"], true),
            PolicyAction::BlockPush,
            PolicyScope::VerificationPhase,
            true,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Block);
        assert!(out.contains("### Verification phase"));
        assert!(out.contains("**Required**"));
        assert!(out.contains("`agent_review`"));
    }

    #[test]
    fn verification_phase_allowed_section_renders() {
        let p = rule(
            required(&["Read", "Grep"], false),
            PolicyAction::Allow,
            PolicyScope::VerificationPhase,
            true,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Block);
        assert!(out.contains("**Allowed**"));
        assert!(out.contains("`Read`"));
        assert!(out.contains("`Grep`"));
        // Allow-listed tools render bare — no must/should language on those lines.
        let read_line = out.lines().find(|l| l.contains("`Read`")).unwrap();
        assert!(!read_line.contains("must be called"));
        assert!(!read_line.contains("should be called"));
    }

    #[test]
    fn verification_phase_section_hidden_when_mode_disabled() {
        let p = rule(
            required(&["agent_review"], false),
            PolicyAction::BlockPush,
            PolicyScope::VerificationPhase,
            true,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
        assert!(!out.contains("### Verification phase"));
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
        let out = render_markdown(&[p], &VerificationPhaseMode::Disabled);
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
        let out = render_markdown(&[p], &VerificationPhaseMode::Block);
        assert!(out.contains("### Before push"));
        assert!(out.contains("### Verification phase"));
        assert_eq!(out.matches("`cargo_fmt`").count(), 2);
    }

    #[test]
    fn warn_mode_does_not_advertise_blocking_in_phase() {
        let p = rule(
            required(&["agent_review"], false),
            PolicyAction::Warn,
            PolicyScope::VerificationPhase,
            true,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Warn);
        assert!(out.contains("### Verification phase"));
        // Critical: in Warn mode the consequence sentence must NOT promise to
        // fail the push — that wording belongs to Block mode only.
        assert!(
            !out.contains("will fail the push"),
            "Warn mode must not advertise blocking behavior; output: {out}"
        );
        assert!(out.contains("recorded as a warning"));
        assert!(out.contains("should be called"));
    }

    #[test]
    fn block_mode_advertises_failing_the_push() {
        let p = rule(
            required(&["agent_review"], false),
            PolicyAction::BlockPush,
            PolicyScope::VerificationPhase,
            true,
        );
        let out = render_markdown(&[p], &VerificationPhaseMode::Block);
        assert!(out.contains("### Verification phase"));
        // Critical: in Block mode the consequence sentence must say so.
        assert!(
            out.contains("will fail the push"),
            "Block mode must explicitly state the push will fail; output: {out}"
        );
        assert!(
            !out.contains("recorded as a warning"),
            "Block mode must not use Warn-mode phrasing; output: {out}"
        );
    }
}
