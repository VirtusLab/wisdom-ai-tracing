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
    },
    TokenBudget {
        max_tokens: Option<u64>,
        max_cost_usd: Option<f64>,
    },
    ConditionalToolCall {
        tool_name: String,
        min_count: Option<u32>,
        when_files_match: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyAction {
    BlockPush,
    Warn,
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
