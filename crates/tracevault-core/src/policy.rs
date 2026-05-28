use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: Uuid,
    pub org_id: Option<String>,
    pub name: String,
    pub description: String,
    pub condition: PolicyCondition,
    pub action: PolicyAction,
    pub severity: PolicySeverity,
    pub enabled: bool,
    #[serde(default)]
    pub scope: PolicyScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PolicyCondition {
    TraceCompleteness,
    AiPercentageThreshold {
        threshold: f32,
    },
    ModelAllowlist {
        allowed_models: Vec<String>,
    },
    SensitivePathPattern {
        patterns: Vec<String>,
    },
    RequiredToolCall {
        tool_names: Vec<String>,
        #[serde(default)]
        must_succeed: bool,
    },
    TokenBudget {
        max_tokens: Option<u64>,
        max_cost_usd: Option<f64>,
    },
    ConditionalToolCall {
        tool_name: String,
        min_count: Option<u32>,
        when_files_match: Option<Vec<String>>,
        #[serde(default)]
        must_succeed: bool,
    },
}

/// Which evaluation phase this policy applies to.
///
/// The "verification phase" is the period after the agent declares it is
/// done changing code (via `tracevault verify-start`) and is now running
/// pre-push checks. Policies scoped to that phase evaluate only tool calls
/// made inside it; policies scoped to `Session` evaluate the whole push.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyScope {
    /// Evaluated over the entire push (default).
    #[default]
    Session,
    /// Evaluated only against events inside the current verification phase.
    VerificationPhase,
    /// Evaluated in both contexts.
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    BlockPush,
    Warn,
    /// Tool is explicitly permitted inside the verification phase without
    /// a count requirement. Prevents it from triggering the unknown-tool
    /// gate.
    Allow,
}

/// Org/repo-level setting controlling what happens when an unknown tool
/// (not covered by any `verification_phase`-scoped policy) is called inside
/// an active verification phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerificationPhaseMode {
    /// No verification-phase enforcement (default).
    #[default]
    Disabled,
    /// Unknown tool call is flagged but the push still succeeds.
    Warn,
    /// Unknown tool call blocks the push.
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySeverity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvaluation {
    pub policy: PolicyRule,
    pub result: EvalResult,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EvalResult {
    Pass,
    Fail,
    Warn,
}
