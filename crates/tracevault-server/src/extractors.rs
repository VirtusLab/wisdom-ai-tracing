use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use uuid::Uuid;

use crate::auth::sha256_hex;
use crate::AppState;

/// Authenticated user — no org context. For org-agnostic endpoints.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header",
            ))?;

        let token_hash = sha256_hex(header);

        // Try auth_sessions first
        let session_row = sqlx::query_as::<_, (Uuid,)>(
            "SELECT user_id FROM auth_sessions WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        if let Some((user_id,)) = session_row {
            return Ok(AuthUser { user_id });
        }

        // Fall back to api_keys — return nil user_id
        let api_key_exists =
            sqlx::query_as::<_, (Uuid,)>("SELECT org_id FROM api_keys WHERE key_hash = $1")
                .bind(&token_hash)
                .fetch_optional(&state.pool)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        if api_key_exists.is_some() {
            return Ok(AuthUser {
                user_id: Uuid::nil(),
            });
        }

        Err((StatusCode::UNAUTHORIZED, "Invalid or expired token"))
    }
}

/// Authenticated user with org context. For org-scoped endpoints.
/// Extracts org from `{slug}` URL parameter, verifies membership.
#[derive(Debug, Clone)]
pub struct OrgAuth {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: String,
}

impl FromRequestParts<AppState> for OrgAuth {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid Authorization header".to_string(),
            ))?;

        let token_hash = sha256_hex(header);

        // Extract slug from URL path: /api/v1/orgs/{slug}/...
        // Parse from raw URI to avoid consuming Path (which handlers need)
        let slug = parts
            .uri
            .path()
            .strip_prefix("/api/v1/orgs/")
            .and_then(|rest| rest.split('/').next())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .ok_or((
                StatusCode::BAD_REQUEST,
                "Missing org slug in URL".to_string(),
            ))?;

        // Resolve org (case-insensitive — GitHub org names are case-insensitive,
        // and stored slugs may preserve mixed case).
        let org_row =
            sqlx::query_as::<_, (Uuid,)>("SELECT id FROM orgs WHERE LOWER(name) = LOWER($1)")
                .bind(&slug)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .ok_or((
                    StatusCode::NOT_FOUND,
                    format!("Organization '{slug}' not found"),
                ))?;

        let org_id = org_row.0;

        // Try session auth
        let session_row = sqlx::query_as::<_, (Uuid,)>(
            "SELECT user_id FROM auth_sessions WHERE token_hash = $1 AND expires_at > NOW()",
        )
        .bind(&token_hash)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some((user_id,)) = session_row {
            // Check membership
            let membership = sqlx::query_as::<_, (String,)>(
                "SELECT role FROM user_org_memberships WHERE user_id = $1 AND org_id = $2",
            )
            .bind(user_id)
            .bind(org_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or((
                StatusCode::FORBIDDEN,
                format!("You are not a member of org '{slug}'"),
            ))?;

            return Ok(OrgAuth {
                user_id,
                org_id,
                role: membership.0,
            });
        }

        // Try API key auth
        let api_key_row =
            sqlx::query_as::<_, (Uuid,)>("SELECT org_id FROM api_keys WHERE key_hash = $1")
                .bind(&token_hash)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if let Some((key_org_id,)) = api_key_row {
            if key_org_id != org_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    "API key does not belong to this org".to_string(),
                ));
            }
            return Ok(OrgAuth {
                user_id: Uuid::nil(),
                org_id,
                role: "admin".to_string(),
            });
        }

        Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or expired token".to_string(),
        ))
    }
}
