pub mod api;
pub mod audit;
pub mod auth;
pub mod branch_tracking;
pub mod config;
pub mod db;
pub mod encryption;
pub mod error;
pub mod extensions;
pub mod extractors;
pub mod llm;
pub mod org_signing;
pub mod password_policy;
pub mod permissions;
pub mod pricing;
pub mod pricing_sync;
pub mod repo;
pub mod repo_manager;
pub mod service;
pub mod signing;
pub mod story;

pub use error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub repo_manager: repo_manager::RepoManager,
    pub extensions: extensions::ExtensionRegistry,
    pub encryption_key: Option<String>,
    pub http_client: reqwest::Client,
    pub cors_origin: String,
    pub invite_expiry_minutes: u64,
    pub embedding_service:
        Option<std::sync::Arc<crate::service::chat_embeddings::EmbeddingService>>,
}
