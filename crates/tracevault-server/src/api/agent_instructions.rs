//! Endpoint for rendering agent-readable policy instructions.
//!
//! `GET /api/v1/orgs/{slug}/repos/{repo_id}/policies/agent-instructions`
//!
//! Consumed by the CLI (`tracevault agent-policies`), the `agent_policies`
//! MCP tool, and the dashboard preview. The rendering itself lives in
//! `tracevault_core::agent_policies` so all three surfaces produce identical
//! output.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::OrgAuth;
use crate::repo::policies::{PolicyRepo, PolicyRow};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct AgentInstructionsResponse {
    /// Output format. Currently always `"markdown"`.
    pub format: String,
    /// Rendered instructions.
    pub content: String,
}

pub async fn get_agent_instructions(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
) -> Result<Json<AgentInstructionsResponse>, AppError> {
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    let rows = PolicyRepo::list_for_repo(&state.pool, auth.org_id, repo_id).await?;
    let mode_str = PolicyRepo::get_validation_window_mode(&state.pool, repo_id).await?;

    // Skip rows that don't parse cleanly — they're invalid but shouldn't fail
    // the whole render. The renderer operates on core's native domain types.
    let rules: Vec<tracevault_core::policy::PolicyRule> = rows
        .into_iter()
        .filter_map(map_row_to_policy_rule)
        .collect();

    let mode: tracevault_core::policy::ValidationWindowMode =
        serde_json::from_value(serde_json::Value::String(mode_str))
            .unwrap_or(tracevault_core::policy::ValidationWindowMode::Disabled);

    let content = tracevault_core::agent_policies::render_markdown(&rules, &mode);

    Ok(Json(AgentInstructionsResponse {
        format: "markdown".into(),
        content,
    }))
}

/// Convert a DB row into a `PolicyRule`, returning None if any field fails to
/// deserialize (the row is then skipped — see `get_agent_instructions`).
fn map_row_to_policy_rule(row: PolicyRow) -> Option<tracevault_core::policy::PolicyRule> {
    use tracevault_core::policy::{
        PolicyAction, PolicyCondition, PolicyRule, PolicyScope, PolicySeverity,
    };

    let condition: PolicyCondition = serde_json::from_value(row.condition).ok()?;
    let action: PolicyAction = serde_json::from_value(serde_json::Value::String(row.action)).ok()?;
    let scope: PolicyScope = serde_json::from_value(serde_json::Value::String(row.scope)).ok()?;
    let severity: PolicySeverity =
        serde_json::from_value(serde_json::Value::String(row.severity)).ok()?;

    Some(PolicyRule {
        id: row.id,
        org_id: Some(row.org_id.to_string()),
        name: row.name,
        description: row.description,
        condition,
        action,
        severity,
        enabled: row.enabled,
        scope,
    })
}
