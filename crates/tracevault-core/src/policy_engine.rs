use crate::policy::*;
use crate::trace::TraceRecord;
use uuid::Uuid;

pub struct PolicyEngine {
    policies: Vec<PolicyRule>,
}

impl PolicyEngine {
    pub fn new(policies: Vec<PolicyRule>) -> Self {
        Self { policies }
    }

    pub fn with_defaults() -> Self {
        Self::new(default_policies())
    }

    pub fn evaluate(&self, trace: &TraceRecord) -> Vec<PolicyEvaluation> {
        self.policies
            .iter()
            .filter(|p| p.enabled)
            .map(|p| evaluate_policy(p, trace))
            .collect()
    }
}

fn evaluate_policy(policy: &PolicyRule, trace: &TraceRecord) -> PolicyEvaluation {
    let (result, details) = match &policy.condition {
        PolicyCondition::TraceCompleteness => eval_trace_completeness(trace),
        PolicyCondition::AiPercentageThreshold { .. } => {
            // AI percentage is now computed server-side from commit_attributions.
            // The client-side policy engine cannot evaluate this.
            (
                EvalResult::Pass,
                "AI percentage evaluated server-side".into(),
            )
        }
        PolicyCondition::ModelAllowlist { allowed_models } => {
            eval_model_allowlist(trace, allowed_models)
        }
        PolicyCondition::SensitivePathPattern { .. } => {
            // Sensitive path checking requires attribution data, now server-side only.
            (
                EvalResult::Pass,
                "Sensitive path review evaluated server-side".into(),
            )
        }
        PolicyCondition::RequiredToolCall { tool_names } => {
            eval_required_tool_call(trace, tool_names)
        }
        PolicyCondition::TokenBudget {
            max_tokens,
            max_cost_usd,
        } => eval_token_budget(trace, *max_tokens, *max_cost_usd),
        PolicyCondition::ConditionalToolCall {
            tool_name,
            min_count,
            when_files_match: _,
        } => eval_conditional_tool_call(trace, tool_name, *min_count),
    };

    PolicyEvaluation {
        policy: policy.clone(),
        result,
        details,
    }
}

fn eval_trace_completeness(trace: &TraceRecord) -> (EvalResult, String) {
    let has_session = !trace.session.session_id.is_empty();
    let has_model = trace.model.is_some();

    if has_session && has_model {
        (EvalResult::Pass, "Trace is complete".into())
    } else {
        let mut missing = vec![];
        if !has_session {
            missing.push("session");
        }
        if !has_model {
            missing.push("model");
        }
        (EvalResult::Warn, format!("Missing: {}", missing.join(", ")))
    }
}

fn eval_model_allowlist(trace: &TraceRecord, allowed: &[String]) -> (EvalResult, String) {
    if allowed.is_empty() {
        return (EvalResult::Pass, "No allowlist configured".into());
    }
    match &trace.model {
        Some(model) if allowed.iter().any(|a| model.contains(a)) => {
            (EvalResult::Pass, format!("Model {model} is allowed"))
        }
        Some(model) => (
            EvalResult::Fail,
            format!("Model {model} is not in allowlist: {}", allowed.join(", ")),
        ),
        None => (EvalResult::Fail, "No model specified in trace".into()),
    }
}

fn eval_required_tool_call(trace: &TraceRecord, required: &[String]) -> (EvalResult, String) {
    let used: Vec<_> = trace.session.tools_used.iter().map(|t| &t.name).collect();
    let missing: Vec<_> = required
        .iter()
        .filter(|r| !used.iter().any(|u| u.as_str() == r.as_str()))
        .collect();

    if missing.is_empty() {
        (EvalResult::Pass, "All required tools were used".into())
    } else {
        (
            EvalResult::Warn,
            format!(
                "Missing required tools: {}",
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    }
}

fn eval_token_budget(
    trace: &TraceRecord,
    max_tokens: Option<u64>,
    max_cost: Option<f64>,
) -> (EvalResult, String) {
    let tokens = trace.session.token_usage.total_tokens;
    let cost = trace.session.token_usage.estimated_cost_usd;

    let token_over = max_tokens.is_some_and(|max| tokens > max);
    let cost_over = max_cost.is_some_and(|max| cost > max);

    if token_over || cost_over {
        (
            EvalResult::Warn,
            format!("Budget exceeded: {tokens} tokens (${cost:.2})"),
        )
    } else {
        (
            EvalResult::Pass,
            format!("Within budget: {tokens} tokens (${cost:.2})"),
        )
    }
}

fn eval_conditional_tool_call(
    trace: &TraceRecord,
    tool_name: &str,
    min_count: Option<u32>,
) -> (EvalResult, String) {
    let min = min_count.unwrap_or(1) as usize;
    let count = trace
        .session
        .tools_used
        .iter()
        .filter(|t| t.name == tool_name)
        .count();

    if count >= min {
        (
            EvalResult::Pass,
            format!(
                "Tool '{}' called {} time(s) (required >= {})",
                tool_name, count, min
            ),
        )
    } else {
        (
            EvalResult::Fail,
            format!(
                "Tool '{}' called {} time(s) (required >= {})",
                tool_name, count, min
            ),
        )
    }
}

fn default_policies() -> Vec<PolicyRule> {
    vec![
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "Trace completeness".into(),
            description: "Every AI commit must have complete trace data".into(),
            condition: PolicyCondition::TraceCompleteness,
            action: PolicyAction::Warn,
            severity: PolicySeverity::Medium,
            enabled: true,
        },
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "AI percentage threshold".into(),
            description: "Warn when AI-generated code exceeds 90%".into(),
            condition: PolicyCondition::AiPercentageThreshold { threshold: 90.0 },
            action: PolicyAction::Warn,
            severity: PolicySeverity::Medium,
            enabled: true,
        },
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "Model allowlist".into(),
            description: "Only approved models may generate code".into(),
            condition: PolicyCondition::ModelAllowlist {
                allowed_models: vec![
                    "anthropic/claude".into(),
                    "openai/gpt".into(),
                    "google/gemini".into(),
                ],
            },
            action: PolicyAction::BlockPush,
            severity: PolicySeverity::High,
            enabled: true,
        },
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "Sensitive path review".into(),
            description: "AI code in sensitive paths requires review".into(),
            condition: PolicyCondition::SensitivePathPattern {
                patterns: vec![
                    "payments".into(),
                    "auth".into(),
                    "security".into(),
                    "crypto".into(),
                ],
            },
            action: PolicyAction::Warn,
            severity: PolicySeverity::High,
            enabled: true,
        },
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "Required tool call".into(),
            description: "Trace must show that tests were run".into(),
            condition: PolicyCondition::RequiredToolCall { tool_names: vec![] },
            action: PolicyAction::Warn,
            severity: PolicySeverity::Low,
            enabled: true,
        },
        PolicyRule {
            id: Uuid::nil(),
            org_id: None,
            name: "Token budget".into(),
            description: "Warn when token usage exceeds budget".into(),
            condition: PolicyCondition::TokenBudget {
                max_tokens: Some(500_000),
                max_cost_usd: Some(50.0),
            },
            action: PolicyAction::Warn,
            severity: PolicySeverity::Medium,
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_usage::TokenUsage;
    use crate::trace::{Session, ToolCall};
    use chrono::Utc;

    fn make_trace(session_id: &str, model: Option<&str>) -> TraceRecord {
        TraceRecord {
            id: Uuid::nil(),
            repo_id: "repo".into(),
            commit_sha: "abc123".into(),
            branch: None,
            author: "dev".into(),
            created_at: Utc::now(),
            model: model.map(String::from),
            tool: "claude-code".into(),
            tool_version: None,
            session: Session {
                session_id: session_id.into(),
                started_at: Utc::now(),
                ended_at: None,
                prompts: vec![],
                responses: vec![],
                token_usage: TokenUsage::default(),
                tools_used: vec![],
            },
            agent_trace: None,
            signature: None,
        }
    }

    fn make_trace_with_tools(tools: Vec<&str>) -> TraceRecord {
        let mut trace = make_trace("sess-1", Some("anthropic/claude-3"));
        trace.session.tools_used = tools
            .into_iter()
            .map(|name| ToolCall {
                name: name.into(),
                input_summary: String::new(),
                timestamp: Utc::now(),
            })
            .collect();
        trace
    }

    fn make_trace_with_tokens(total_tokens: u64, cost: f64) -> TraceRecord {
        let mut trace = make_trace("sess-1", Some("anthropic/claude-3"));
        trace.session.token_usage.total_tokens = total_tokens;
        trace.session.token_usage.estimated_cost_usd = cost;
        trace
    }

    // --- TraceCompleteness ---

    #[test]
    fn completeness_pass_when_both_present() {
        let trace = make_trace("sess-1", Some("claude-3"));
        let (result, _) = eval_trace_completeness(&trace);
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn completeness_fail_missing_session_id() {
        let trace = make_trace("", Some("claude-3"));
        let (result, details) = eval_trace_completeness(&trace);
        assert_eq!(result, EvalResult::Warn);
        assert!(details.contains("session"));
    }

    #[test]
    fn completeness_fail_missing_model() {
        let trace = make_trace("sess-1", None);
        let (result, details) = eval_trace_completeness(&trace);
        assert_eq!(result, EvalResult::Warn);
        assert!(details.contains("model"));
    }

    #[test]
    fn completeness_fail_both_missing() {
        let trace = make_trace("", None);
        let (result, details) = eval_trace_completeness(&trace);
        assert_eq!(result, EvalResult::Warn);
        assert!(details.contains("session"));
        assert!(details.contains("model"));
    }

    // --- ModelAllowlist ---

    #[test]
    fn model_allowlist_pass_empty_list() {
        let trace = make_trace("sess-1", Some("anything"));
        let (result, _) = eval_model_allowlist(&trace, &[]);
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn model_allowlist_pass_match() {
        let trace = make_trace("sess-1", Some("anthropic/claude-3"));
        let allowed = vec!["anthropic/claude".into(), "openai/gpt".into()];
        let (result, _) = eval_model_allowlist(&trace, &allowed);
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn model_allowlist_fail_no_match() {
        let trace = make_trace("sess-1", Some("unknown/model"));
        let allowed = vec!["anthropic/claude".into(), "openai/gpt".into()];
        let (result, _) = eval_model_allowlist(&trace, &allowed);
        assert_eq!(result, EvalResult::Fail);
    }

    // --- RequiredToolCall ---

    #[test]
    fn required_tool_call_pass_all_present() {
        let trace = make_trace_with_tools(vec!["cargo test", "cargo clippy"]);
        let required = vec!["cargo test".into(), "cargo clippy".into()];
        let (result, _) = eval_required_tool_call(&trace, &required);
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn required_tool_call_warn_missing() {
        let trace = make_trace_with_tools(vec!["cargo test"]);
        let required = vec!["cargo test".into(), "cargo clippy".into()];
        let (result, details) = eval_required_tool_call(&trace, &required);
        assert_eq!(result, EvalResult::Warn);
        assert!(details.contains("cargo clippy"));
    }

    // --- TokenBudget ---

    #[test]
    fn token_budget_pass_under_limits() {
        let trace = make_trace_with_tokens(1000, 1.0);
        let (result, _) = eval_token_budget(&trace, Some(5000), Some(10.0));
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn token_budget_warn_over_max_tokens() {
        let trace = make_trace_with_tokens(10_000, 1.0);
        let (result, _) = eval_token_budget(&trace, Some(5000), Some(10.0));
        assert_eq!(result, EvalResult::Warn);
    }

    #[test]
    fn token_budget_warn_over_max_cost() {
        let trace = make_trace_with_tokens(1000, 20.0);
        let (result, _) = eval_token_budget(&trace, Some(5000), Some(10.0));
        assert_eq!(result, EvalResult::Warn);
    }

    #[test]
    fn token_budget_pass_both_none() {
        let trace = make_trace_with_tokens(999_999, 999.0);
        let (result, _) = eval_token_budget(&trace, None, None);
        assert_eq!(result, EvalResult::Pass);
    }

    // --- ConditionalToolCall ---

    #[test]
    fn conditional_tool_call_pass_count_met() {
        let trace = make_trace_with_tools(vec!["cargo test", "cargo test"]);
        let (result, _) = eval_conditional_tool_call(&trace, "cargo test", Some(2));
        assert_eq!(result, EvalResult::Pass);
    }

    #[test]
    fn conditional_tool_call_fail_count_not_met() {
        let trace = make_trace_with_tools(vec!["cargo test"]);
        let (result, _) = eval_conditional_tool_call(&trace, "cargo test", Some(3));
        assert_eq!(result, EvalResult::Fail);
    }

    #[test]
    fn conditional_tool_call_fail_absent() {
        let trace = make_trace_with_tools(vec!["cargo clippy"]);
        let (result, _) = eval_conditional_tool_call(&trace, "cargo test", Some(1));
        assert_eq!(result, EvalResult::Fail);
    }

    // --- PolicyEngine ---

    #[test]
    fn evaluate_skips_disabled_policies() {
        let policies = vec![
            PolicyRule {
                id: Uuid::nil(),
                org_id: None,
                name: "enabled".into(),
                description: String::new(),
                condition: PolicyCondition::TraceCompleteness,
                action: PolicyAction::Warn,
                severity: PolicySeverity::Low,
                enabled: true,
            },
            PolicyRule {
                id: Uuid::nil(),
                org_id: None,
                name: "disabled".into(),
                description: String::new(),
                condition: PolicyCondition::TraceCompleteness,
                action: PolicyAction::Warn,
                severity: PolicySeverity::Low,
                enabled: false,
            },
        ];
        let engine = PolicyEngine::new(policies);
        let trace = make_trace("sess-1", Some("claude-3"));
        let results = engine.evaluate(&trace);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].policy.name, "enabled");
    }

    #[test]
    fn with_defaults_returns_six_enabled() {
        let engine = PolicyEngine::with_defaults();
        let trace = make_trace("sess-1", Some("anthropic/claude-3"));
        let results = engine.evaluate(&trace);
        assert_eq!(results.len(), 6);
        assert!(results.iter().all(|r| r.policy.enabled));
    }
}
