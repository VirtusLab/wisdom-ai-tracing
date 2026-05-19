use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::extractors::OrgAuth;
use crate::permissions::{has_permission, Permission};
use crate::repo::policies::{PolicyEvaluationFilter, PolicyRepo};
use crate::AppState;

fn require_policy_manage(role: &str) -> Result<(), AppError> {
    if !has_permission(role, Permission::PolicyManage) {
        return Err(AppError::Forbidden(
            "PolicyManage permission required (owner, admin, or policy_admin)".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct PolicyResponse {
    pub id: Uuid,
    pub org_id: Uuid,
    pub repo_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub condition: serde_json::Value,
    pub action: String,
    pub severity: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyRequest {
    pub name: String,
    pub description: Option<String>,
    pub condition: serde_json::Value,
    pub action: String,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePolicyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub action: Option<String>,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
}

/// GET /api/v1/repos/{repo_id}/policies
/// Returns all policies for a repo (repo-specific + org-wide)
pub async fn list_repo_policies(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
) -> Result<Json<Vec<PolicyResponse>>, AppError> {
    // Verify repo belongs to org
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    let rows = PolicyRepo::list_for_repo(&state.pool, auth.org_id, repo_id).await?;

    let policies = rows
        .into_iter()
        .map(|r| PolicyResponse {
            id: r.id,
            org_id: r.org_id,
            repo_id: r.repo_id,
            name: r.name,
            description: r.description,
            condition: r.condition,
            action: r.action,
            severity: r.severity,
            enabled: r.enabled,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(policies))
}

/// POST /api/v1/repos/{repo_id}/policies
/// Create a policy for this repo
pub async fn create_repo_policy(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Json(req): Json<CreatePolicyRequest>,
) -> Result<(StatusCode, Json<PolicyResponse>), AppError> {
    require_policy_manage(&auth.role)?;

    // Verify repo belongs to org
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    validate_action(&req.action)?;

    let description = req.description.as_deref().unwrap_or("");
    let severity = req.severity.as_deref().unwrap_or("medium");
    let enabled = req.enabled.unwrap_or(true);

    let (policy_id, created_at, updated_at) = PolicyRepo::create(
        &state.pool,
        auth.org_id,
        repo_id,
        &req.name,
        description,
        &req.condition,
        &req.action,
        severity,
        enabled,
    )
    .await?;

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "policy.create",
            "policy",
            Some(policy_id),
            Some(serde_json::json!({"name": &req.name})),
        ),
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(PolicyResponse {
            id: policy_id,
            org_id: auth.org_id,
            repo_id: Some(repo_id),
            name: req.name,
            description: req.description.unwrap_or_default(),
            condition: req.condition,
            action: req.action,
            severity: req.severity.unwrap_or_else(|| "medium".into()),
            enabled,
            created_at,
            updated_at,
        }),
    ))
}

/// PUT /api/v1/policies/{id}
/// Update a policy
pub async fn update_policy(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
    Json(req): Json<UpdatePolicyRequest>,
) -> Result<Json<PolicyResponse>, AppError> {
    require_policy_manage(&auth.role)?;

    if let Some(action) = &req.action {
        validate_action(action)?;
    }

    let row = PolicyRepo::update(
        &state.pool,
        id,
        auth.org_id,
        &req.name,
        &req.description,
        &req.condition,
        &req.action,
        &req.severity,
        req.enabled,
    )
    .await?;

    match row {
        Some(r) => {
            crate::audit::log(
                &state.pool,
                crate::audit::user_action(
                    auth.org_id,
                    auth.user_id,
                    "policy.update",
                    "policy",
                    Some(id),
                    None,
                ),
            )
            .await;

            Ok(Json(PolicyResponse {
                id,
                org_id: r.org_id,
                repo_id: r.repo_id,
                name: r.name,
                description: r.description,
                condition: r.condition,
                action: r.action,
                severity: r.severity,
                enabled: r.enabled,
                created_at: r.created_at,
                updated_at: r.updated_at,
            }))
        }
        None => Err(AppError::NotFound("Policy not found".into())),
    }
}

/// DELETE /api/v1/policies/{id}
/// Delete a policy
pub async fn delete_policy(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, id)): Path<(String, Uuid)>,
) -> Result<StatusCode, AppError> {
    require_policy_manage(&auth.role)?;

    let rows_affected = PolicyRepo::delete(&state.pool, id, auth.org_id).await?;

    if rows_affected == 0 {
        return Err(AppError::NotFound("Policy not found".into()));
    }

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "policy.delete",
            "policy",
            Some(id),
            None,
        ),
    )
    .await;

    Ok(StatusCode::OK)
}

// --- Policy Check (evaluation) ---

#[derive(Debug, Deserialize)]
pub struct CheckRequest {
    pub sessions: Vec<SessionCheckData>,
    /// HEAD commit SHA at the time of the check, if available. Optional for
    /// backwards compatibility with older CLIs.
    #[serde(default)]
    pub commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionCheckData {
    pub session_id: String,
    pub tool_calls: Option<serde_json::Value>, // {"tool_name": count}
    pub files_modified: Option<Vec<String>>,
    #[serde(rename = "total_tool_calls")]
    pub _total_tool_calls: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub passed: bool,
    pub results: Vec<CheckResult>,
    pub blocked: bool,
}

#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub rule_name: String,
    pub result: String, // "pass", "fail", "warn"
    pub action: String,
    pub severity: String,
    pub details: String,
}

/// POST /api/v1/repos/{repo_id}/policies/check
/// Evaluate all applicable policies against provided session data
pub async fn check_policies(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Json(req): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, AppError> {
    // Verify repo belongs to org
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    // Fetch all enabled policies for this repo (repo-specific + org-wide)
    let rows = PolicyRepo::list_enabled_for_check(&state.pool, auth.org_id, repo_id).await?;

    // Aggregate session data: merge tool_calls across all sessions, union files_modified.
    let mut all_tool_calls: std::collections::HashMap<
        String,
        tracevault_core::policy_eval::ToolCallStats,
    > = std::collections::HashMap::new();
    let mut all_files: Vec<String> = Vec::new();

    for session in &req.sessions {
        if let Some(tc) = &session.tool_calls {
            if let Some(obj) = tc.as_object() {
                for (k, v) in obj {
                    let delta = parse_tool_call_stats(v);
                    let stats = all_tool_calls.entry(k.clone()).or_default();
                    stats.total += delta.total;
                    stats.successful += delta.successful;
                }
            }
        }
        if let Some(files) = &session.files_modified {
            all_files.extend(files.iter().cloned());
        }
    }

    // Pick a representative session id for the activity log — if sessions
    // collapse across a push, storing each against the first one is fine
    // and still lets users filter by session.
    let session_id_for_log = req.sessions.first().map(|s| s.session_id.as_str());
    let actor_for_log = if auth.user_id.is_nil() {
        None
    } else {
        Some(auth.user_id)
    };

    let mut results = Vec::new();
    let mut has_block_failure = false;

    for (policy_id, name, condition, action, severity) in &rows {
        let check_result = tracevault_core::policy_eval::evaluate_condition(
            condition,
            &all_tool_calls,
            &all_files,
        );
        let result_str = classify_result(&check_result);

        if !check_result.passed && action == "block_push" {
            has_block_failure = true;
        }

        if let Err(e) = PolicyRepo::insert_evaluation(
            &state.pool,
            auth.org_id,
            repo_id,
            *policy_id,
            name,
            session_id_for_log,
            req.commit_sha.as_deref(),
            result_str,
            action,
            &check_result.details,
            "cli_check",
            actor_for_log,
        )
        .await
        {
            tracing::warn!(
                policy_id = %policy_id,
                error = %e,
                "failed to record policy evaluation"
            );
        }

        results.push(CheckResult {
            rule_name: name.clone(),
            result: result_str.into(),
            action: action.clone(),
            severity: severity.clone(),
            details: check_result.details,
        });
    }

    let all_passed = results.iter().all(|r| r.result == "pass");

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "policy.check",
            "commit",
            None,
            Some(serde_json::json!({"passed": all_passed, "blocked": has_block_failure})),
        ),
    )
    .await;

    Ok(Json(CheckResponse {
        passed: all_passed,
        results,
        blocked: has_block_failure,
    }))
}

// --- Policy Evaluation Activity (list) ---

#[derive(Debug, Deserialize)]
pub struct ListEvaluationsQuery {
    pub policy_id: Option<Uuid>,
    pub result: Option<String>,
    pub source: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvaluationItem {
    pub id: Uuid,
    pub policy_id: Option<Uuid>,
    pub policy_name: String,
    pub session_id: Option<String>,
    pub commit_sha: Option<String>,
    pub result: String,
    pub action: String,
    pub details: String,
    pub source: String,
    pub actor_id: Option<Uuid>,
    pub evaluated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PolicyEvaluationPage {
    pub items: Vec<PolicyEvaluationItem>,
    pub total: i64,
}

/// GET /api/v1/orgs/{slug}/repos/{repo_id}/policy-evaluations
/// List recent policy evaluations for this repo. Open to any org member —
/// this is operational visibility, not a mutation.
pub async fn list_policy_evaluations(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Query(q): Query<ListEvaluationsQuery>,
) -> Result<Json<PolicyEvaluationPage>, AppError> {
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    let limit = q.limit.unwrap_or(25).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);

    let filter = PolicyEvaluationFilter {
        policy_id: q.policy_id,
        result: q.result,
        source: q.source,
        since: q.since,
        limit,
        offset,
    };

    let (rows, total) = tokio::try_join!(
        PolicyRepo::list_evaluations(&state.pool, auth.org_id, repo_id, &filter),
        PolicyRepo::count_evaluations(&state.pool, auth.org_id, repo_id, &filter),
    )?;

    Ok(Json(PolicyEvaluationPage {
        items: rows
            .into_iter()
            .map(|r| PolicyEvaluationItem {
                id: r.id,
                policy_id: r.policy_id,
                policy_name: r.policy_name,
                session_id: r.session_id,
                commit_sha: r.commit_sha,
                result: r.result,
                action: r.action,
                details: r.details,
                source: r.source,
                actor_id: r.actor_id,
                evaluated_at: r.evaluated_at,
            })
            .collect(),
        total,
    }))
}

/// Map an EvalOutcome into the stored result string. Today evaluate_condition
/// only exposes pass/fail, but "skip" is already a concept inside the
/// evaluator (rule skipped when no files matched) — surface it here so the
/// activity log can distinguish "rule didn't apply" from "rule passed".
/// Parse a single tool_call value from the client payload into ToolCallStats.
/// Accepts legacy format (plain i64 count) and new format ({total, successful}).
/// Legacy counts treat all calls as successful for backward compatibility.
fn parse_tool_call_stats(v: &serde_json::Value) -> tracevault_core::policy_eval::ToolCallStats {
    if let Some(total) = v.as_i64() {
        // Legacy: plain count — treat all as successful
        tracevault_core::policy_eval::ToolCallStats {
            total,
            successful: total,
        }
    } else if let Some(o) = v.as_object() {
        tracevault_core::policy_eval::ToolCallStats {
            total: o.get("total").and_then(|x| x.as_i64()).unwrap_or(0),
            successful: o.get("successful").and_then(|x| x.as_i64()).unwrap_or(0),
        }
    } else {
        tracevault_core::policy_eval::ToolCallStats::default()
    }
}

fn classify_result(outcome: &tracevault_core::policy_eval::EvalOutcome) -> &'static str {
    if !outcome.passed {
        "fail"
    } else if outcome.details.starts_with("Rule skipped") {
        "skip"
    } else {
        "pass"
    }
}

/// Actions the enforcement engine actually handles. Keep in sync with
/// `PolicyAction` in tracevault-core.
const VALID_ACTIONS: &[&str] = &["block_push", "warn"];

fn validate_action(action: &str) -> Result<(), AppError> {
    if !VALID_ACTIONS.contains(&action) {
        return Err(AppError::BadRequest(format!(
            "action must be one of: {}",
            VALID_ACTIONS.join(", ")
        )));
    }
    Ok(())
}
