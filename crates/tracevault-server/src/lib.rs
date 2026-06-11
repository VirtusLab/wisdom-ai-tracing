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
pub mod plugins;
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

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
    Router,
};
use http::Method;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Assemble the full HTTP router from `state`. Route groups, layers, and
/// merge/layer ordering are identical to the original `main()` assembly;
/// the only additions are the RoutePlugin routers and the capabilities
/// endpoint, both additive (they do not alter existing routes).
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .cors_origin
                .parse::<http::HeaderValue>()
                .expect("CORS_ORIGIN must be a valid header value"),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]);

    let auth_rate_limit = GovernorConfigBuilder::default()
        .per_second(6)
        .burst_size(10)
        .finish()
        .expect("Failed to build auth rate limiter");

    let public_rate_limit = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(60)
        .finish()
        .expect("Failed to build public rate limiter");

    // Auth routes (strict: 10 req/min per IP)
    let auth_routes = Router::new()
        .route("/api/v1/auth/register", post(crate::api::auth::register))
        .route("/api/v1/auth/login", post(crate::api::auth::login))
        .route("/api/v1/auth/device", post(crate::api::auth::device_start))
        .layer(GovernorLayer::new(auth_rate_limit));

    // Public routes (moderate: 60 req/min per IP)
    let public_routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/v1/features", get(crate::api::features::get_features))
        .route(
            "/api/v1/auth/device/{token}/status",
            get(crate::api::auth::device_status),
        )
        .route(
            "/api/v1/orgs/public",
            get(crate::api::auth::list_public_orgs),
        )
        .route(
            "/api/v1/invitation-requests",
            post(crate::api::auth::request_invitation),
        )
        .route("/api/v1/github/webhook", post(crate::api::github::webhook))
        .route(
            "/api/v1/auth/sso-status/{slug}",
            get(crate::api::sso::sso_status),
        )
        .route(
            "/api/v1/auth/sso/{slug}",
            get(crate::api::sso::sso_initiate),
        )
        .route(
            "/api/v1/auth/sso/{slug}/callback",
            get(crate::api::sso::sso_callback),
        )
        .route(
            "/api/v1/invite/{token}",
            get(crate::api::invites::get_invite_details),
        )
        .route(
            "/api/v1/invite/{token}/accept",
            post(crate::api::invites::accept_invite_new_user),
        )
        .layer(GovernorLayer::new(public_rate_limit));

    // Authenticated routes (no rate limiting)
    let authenticated_routes = Router::new()
        .route(
            "/api/v1/auth/device/{token}/approve",
            post(crate::api::auth::device_approve),
        )
        .route("/api/v1/auth/logout", post(crate::api::auth::logout))
        .route("/api/v1/auth/me", get(crate::api::auth::me))
        // User endpoints
        .route("/api/v1/me/orgs", get(crate::api::auth::list_my_orgs))
        .route(
            "/api/v1/me/anthropic-key",
            get(crate::api::me::get_anthropic_key_status)
                .put(crate::api::me::put_anthropic_key)
                .delete(crate::api::me::delete_anthropic_key),
        )
        .route(
            "/api/v1/me/credentials",
            get(crate::api::me::list_credentials),
        )
        .route(
            "/api/v1/me/credentials/{name}",
            put(crate::api::me::put_credential).delete(crate::api::me::delete_credential),
        )
        .route(
            "/api/v1/me/proxy-routing",
            get(crate::api::me::list_routing_rules).put(crate::api::me::put_routing_rule),
        )
        .route(
            "/api/v1/me/proxy-routing/{id}",
            delete(crate::api::me::delete_routing_rule),
        )
        // Org management (create is org-agnostic)
        .route("/api/v1/orgs", post(crate::api::orgs::create_org))
        // Org-scoped: org details & members
        .route(
            "/api/v1/orgs/{slug}",
            get(crate::api::orgs::get_org).put(crate::api::orgs::update_org),
        )
        .route(
            "/api/v1/orgs/{slug}/members",
            get(crate::api::orgs::list_members),
        )
        .route(
            "/api/v1/orgs/{slug}/members/{user_id}",
            delete(crate::api::orgs::remove_member),
        )
        .route(
            "/api/v1/orgs/{slug}/members/{user_id}/role",
            put(crate::api::orgs::change_role),
        )
        // Invitation requests (admin)
        .route(
            "/api/v1/orgs/{slug}/invitation-requests",
            get(crate::api::orgs::list_invitation_requests),
        )
        .route(
            "/api/v1/orgs/{slug}/invitation-requests/{id}/approve",
            post(crate::api::orgs::approve_invitation_request),
        )
        .route(
            "/api/v1/orgs/{slug}/invitation-requests/{id}/reject",
            post(crate::api::orgs::reject_invitation_request),
        )
        // Org-scoped: invites
        .route(
            "/api/v1/orgs/{slug}/invites",
            get(crate::api::invites::list_invites).post(crate::api::invites::create_invite),
        )
        .route(
            "/api/v1/orgs/{slug}/invites/{invite_id}",
            delete(crate::api::invites::revoke_invite),
        )
        // Accept invite for existing authenticated users
        .route(
            "/api/v1/invite/{token}/accept/existing",
            post(crate::api::invites::accept_invite_existing_user),
        )
        .route(
            "/api/v1/orgs/{slug}/llm-settings",
            get(crate::api::orgs::get_llm_settings).put(crate::api::orgs::update_llm_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/chat-settings",
            get(crate::api::orgs::get_chat_settings).put(crate::api::orgs::update_chat_settings),
        )
        // Org-scoped: SSO
        .route(
            "/api/v1/orgs/{slug}/sso",
            get(crate::api::sso::get_sso_config)
                .put(crate::api::sso::upsert_sso_config)
                .delete(crate::api::sso::delete_sso_config),
        )
        // Org-scoped: repos
        .route(
            "/api/v1/orgs/{slug}/repos",
            get(crate::api::repos::list_repos).post(crate::api::repos::register_repo),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}",
            get(crate::api::repos::get_repo).delete(crate::api::repos::delete_repo),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}/settings",
            get(crate::api::repos::get_settings).put(crate::api::repos::update_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}/sync",
            post(crate::api::repos::sync_repo),
        )
        // Org-scoped: code browser
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/branches",
            get(crate::api::code::list_branches),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/tree",
            get(crate::api::code::get_tree),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/blob",
            get(crate::api::code::get_blob),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/blame",
            get(crate::api::code::get_blame),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/commits",
            get(crate::api::code::list_file_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/info",
            get(crate::api::code::get_ref_info),
        )
        // Org-scoped: story
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/story",
            post(crate::api::code::generate_story),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/sessions",
            get(crate::api::code::get_function_sessions),
        )
        // Org-scoped: traces
        .route(
            "/api/v1/orgs/{slug}/traces/stats",
            get(crate::api::traces_ui::get_stats),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions",
            get(crate::api::traces_ui::list_sessions),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/filter-options",
            get(crate::api::traces_ui::get_session_filter_options),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}",
            get(crate::api::traces_ui::get_session),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/events",
            get(crate::api::traces_ui::get_session_events),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/file-changes",
            get(crate::api::traces_ui::get_session_file_changes),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/transcript",
            get(crate::api::traces_ui::get_session_transcript),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/linked-commits",
            get(crate::api::traces_ui::get_session_linked_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits",
            get(crate::api::traces_ui::list_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits/{id}",
            get(crate::api::traces_ui::get_commit),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits/{id}/verify",
            get(crate::api::compliance::verify_trace),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/timeline",
            get(crate::api::traces_ui::get_timeline),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/attribution/{commit_id}/{*file_path}",
            get(crate::api::traces_ui::get_attribution),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/branches",
            get(crate::api::traces_ui::get_branches),
        )
        // Org-scoped: api keys
        .route(
            "/api/v1/orgs/{slug}/api-keys",
            post(crate::api::api_keys::create_api_key).get(crate::api::api_keys::list_api_keys),
        )
        .route(
            "/api/v1/orgs/{slug}/api-keys/{id}",
            delete(crate::api::api_keys::delete_api_key),
        )
        // Org-scoped: policies
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies",
            get(crate::api::policies::list_repo_policies)
                .post(crate::api::policies::create_repo_policy),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies/check",
            post(crate::api::policies::check_policies),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies/agent-instructions",
            get(crate::api::agent_instructions::get_agent_instructions),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policy-evaluations",
            get(crate::api::policies::list_policy_evaluations),
        )
        .route(
            "/api/v1/orgs/{slug}/policies/{id}",
            put(crate::api::policies::update_policy).delete(crate::api::policies::delete_policy),
        )
        // Org-scoped: compliance
        .route(
            "/api/v1/orgs/{slug}/compliance",
            get(crate::api::compliance::get_compliance_settings)
                .put(crate::api::compliance::update_compliance_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/public-key",
            get(crate::api::compliance::get_public_key),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/verify-chain",
            post(crate::api::compliance::verify_chain),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/chain-status",
            get(crate::api::compliance::get_chain_status),
        )
        .route(
            "/api/v1/orgs/{slug}/audit-log",
            get(crate::api::compliance::list_audit_log),
        )
        .route(
            "/api/v1/orgs/{slug}/audit-log/actions",
            get(crate::api::compliance::audit_log_actions),
        )
        // Org-scoped: pricing
        .route(
            "/api/v1/orgs/{slug}/pricing",
            get(crate::api::pricing::list_pricing).post(crate::api::pricing::create_pricing),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/models",
            get(crate::api::pricing::list_models),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/sync",
            post(crate::api::pricing::trigger_sync),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/sync/status",
            get(crate::api::pricing::sync_status),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/{id}",
            put(crate::api::pricing::update_pricing),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/{id}/recalculate",
            post(crate::api::pricing::recalculate),
        )
        // Org-scoped: streaming
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/stream",
            post(crate::api::stream::handle_stream),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/commits",
            post(crate::api::commit_push::handle_commit_push),
        )
        // Org-scoped: dashboard
        .route(
            "/api/v1/orgs/{slug}/dashboard",
            get(crate::api::dashboard::get_dashboard),
        )
        // Org-scoped: analytics
        .route(
            "/api/v1/orgs/{slug}/analytics/filters",
            get(crate::api::analytics::get_filters),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/overview",
            get(crate::api::analytics::get_overview),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/tokens",
            get(crate::api::analytics::get_tokens),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/models",
            get(crate::api::analytics::get_models),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/authors",
            get(crate::api::analytics::get_authors),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/authors/{user_id}",
            get(crate::api::analytics::get_author_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/attribution",
            get(crate::api::analytics::get_attribution),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/sessions",
            get(crate::api::analytics::get_sessions),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/sessions/{id}/detail",
            get(crate::api::session_detail::get_session_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/cost",
            get(crate::api::analytics::get_cost),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/software",
            get(crate::api::analytics::get_software),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/software/users/{user_id}",
            get(crate::api::analytics::get_software_user_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/ai-tools",
            get(crate::api::analytics::get_ai_tools),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/ai-tools/users/{user_id}",
            get(crate::api::analytics::get_ai_tools_user_detail),
        )
        // Org-scoped: chat (enterprise)
        .route(
            "/api/v1/orgs/{slug}/chat/mentions",
            get(crate::api::chat::list_mentions),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations",
            get(crate::api::chat::list_conversations).post(crate::api::chat::create_conversation),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations/{id}",
            get(crate::api::chat::get_conversation)
                .patch(crate::api::chat::rename_conversation)
                .delete(crate::api::chat::delete_conversation),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations/{id}/messages",
            post(crate::api::chat::send_message),
        )
        .route("/api/v1/orgs/{slug}/chat/ask", post(crate::api::chat::ask))
        .route(
            "/api/v1/orgs/{slug}/chat/indexing/status",
            get(crate::api::chat_indexing::get_indexing_status),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/indexing/backfill",
            post(crate::api::chat_indexing::trigger_backfill),
        )
        // Org-scoped: CI
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/ci/verify",
            post(crate::api::ci::verify_commits),
        );

    // Anthropic LLM proxy — authenticates via x-api-key inside the handler
    // (not the standard Authorization-bearer extractor), so it is its own
    // router with no rate-limiting layer. Issue #207 / parent #181.
    //
    // Body limit: Axum's default `Bytes` cap is 2 MB, which silently rejects
    // legitimate Anthropic requests (vision inputs, long conversations,
    // large `system` prompts). Raise to 32 MB to match Anthropic's own
    // request size envelope while still bounding worst-case server memory
    // per in-flight request.
    let proxy_routes = Router::new()
        .route(
            "/proxy/anthropic/{*path}",
            get(crate::api::proxy::anthropic_proxy)
                .post(crate::api::proxy::anthropic_proxy)
                .put(crate::api::proxy::anthropic_proxy)
                .delete(crate::api::proxy::anthropic_proxy),
        )
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024));

    let mut router = Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(authenticated_routes)
        .merge(proxy_routes);
    for p in &state.plugins.routes {
        router = router.merge(p.router());
    }
    router
        .route("/api/v1/capabilities", get(capabilities_handler))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

#[derive(serde::Serialize)]
struct CapabilitiesResponse {
    capabilities: Vec<String>,
}

async fn capabilities_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<CapabilitiesResponse> {
    axum::Json(CapabilitiesResponse {
        capabilities: state.plugins.capabilities.iter().cloned().collect(),
    })
}

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
    pub plugins: std::sync::Arc<crate::plugins::Plugins>,
}
