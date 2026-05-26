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
use crate::repo::policies::PolicyRepo;
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

    // Build PolicyRule values from the DB rows so the core renderer can
    // operate on its native domain types. Skip rows whose condition/action/
    // scope/severity can't be parsed — they're invalid but shouldn't fail
    // the whole render.
    let rules: Vec<tracevault_core::policy::PolicyRule> = rows
        .into_iter()
        .filter_map(|r| {
            let condition: tracevault_core::policy::PolicyCondition =
                serde_json::from_value(r.condition).ok()?;
            let action: tracevault_core::policy::PolicyAction =
                serde_json::from_value(serde_json::Value::String(r.action)).ok()?;
            let scope: tracevault_core::policy::PolicyScope =
                serde_json::from_value(serde_json::Value::String(r.scope)).ok()?;
            let severity: tracevault_core::policy::PolicySeverity =
                serde_json::from_value(serde_json::Value::String(r.severity)).ok()?;
            Some(tracevault_core::policy::PolicyRule {
                id: r.id,
                org_id: Some(r.org_id.to_string()),
                name: r.name,
                description: r.description,
                condition,
                action,
                severity,
                enabled: r.enabled,
                scope,
            })
        })
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
