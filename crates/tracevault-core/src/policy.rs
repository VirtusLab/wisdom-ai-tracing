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

/// Which evaluation window this policy applies to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyScope {
    /// Evaluated over the entire push window (default, existing behaviour).
    #[default]
    Session,
    /// Evaluated only against events inside the last validation window.
    ValidationWindow,
    /// Evaluated in both contexts.
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    BlockPush,
    Warn,
    /// Tool is explicitly permitted inside a validation window without a count
    /// requirement. Prevents it from triggering the unknown-tool gate.
    Allow,
}

/// Org/repo-level setting controlling what happens when an unknown tool
/// (not covered by any validation_window-scoped policy) is called inside
/// the validation window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ValidationWindowMode {
    /// No window enforcement (default).
    #[default]
    Disabled,
    /// Unknown tool call is flagged but push succeeds.
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
