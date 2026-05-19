use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use tracevault_core::policy_eval::{evaluate_condition, EvalOutcome, ToolCallStats};

use crate::error::AppError;
use crate::extractors::OrgAuth;
use crate::repo::policies::PolicyRepo;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CiVerifyRequest {
    pub commits: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CiVerifyResponse {
    pub status: String,
    pub total_commits: usize,
    pub registered_commits: usize,
    pub sealed_commits: usize,
    pub policy_passed_commits: usize,
    pub results: Vec<CommitVerifyResult>,
}

#[derive(Debug, Serialize)]
pub struct CommitVerifyResult {
    pub commit_sha: String,
    pub status: String,
    pub registered: bool,
    pub sealed: bool,
    pub signature_valid: bool,
    pub chain_valid: bool,
    pub attributed_sessions_sealed: bool,
    pub policy_results: Vec<PolicyResult>,
}

#[derive(Debug, Serialize)]
pub struct PolicyResult {
    pub rule_name: String,
    pub result: String,
    pub action: String,
    pub severity: String,
    pub details: String,
}

/// POST /api/v1/repos/{repo_id}/ci/verify
/// Verify a list of commits against the TraceVault database.
pub async fn verify_commits(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, repo_id)): Path<(String, Uuid)>,
    Json(req): Json<CiVerifyRequest>,
) -> Result<Json<CiVerifyResponse>, AppError> {
    // Verify repo belongs to org
    if !PolicyRepo::repo_belongs_to_org(&state.pool, repo_id, auth.org_id).await? {
        return Err(AppError::NotFound("Repo not found".into()));
    }

    // Fetch all enabled policies for this repo
    let policies = PolicyRepo::list_enabled_for_check(&state.pool, auth.org_id, repo_id).await?;

    let mut results = Vec::new();
    let mut registered_count = 0usize;
    let mut sealed_count = 0usize;
    let mut policy_passed_count = 0usize;

    for commit_sha in &req.commits {
        // Look up commit by (repo_id, commit_sha)
        let commit = sqlx::query_as::<
            _,
            (
                Uuid,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<chrono::DateTime<chrono::Utc>>,
            ),
        >(
            "SELECT c.id, cs.record_hash, cs.chain_hash, cs.prev_chain_hash, cs.signature, cs.sealed_at
             FROM commits c
             LEFT JOIN commit_seals cs ON cs.commit_id = c.id
             WHERE c.repo_id = $1 AND c.commit_sha = $2",
        )
        .bind(repo_id)
        .bind(commit_sha)
        .fetch_optional(&state.pool)
        .await?;

        let Some((commit_id, record_hash, chain_hash, prev_chain_hash, signature, sealed_at)) =
            commit
        else {
            results.push(CommitVerifyResult {
                commit_sha: commit_sha.clone(),
                status: "unregistered".into(),
                registered: false,
                sealed: false,
                signature_valid: false,
                chain_valid: false,
                attributed_sessions_sealed: false,
                policy_results: vec![],
            });
            continue;
        };

        registered_count += 1;

        if sealed_at.is_none() {
            results.push(CommitVerifyResult {
                commit_sha: commit_sha.clone(),
                status: "unsealed".into(),
                registered: true,
                sealed: false,
                signature_valid: false,
                chain_valid: false,
                attributed_sessions_sealed: false,
                policy_results: vec![],
            });
            continue;
        }

        sealed_count += 1;

        // Load per-org signing service at the time the commit was sealed
        let svc = if let Some(ref sat) = sealed_at {
            let encryption_key = state.encryption_key.as_deref();
            if let Some(ek) = encryption_key {
                crate::org_signing::load_at_time(&state.pool, auth.org_id, sat, ek)
                    .await
                    .map_err(AppError::internal)?
            } else {
                None
            }
        } else {
            None
        };

        // Verify signature and chain
        let signature_valid = match (&record_hash, &signature, &svc) {
            (Some(rh), Some(sig), Some(svc)) => svc.verify(rh, sig),
            _ => false,
        };

        let chain_valid = match (&record_hash, &chain_hash, &svc) {
            (Some(rh), Some(ch), Some(svc)) => {
                let expected = svc.chain_hash(prev_chain_hash.as_deref(), rh);
                expected == *ch
            }
            _ => false,
        };

        // Re-evaluate policies from events/file_changes linked via commit_attributions
        // Get distinct session IDs linked to this commit
        let session_ids = sqlx::query_as::<_, (Uuid,)>(
            "SELECT DISTINCT ca.session_id FROM commit_attributions ca WHERE ca.commit_id = $1",
        )
        .bind(commit_id)
        .fetch_all(&state.pool)
        .await?;

        let sids: Vec<Uuid> = session_ids.into_iter().map(|(id,)| id).collect();

        // Check if all attributed sessions have at least one seal
        let attributed_sessions_sealed = if sids.is_empty() {
            true // No sessions = vacuously true
        } else {
            let sealed_session_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(DISTINCT ca.session_id)
                 FROM commit_attributions ca
                 WHERE ca.commit_id = $1
                   AND EXISTS (SELECT 1 FROM session_seals ss WHERE ss.session_id = ca.session_id)",
            )
            .bind(commit_id)
            .fetch_one(&state.pool)
            .await?;

            sealed_session_count as usize == sids.len()
        };

        // Aggregate tool_calls from events table (total + successful)
        let mut all_tool_calls: std::collections::HashMap<String, ToolCallStats> =
            std::collections::HashMap::new();

        if !sids.is_empty() {
            let tool_counts = sqlx::query_as::<_, (String, i64, i64)>(include_str!(
                "../repo/sql/aggregate_tool_calls_by_sessions.sql"
            ))
            .bind(&sids)
            .fetch_all(&state.pool)
            .await?;

            for (name, total, successful) in tool_counts {
                let stats = all_tool_calls.entry(name).or_default();
                stats.total += total;
                stats.successful += successful;
            }
        }

        // Aggregate files_modified from file_changes table
        let all_files: Vec<String> = if !sids.is_empty() {
            sqlx::query_scalar::<_, String>(
                "SELECT DISTINCT fc.file_path FROM file_changes fc
                 WHERE fc.session_id = ANY($1)",
            )
            .bind(&sids)
            .fetch_all(&state.pool)
            .await?
        } else {
            vec![]
        };

        // Evaluate each policy and log a row per rule.
        let mut policy_results = Vec::new();
        let mut all_policies_passed = true;
        let actor_for_log = if auth.user_id.is_nil() {
            None
        } else {
            Some(auth.user_id)
        };

        for (policy_id, name, condition, action, severity) in &policies {
            let EvalOutcome { passed, details } =
                evaluate_condition(condition, &all_tool_calls, &all_files);
            let result_str = if !passed {
                "fail"
            } else if details.starts_with("Rule skipped") {
                "skip"
            } else {
                "pass"
            };

            if !passed {
                all_policies_passed = false;
            }

            if let Err(e) = PolicyRepo::insert_evaluation(
                &state.pool,
                auth.org_id,
                repo_id,
                *policy_id,
                name,
                None,
                Some(commit_sha),
                result_str,
                action,
                &details,
                "ci_verify",
                actor_for_log,
            )
            .await
            {
                tracing::warn!(
                    policy_id = %policy_id,
                    error = %e,
                    "failed to record policy evaluation (ci_verify)"
                );
            }

            policy_results.push(PolicyResult {
                rule_name: name.clone(),
                result: result_str.into(),
                action: action.clone(),
                severity: severity.clone(),
                details,
            });
        }

        let commit_status = if signature_valid && chain_valid && all_policies_passed {
            policy_passed_count += 1;
            "pass"
        } else {
            "fail"
        };

        results.push(CommitVerifyResult {
            commit_sha: commit_sha.clone(),
            status: commit_status.into(),
            registered: true,
            sealed: true,
            signature_valid,
            chain_valid,
            attributed_sessions_sealed,
            policy_results,
        });
    }

    let overall_status = if results.iter().all(|r| r.status == "pass") {
        "pass"
    } else {
        "fail"
    };

    crate::audit::log(
        &state.pool,
        crate::audit::user_action(
            auth.org_id,
            auth.user_id,
            "ci.verify",
            "repo",
            Some(repo_id),
            Some(serde_json::json!({
                "status": overall_status,
                "total_commits": req.commits.len(),
                "registered": registered_count,
                "sealed": sealed_count,
                "policy_passed": policy_passed_count,
            })),
        ),
    )
    .await;

    Ok(Json(CiVerifyResponse {
        status: overall_status.into(),
        total_commits: req.commits.len(),
        registered_commits: registered_count,
        sealed_commits: sealed_count,
        policy_passed_commits: policy_passed_count,
        results,
    }))
}
