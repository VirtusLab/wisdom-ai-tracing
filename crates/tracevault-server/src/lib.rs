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
mod proxy_url;
pub mod repo;
pub mod repo_manager;
pub mod service;
pub mod signing;
pub mod story;

pub use error::AppError;
pub use proxy_url::validate_base_url;

/// Stable replacement for `str::floor_char_boundary` (nightly-only).
/// Returns the largest byte index `<= index` that is a char boundary.
pub fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        s.len()
    } else {
        let mut i = index;
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub repo_manager: repo_manager::RepoManager,
    pub extensions: extensions::ExtensionRegistry,
    pub encryption_key: Option<String>,
    /// General-purpose HTTP client (pricing sync, future short-lived
    /// outbound calls). Built with reqwest defaults — no per-request
    /// timeout, suitable for one-shot non-streaming calls.
    pub http_client: reqwest::Client,
    /// HTTP client dedicated to the Anthropic proxy. Has a bounded
    /// `connect_timeout` so a stalled TCP handshake on api.anthropic.com
    /// cannot park the proxy task indefinitely; intentionally has no
    /// overall `timeout()` because the proxy carries long-lived SSE
    /// streams whose total duration depends on the model's output.
    pub proxy_http_client: reqwest::Client,
    pub cors_origin: String,
    pub invite_expiry_minutes: u64,
    pub embedding_service:
        Option<std::sync::Arc<crate::service::chat_embeddings::EmbeddingService>>,
    /// Default upstream base URL applied to newly-stored credentials when the
    /// caller does not supply one. Defaults to `https://api.anthropic.com` in
    /// production; overridden in tests so a wiremock stub upstream can stand in
    /// for the real Anthropic API. The proxy forwards to each credential's own
    /// stored `base_url`, not this field.
    pub default_credential_base_url: String,
    /// Optional global cap on in-flight proxy requests across all users.
    /// `None` = unlimited (default); set the operator env var
    /// `PROXY_MAX_GLOBAL_CONCURRENT` to enable.
    pub proxy_global_semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>>,
    /// Per-credential concurrency semaphores. Keyed by `credentials.id` so a
    /// user with multiple credentials (per-model routing) gets an independent
    /// cap per credential rather than one shared cap. Each semaphore is
    /// lazily created on first request for a credential, sized to the
    /// credential's stored `max_concurrent` at that moment.
    ///
    /// Update semantics are intentionally lazy: a PUT that changes
    /// `max_concurrent` only updates the DB row, *not* the in-memory
    /// semaphore. The new cap takes effect on the next process restart, or
    /// after the entry is explicitly evicted. This avoids the atomic-swap
    /// edge cases of mid-flight cap changes.
    ///
    /// Growth: this DashMap grows monotonically with credentials that have
    /// received at least one proxy request since startup. At expected scale
    /// (<= ~10k credentials) the footprint is a few MB. Revisit eviction
    /// (TTL or LRU) if active credentials exceed that threshold.
    pub proxy_per_credential_semaphores:
        std::sync::Arc<dashmap::DashMap<uuid::Uuid, std::sync::Arc<tokio::sync::Semaphore>>>,
}
