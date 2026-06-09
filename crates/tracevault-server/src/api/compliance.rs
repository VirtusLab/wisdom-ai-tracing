use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit;
use crate::error::{self, AppError};
use crate::extractors::OrgAuth;
use crate::permissions::Permission;
use crate::repo::compliance::ComplianceRepo;
use crate::AppState;

// --- Compliance Settings ---

#[derive(Debug, Serialize)]
pub struct ComplianceSettingsResponse {
    pub org_id: Uuid,
    pub retention_days: i32,
    pub signing_enabled: bool,
    pub chain_verification_interval_hours: Option<i32>,
    pub compliance_mode: Option<String>,
    pub usage_source: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_compliance_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<ComplianceSettingsResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::ComplianceView)?;

    let org_id = auth.org_id;

    let row = ComplianceRepo::get_settings(&state.pool, org_id).await?;

    if let Some(r) = row {
        Ok(Json(ComplianceSettingsResponse {
            org_id: r.org_id,
            retention_days: r.retention_days,
            signing_enabled: r.signing_enabled,
            chain_verification_interval_hours: r.chain_verification_interval_hours,
            compliance_mode: r.compliance_mode,
            usage_source: r.usage_source,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }))
    } else {
        Ok(Json(ComplianceSettingsResponse {
            org_id,
            retention_days: 365,
            signing_enabled: false,
            chain_verification_interval_hours: Some(24),
            compliance_mode: None,
            usage_source: "both".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }))
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateComplianceSettingsRequest {
    pub retention_days: Option<i32>,
    pub signing_enabled: Option<bool>,
    pub chain_verification_interval_hours: Option<i32>,
    pub compliance_mode: Option<String>,
    pub usage_source: Option<String>,
}

pub async fn update_compliance_settings(
    State(state): State<AppState>,
    auth: OrgAuth,
    Json(req): Json<UpdateComplianceSettingsRequest>,
) -> Result<Json<ComplianceSettingsResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::ComplianceManage)?;

    let org_id = auth.org_id;

    let valid_modes = ["none", "sox", "pci_dss", "sr_11_7", "custom"];
    if let Some(mode) = &req.compliance_mode {
        if !valid_modes.contains(&mode.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid compliance mode. Must be one of: {}",
                valid_modes.join(", ")
            )));
        }
    }

    if let Some(ref s) = req.usage_source {
        if !matches!(s.as_str(), "hook" | "proxy" | "both") {
            return Err(AppError::BadRequest(
                "usage_source must be hook, proxy, or both".into(),
            ));
        }
    }

    let mode = req.compliance_mode.as_deref().unwrap_or("none");
    let min_retention = match mode {
        "sox" => 2555,
        "pci_dss" => 365,
        "sr_11_7" => 1095,
        _ => 0,
    };
    if let Some(days) = req.retention_days {
        if days < min_retention {
            return Err(AppError::BadRequest(format!(
                "Compliance mode '{}' requires minimum {} days retention",
                mode, min_retention
            )));
        }
    }

    let r = ComplianceRepo::upsert_settings(
        &state.pool,
        org_id,
        req.retention_days.unwrap_or(365),
        req.signing_enabled.unwrap_or(false),
        req.chain_verification_interval_hours,
        mode,
        req.usage_source.as_deref(),
    )
    .await?;

    audit::log(
        &state.pool,
        audit::user_action(
            auth.org_id,
            auth.user_id,
            "org.compliance.update",
            "org",
            Some(org_id),
            Some(
                serde_json::json!({"compliance_mode": mode, "retention_days": req.retention_days}),
            ),
        ),
    )
    .await;

    Ok(Json(ComplianceSettingsResponse {
        org_id: r.org_id,
        retention_days: r.retention_days,
        signing_enabled: r.signing_enabled,
        chain_verification_interval_hours: r.chain_verification_interval_hours,
        compliance_mode: r.compliance_mode,
        usage_source: r.usage_source,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }))
}

// --- Public Key ---

#[derive(Serialize)]
pub struct PublicKeyResponse {
    pub algorithm: String,
    pub public_key: String,
}

pub async fn get_public_key(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<PublicKeyResponse>, AppError> {
    let encryption_key = state
        .encryption_key
        .as_deref()
        .ok_or_else(|| AppError::Internal("Encryption not configured".into()))?;

    let svc = crate::org_signing::load_current(&state.pool, auth.org_id, encryption_key)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::NotFound("No signing key configured for this org".into()))?;

    Ok(Json(PublicKeyResponse {
        algorithm: "Ed25519".into(),
        public_key: svc.public_key_b64(),
    }))
}

// --- Chain Verification ---

#[derive(Serialize)]
pub struct ChainStatusResponse {
    pub status: String,
    pub total_commits: i32,
    pub verified_commits: i32,
    pub total_sessions: i32,
    pub verified_sessions: i32,
    pub errors: Option<serde_json::Value>,
    pub last_verified_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_chain_status(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<ChainStatusResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::ComplianceView)?;

    let org_id = auth.org_id;

    let row = ComplianceRepo::get_chain_status(&state.pool, org_id).await?;

    if let Some(r) = row {
        Ok(Json(ChainStatusResponse {
            status: r.0,
            total_commits: r.1,
            verified_commits: r.2,
            total_sessions: r.3,
            verified_sessions: r.4,
            errors: r.5,
            last_verified_at: r.6,
        }))
    } else {
        Ok(Json(ChainStatusResponse {
            status: "never_run".into(),
            total_commits: 0,
            verified_commits: 0,
            total_sessions: 0,
            verified_sessions: 0,
            errors: None,
            last_verified_at: None,
        }))
    }
}

pub async fn verify_chain(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<ChainStatusResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::ComplianceView)?;

    let org_id = auth.org_id;

    let encryption_key = state
        .encryption_key
        .as_deref()
        .ok_or_else(|| AppError::Internal("Encryption not configured".into()))?;

    let commits = ComplianceRepo::get_sealed_commits_for_verification(&state.pool, org_id).await?;

    let total = commits.len() as i32;
    let mut verified = 0i32;
    let mut errors = Vec::new();
    let mut prev_hash: Option<String> = None;

    for (id, record_hash, chain_hash, prev_chain_hash, signature, sealed_at) in &commits {
        let (Some(rh), Some(ch), Some(sig)) = (record_hash, chain_hash, signature) else {
            errors.push(serde_json::json!({"commit_id": id, "error": "missing integrity fields"}));
            prev_hash = chain_hash.clone();
            continue;
        };

        let svc = crate::org_signing::load_at_time(&state.pool, org_id, sealed_at, encryption_key)
            .await
            .map_err(AppError::internal)?;

        let Some(svc) = svc else {
            errors.push(serde_json::json!({"commit_id": id, "error": "no signing key found for sealed_at time"}));
            prev_hash = chain_hash.clone();
            continue;
        };

        let expected_chain = svc.chain_hash(prev_chain_hash.as_deref(), rh);
        if &expected_chain != ch {
            errors.push(serde_json::json!({"commit_id": id, "error": "chain_hash mismatch"}));
            prev_hash = Some(ch.clone());
            continue;
        }

        if prev_hash.as_deref() != prev_chain_hash.as_deref() {
            errors.push(serde_json::json!({"commit_id": id, "error": "prev_chain_hash mismatch"}));
        }

        if !svc.verify(rh, sig) {
            errors.push(
                serde_json::json!({"commit_id": id, "error": "signature verification failed"}),
            );
            prev_hash = Some(ch.clone());
            continue;
        }

        verified += 1;
        prev_hash = Some(ch.clone());
    }

    // Verify session seals
    let session_seals =
        crate::repo::sealing::SealingRepo::get_session_seals_for_verification(&state.pool, org_id)
            .await?;

    let total_sessions = session_seals.len() as i32;
    let mut verified_sessions = 0i32;

    for (seal_id, session_db_id, record_hash, signature, _seal_type, sealed_at) in &session_seals {
        let svc = crate::org_signing::load_at_time(&state.pool, org_id, sealed_at, encryption_key)
            .await
            .map_err(AppError::internal)?;

        let Some(svc) = svc else {
            errors.push(
                serde_json::json!({"session_seal_id": seal_id, "error": "no signing key found"}),
            );
            continue;
        };

        if let Some(sig) = signature {
            if svc.verify(record_hash, sig) {
                verified_sessions += 1;
            } else {
                errors.push(serde_json::json!({"session_seal_id": seal_id, "session_id": session_db_id, "error": "signature verification failed"}));
            }
        } else {
            errors.push(
                serde_json::json!({"session_seal_id": seal_id, "error": "missing signature"}),
            );
        }
    }

    let status = if errors.is_empty() { "pass" } else { "fail" };
    let errors_json = if errors.is_empty() {
        None
    } else {
        Some(serde_json::json!(errors))
    };

    ComplianceRepo::insert_chain_verification(
        &state.pool,
        org_id,
        status,
        total,
        verified,
        total_sessions,
        verified_sessions,
        &errors_json,
    )
    .await?;

    audit::log(
        &state.pool,
        audit::user_action(
            auth.org_id,
            auth.user_id,
            "chain.verify",
            "org",
            Some(org_id),
            Some(serde_json::json!({"status": status, "total": total, "verified": verified, "total_sessions": total_sessions, "verified_sessions": verified_sessions})),
        ),
    )
    .await;

    Ok(Json(ChainStatusResponse {
        status: status.into(),
        total_commits: total,
        verified_commits: verified,
        total_sessions,
        verified_sessions,
        errors: errors_json,
        last_verified_at: Some(chrono::Utc::now()),
    }))
}

// --- Audit Log ---

pub async fn audit_log_actions(
    State(state): State<AppState>,
    auth: OrgAuth,
) -> Result<Json<Vec<String>>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::AuditLogView)?;
    let actions = ComplianceRepo::distinct_audit_actions(&state.pool, auth.org_id).await?;
    Ok(Json(actions))
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub action: Option<String>,
    pub actor_id: Option<Uuid>,
    pub resource_type: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Serialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Uuid>,
    pub details: Option<serde_json::Value>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct AuditLogResponse {
    pub entries: Vec<AuditLogEntry>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

pub async fn list_audit_log(
    State(state): State<AppState>,
    auth: OrgAuth,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<AuditLogResponse>, AppError> {
    error::require_permission(&state.extensions, &auth.role, Permission::AuditLogView)?;

    let org_id = auth.org_id;

    let per_page = query.per_page.unwrap_or(50).min(200);
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * per_page;

    let total = ComplianceRepo::count_audit_log(
        &state.pool,
        org_id,
        &query.action,
        query.actor_id,
        &query.resource_type,
        query.from,
        query.to,
    )
    .await?;

    let rows = ComplianceRepo::list_audit_log(
        &state.pool,
        org_id,
        &query.action,
        query.actor_id,
        &query.resource_type,
        query.from,
        query.to,
        per_page,
        offset,
    )
    .await?;

    let entries = rows
        .into_iter()
        .map(|r| AuditLogEntry {
            id: r.0,
            actor_id: r.1,
            action: r.2,
            resource_type: r.3,
            resource_id: r.4,
            details: r.5,
            ip_address: r.6,
            user_agent: r.7,
            created_at: r.8,
        })
        .collect();

    Ok(Json(AuditLogResponse {
        entries,
        total,
        page,
        per_page,
    }))
}

// --- Trace Verification ---

#[derive(Serialize)]
pub struct TraceVerifyResponse {
    pub commit_id: Uuid,
    pub record_hash_valid: bool,
    pub signature_valid: bool,
    pub chain_valid: bool,
    pub sealed_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn verify_trace(
    State(state): State<AppState>,
    auth: OrgAuth,
    Path((_slug, sha)): Path<(String, String)>,
) -> Result<Json<TraceVerifyResponse>, AppError> {
    let commit = ComplianceRepo::get_commit_seal_by_sha(&state.pool, &sha, auth.org_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Commit not found".into()))?;

    let (commit_id, record_hash, chain_hash, prev_chain_hash, signature, sealed_at) = commit;

    let svc = if let Some(ref sat) = sealed_at {
        let encryption_key = state
            .encryption_key
            .as_deref()
            .ok_or_else(|| AppError::Internal("Encryption not configured".into()))?;
        crate::org_signing::load_at_time(&state.pool, auth.org_id, sat, encryption_key)
            .await
            .map_err(AppError::internal)?
    } else {
        None
    };

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

    Ok(Json(TraceVerifyResponse {
        commit_id,
        record_hash_valid: record_hash.is_some(),
        signature_valid,
        chain_valid,
        sealed_at,
    }))
}
