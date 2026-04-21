use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Conflict ({code}): {message}")]
    ConflictCode { code: &'static str, message: String },

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),

    #[error(transparent)]
    Git(#[from] git2::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, code) = match &self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone(), None),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone(), None),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone(), None),
            AppError::ConflictCode { code, message } => {
                (StatusCode::CONFLICT, message.clone(), Some(*code))
            }
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone(), None),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".into(), None),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone(), None),
            AppError::Sqlx(e) => {
                tracing::error!("Database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                    None,
                )
            }
            AppError::Git(e) => {
                tracing::error!("Git error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                    None,
                )
            }
        };
        let body = match code {
            Some(code) => json!({ "error": message, "code": code }),
            None => json!({ "error": message }),
        };
        (status, Json(body)).into_response()
    }
}

impl AppError {
    pub fn internal(msg: impl std::fmt::Display) -> Self {
        AppError::Internal(msg.to_string())
    }
}

use crate::extensions::ExtensionRegistry;
use crate::permissions::Permission;

pub fn require_permission(
    extensions: &ExtensionRegistry,
    role: &str,
    perm: Permission,
) -> Result<(), AppError> {
    if !extensions.permissions.has_permission(role, perm) {
        return Err(AppError::Forbidden(format!(
            "Missing permission: {:?}",
            perm
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_is_404() {
        let resp = AppError::NotFound("x".into()).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn forbidden_is_403() {
        let resp = AppError::Forbidden("x".into()).into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn bad_request_is_400() {
        let resp = AppError::BadRequest("x".into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn unauthorized_is_401() {
        let resp = AppError::Unauthorized.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn conflict_is_409() {
        let resp = AppError::Conflict("x".into()).into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn conflict_code_emits_code_in_body() {
        let err = AppError::ConflictCode {
            code: "account_exists",
            message: "m".into(),
        };
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let body = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        });
        assert!(body.contains("\"code\":\"account_exists\""));
        assert!(body.contains("\"error\":\"m\""));
    }

    #[test]
    fn internal_is_500() {
        let resp = AppError::Internal("x".into()).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn sqlx_error_does_not_leak_details() {
        let err = AppError::Sqlx(sqlx::Error::ColumnNotFound("password_hash".into()));
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        });
        assert!(!body.contains("password_hash"));
        assert!(body.contains("Internal server error"));
    }

    #[test]
    fn git_error_does_not_leak_details() {
        let err = AppError::Git(git2::Error::from_str("path /secret/repo not found"));
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
            String::from_utf8(bytes.to_vec()).unwrap()
        });
        assert!(!body.contains("/secret/repo"));
        assert!(body.contains("Internal server error"));
    }
}
