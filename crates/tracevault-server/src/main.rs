use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
    Router,
};
use http::Method;
use std::net::SocketAddr;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use tracevault_server::{api, config, db, extensions, pricing_sync, repo_manager, AppState};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::ServerConfig::from_env();
    let pool = db::create_pool(&cfg.database_url)
        .await
        .expect("Failed to connect to database");

    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    let cors = CorsLayer::new()
        .allow_origin(
            cfg.cors_origin
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

    let repo_manager = repo_manager::RepoManager::new(&cfg.repos_dir);
    let extensions = build_extensions(&cfg);
    let http_client = reqwest::Client::new();
    // Dedicated client for the Anthropic proxy (no redirects, bounded connect
    // timeout, no overall timeout for long-lived SSE) — see the function.
    let proxy_http_client = api::proxy::build_proxy_http_client();

    // Optional global concurrency cap across all proxy requests. Unset = no
    // global limit; this is the right default for the small-team deployments
    // we ship to today. Operators turn this on after capacity testing; a
    // sensible starting value is 256.
    let proxy_global_semaphore: Option<std::sync::Arc<tokio::sync::Semaphore>> =
        std::env::var("PROXY_MAX_GLOBAL_CONCURRENT")
            .ok()
            .and_then(|s| match s.parse::<usize>() {
                Ok(n) if n > 0 => Some(n),
                // Set but not a positive integer — warn and ignore, rather
                // than silently treating a garbage value as "no cap".
                _ => {
                    tracing::warn!(
                        value = %s,
                        "PROXY_MAX_GLOBAL_CONCURRENT is set but not a positive integer; ignoring"
                    );
                    None
                }
            })
            .map(|n| {
                tracing::info!(cap = n, "proxy global concurrency cap enabled");
                std::sync::Arc::new(tokio::sync::Semaphore::new(n))
            });
    let proxy_per_credential_semaphores = std::sync::Arc::new(dashmap::DashMap::new());

    // Recover clones orphaned by the previous shutdown and re-clone them; a
    // failure falls through to the normal backoff retry. Spawned so a slow
    // clone doesn't delay startup. Then auto-sync repos that are 'ready'.
    {
        let pool = pool.clone();
        let repo_manager = repo_manager.clone();
        let extensions = extensions.clone();
        tokio::spawn(async move {
            tracevault_server::service::clone_recovery::recover_orphaned_on_startup(
                &pool,
                &repo_manager,
                &extensions,
            )
            .await;
        });
    }
    sync_repos_on_startup(&pool, &repo_manager, &extensions).await;

    // Cost analytics (recalculating stored session costs) is enterprise-only.
    let cost_enabled = extensions.pricing.cost_enabled();

    // Sync pricing from LiteLLM on startup (non-blocking on failure)
    match pricing_sync::sync_pricing(&pool, &http_client, cost_enabled).await {
        Ok(result) => {
            if result.models_updated.is_empty() {
                tracing::info!("Pricing sync: all prices up to date");
            } else {
                tracing::info!("Pricing sync: updated {}", result.models_updated.join(", "));
            }
        }
        Err(e) => tracing::warn!("Pricing sync failed on startup (non-fatal): {e}"),
    }

    // Background daily pricing sync
    {
        let pool = pool.clone();
        let client = http_client.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(86400));
            interval.tick().await; // skip immediate tick (startup sync already ran)
            loop {
                interval.tick().await;
                tracing::info!("Running daily pricing sync...");
                match pricing_sync::sync_pricing(&pool, &client, cost_enabled).await {
                    Ok(result) => {
                        if !result.models_updated.is_empty() {
                            tracing::info!(
                                "Daily pricing sync: updated {}",
                                result.models_updated.join(", ")
                            );
                        }
                    }
                    Err(e) => tracing::warn!("Daily pricing sync failed: {e}"),
                }
            }
        });
    }

    // Background materialized view refresh (every 5 minutes)
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                if let Err(e) =
                    sqlx::query("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_daily_session_stats")
                        .execute(&pool)
                        .await
                {
                    tracing::warn!("Failed to refresh materialized view: {e}");
                }
            }
        });
    }

    // Background stale session sealing (every 5 minutes)
    {
        let pool = pool.clone();
        let encryption_key = cfg.encryption_key.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            interval.tick().await; // skip immediate tick
            loop {
                interval.tick().await;
                tracevault_server::service::sealing::SealingService::sweep_stale_sessions(
                    &pool,
                    encryption_key.as_deref(),
                    30, // inactive for 30 minutes
                )
                .await;
            }
        });
    }

    // Background auto-retry of failed clones (every 5 minutes). Capped, with
    // backoff, so transient failures self-heal without a manual sync.
    {
        let pool = pool.clone();
        let repo_manager = repo_manager.clone();
        let extensions = extensions.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
            interval.tick().await; // skip immediate tick
            loop {
                interval.tick().await;
                tracevault_server::service::clone_recovery::retry_failed_clones(
                    &pool,
                    &repo_manager,
                    &extensions,
                )
                .await;
            }
        });
    }

    // Background cleanup of expired SSO auth requests (every hour)
    {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                let _ = sqlx::query("DELETE FROM sso_auth_requests WHERE expires_at < NOW()")
                    .execute(&pool)
                    .await;
            }
        });
    }

    let embedding_service: Option<
        std::sync::Arc<tracevault_server::service::chat_embeddings::EmbeddingService>,
    > = if extensions.features.chat_search {
        match tracevault_server::service::chat_embeddings::EmbeddingService::new() {
            Ok(svc) => {
                tracing::info!("Chat embedding service initialized");
                Some(std::sync::Arc::new(svc))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize embedding service: {e}");
                None
            }
        }
    } else {
        None
    };

    let bind_addr = cfg.bind_addr();

    // Auth routes (strict: 10 req/min per IP)
    let auth_routes = Router::new()
        .route("/api/v1/auth/register", post(api::auth::register))
        .route("/api/v1/auth/login", post(api::auth::login))
        .route("/api/v1/auth/device", post(api::auth::device_start))
        .layer(GovernorLayer::new(auth_rate_limit));

    // Public routes (moderate: 60 req/min per IP)
    let public_routes = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/v1/features", get(api::features::get_features))
        .route(
            "/api/v1/auth/device/{token}/status",
            get(api::auth::device_status),
        )
        .route("/api/v1/orgs/public", get(api::auth::list_public_orgs))
        .route(
            "/api/v1/invitation-requests",
            post(api::auth::request_invitation),
        )
        .route("/api/v1/github/webhook", post(api::github::webhook))
        .route("/api/v1/auth/sso-status/{slug}", get(api::sso::sso_status))
        .route("/api/v1/auth/sso/{slug}", get(api::sso::sso_initiate))
        .route(
            "/api/v1/auth/sso/{slug}/callback",
            get(api::sso::sso_callback),
        )
        .route(
            "/api/v1/invite/{token}",
            get(api::invites::get_invite_details),
        )
        .route(
            "/api/v1/invite/{token}/accept",
            post(api::invites::accept_invite_new_user),
        )
        .layer(GovernorLayer::new(public_rate_limit));

    // Authenticated routes (no rate limiting)
    let authenticated_routes = Router::new()
        .route(
            "/api/v1/auth/device/{token}/approve",
            post(api::auth::device_approve),
        )
        .route("/api/v1/auth/logout", post(api::auth::logout))
        .route("/api/v1/auth/me", get(api::auth::me))
        // User endpoints
        .route("/api/v1/me/orgs", get(api::auth::list_my_orgs))
        .route(
            "/api/v1/me/anthropic-key",
            get(api::me::get_anthropic_key_status)
                .put(api::me::put_anthropic_key)
                .delete(api::me::delete_anthropic_key),
        )
        .route("/api/v1/me/credentials", get(api::me::list_credentials))
        .route(
            "/api/v1/me/credentials/{name}",
            put(api::me::put_credential).delete(api::me::delete_credential),
        )
        .route(
            "/api/v1/me/proxy-routing",
            get(api::me::list_routing_rules).put(api::me::put_routing_rule),
        )
        .route(
            "/api/v1/me/proxy-routing/{id}",
            delete(api::me::delete_routing_rule),
        )
        // Org management (create is org-agnostic)
        .route("/api/v1/orgs", post(api::orgs::create_org))
        // Org-scoped: org details & members
        .route(
            "/api/v1/orgs/{slug}",
            get(api::orgs::get_org).put(api::orgs::update_org),
        )
        .route("/api/v1/orgs/{slug}/members", get(api::orgs::list_members))
        .route(
            "/api/v1/orgs/{slug}/members/{user_id}",
            delete(api::orgs::remove_member),
        )
        .route(
            "/api/v1/orgs/{slug}/members/{user_id}/role",
            put(api::orgs::change_role),
        )
        // Invitation requests (admin)
        .route(
            "/api/v1/orgs/{slug}/invitation-requests",
            get(api::orgs::list_invitation_requests),
        )
        .route(
            "/api/v1/orgs/{slug}/invitation-requests/{id}/approve",
            post(api::orgs::approve_invitation_request),
        )
        .route(
            "/api/v1/orgs/{slug}/invitation-requests/{id}/reject",
            post(api::orgs::reject_invitation_request),
        )
        // Org-scoped: invites
        .route(
            "/api/v1/orgs/{slug}/invites",
            get(api::invites::list_invites).post(api::invites::create_invite),
        )
        .route(
            "/api/v1/orgs/{slug}/invites/{invite_id}",
            delete(api::invites::revoke_invite),
        )
        // Accept invite for existing authenticated users
        .route(
            "/api/v1/invite/{token}/accept/existing",
            post(api::invites::accept_invite_existing_user),
        )
        .route(
            "/api/v1/orgs/{slug}/llm-settings",
            get(api::orgs::get_llm_settings).put(api::orgs::update_llm_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/chat-settings",
            get(api::orgs::get_chat_settings).put(api::orgs::update_chat_settings),
        )
        // Org-scoped: SSO
        .route(
            "/api/v1/orgs/{slug}/sso",
            get(api::sso::get_sso_config)
                .put(api::sso::upsert_sso_config)
                .delete(api::sso::delete_sso_config),
        )
        // Org-scoped: repos
        .route(
            "/api/v1/orgs/{slug}/repos",
            get(api::repos::list_repos).post(api::repos::register_repo),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}",
            get(api::repos::get_repo).delete(api::repos::delete_repo),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}/settings",
            get(api::repos::get_settings).put(api::repos::update_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{id}/sync",
            post(api::repos::sync_repo),
        )
        // Org-scoped: code browser
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/branches",
            get(api::code::list_branches),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/tree",
            get(api::code::get_tree),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/blob",
            get(api::code::get_blob),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/blame",
            get(api::code::get_blame),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/commits",
            get(api::code::list_file_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/info",
            get(api::code::get_ref_info),
        )
        // Org-scoped: story
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/story",
            post(api::code::generate_story),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/code/sessions",
            get(api::code::get_function_sessions),
        )
        // Org-scoped: traces
        .route(
            "/api/v1/orgs/{slug}/traces/stats",
            get(api::traces_ui::get_stats),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions",
            get(api::traces_ui::list_sessions),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/filter-options",
            get(api::traces_ui::get_session_filter_options),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}",
            get(api::traces_ui::get_session),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/events",
            get(api::traces_ui::get_session_events),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/file-changes",
            get(api::traces_ui::get_session_file_changes),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/transcript",
            get(api::traces_ui::get_session_transcript),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/sessions/{id}/linked-commits",
            get(api::traces_ui::get_session_linked_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits",
            get(api::traces_ui::list_commits),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits/{id}",
            get(api::traces_ui::get_commit),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/commits/{id}/verify",
            get(api::compliance::verify_trace),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/timeline",
            get(api::traces_ui::get_timeline),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/attribution/{commit_id}/{*file_path}",
            get(api::traces_ui::get_attribution),
        )
        .route(
            "/api/v1/orgs/{slug}/traces/branches",
            get(api::traces_ui::get_branches),
        )
        // Org-scoped: api keys
        .route(
            "/api/v1/orgs/{slug}/api-keys",
            post(api::api_keys::create_api_key).get(api::api_keys::list_api_keys),
        )
        .route(
            "/api/v1/orgs/{slug}/api-keys/{id}",
            delete(api::api_keys::delete_api_key),
        )
        // Org-scoped: policies
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies",
            get(api::policies::list_repo_policies).post(api::policies::create_repo_policy),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies/check",
            post(api::policies::check_policies),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policies/agent-instructions",
            get(api::agent_instructions::get_agent_instructions),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/policy-evaluations",
            get(api::policies::list_policy_evaluations),
        )
        .route(
            "/api/v1/orgs/{slug}/policies/{id}",
            put(api::policies::update_policy).delete(api::policies::delete_policy),
        )
        // Org-scoped: compliance
        .route(
            "/api/v1/orgs/{slug}/compliance",
            get(api::compliance::get_compliance_settings)
                .put(api::compliance::update_compliance_settings),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/public-key",
            get(api::compliance::get_public_key),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/verify-chain",
            post(api::compliance::verify_chain),
        )
        .route(
            "/api/v1/orgs/{slug}/compliance/chain-status",
            get(api::compliance::get_chain_status),
        )
        .route(
            "/api/v1/orgs/{slug}/audit-log",
            get(api::compliance::list_audit_log),
        )
        .route(
            "/api/v1/orgs/{slug}/audit-log/actions",
            get(api::compliance::audit_log_actions),
        )
        // Org-scoped: pricing
        .route(
            "/api/v1/orgs/{slug}/pricing",
            get(api::pricing::list_pricing).post(api::pricing::create_pricing),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/models",
            get(api::pricing::list_models),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/sync",
            post(api::pricing::trigger_sync),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/sync/status",
            get(api::pricing::sync_status),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/{id}",
            put(api::pricing::update_pricing),
        )
        .route(
            "/api/v1/orgs/{slug}/pricing/{id}/recalculate",
            post(api::pricing::recalculate),
        )
        // Org-scoped: streaming
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/stream",
            post(api::stream::handle_stream),
        )
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/commits",
            post(api::commit_push::handle_commit_push),
        )
        // Org-scoped: dashboard
        .route(
            "/api/v1/orgs/{slug}/dashboard",
            get(api::dashboard::get_dashboard),
        )
        // Org-scoped: analytics
        .route(
            "/api/v1/orgs/{slug}/analytics/filters",
            get(api::analytics::get_filters),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/overview",
            get(api::analytics::get_overview),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/tokens",
            get(api::analytics::get_tokens),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/models",
            get(api::analytics::get_models),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/authors",
            get(api::analytics::get_authors),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/authors/{user_id}",
            get(api::analytics::get_author_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/attribution",
            get(api::analytics::get_attribution),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/sessions",
            get(api::analytics::get_sessions),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/sessions/{id}/detail",
            get(api::session_detail::get_session_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/cost",
            get(api::analytics::get_cost),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/software",
            get(api::analytics::get_software),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/software/users/{user_id}",
            get(api::analytics::get_software_user_detail),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/ai-tools",
            get(api::analytics::get_ai_tools),
        )
        .route(
            "/api/v1/orgs/{slug}/analytics/ai-tools/users/{user_id}",
            get(api::analytics::get_ai_tools_user_detail),
        )
        // Org-scoped: chat (enterprise)
        .route(
            "/api/v1/orgs/{slug}/chat/mentions",
            get(api::chat::list_mentions),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations",
            get(api::chat::list_conversations).post(api::chat::create_conversation),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations/{id}",
            get(api::chat::get_conversation)
                .patch(api::chat::rename_conversation)
                .delete(api::chat::delete_conversation),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/conversations/{id}/messages",
            post(api::chat::send_message),
        )
        .route("/api/v1/orgs/{slug}/chat/ask", post(api::chat::ask))
        .route(
            "/api/v1/orgs/{slug}/chat/indexing/status",
            get(api::chat_indexing::get_indexing_status),
        )
        .route(
            "/api/v1/orgs/{slug}/chat/indexing/backfill",
            post(api::chat_indexing::trigger_backfill),
        )
        // Org-scoped: CI
        .route(
            "/api/v1/orgs/{slug}/repos/{repo_id}/ci/verify",
            post(api::ci::verify_commits),
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
            get(api::proxy::anthropic_proxy)
                .post(api::proxy::anthropic_proxy)
                .put(api::proxy::anthropic_proxy)
                .delete(api::proxy::anthropic_proxy),
        )
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024));

    let app = Router::new()
        .merge(auth_routes)
        .merge(public_routes)
        .merge(authenticated_routes)
        .merge(proxy_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(AppState {
            pool: pool.clone(),
            repo_manager,
            extensions,
            encryption_key: cfg.encryption_key.clone(),
            http_client: http_client.clone(),
            proxy_http_client: proxy_http_client.clone(),
            cors_origin: cfg.cors_origin.clone(),
            invite_expiry_minutes: cfg.invite_expiry_minutes,
            default_credential_base_url: api::proxy::DEFAULT_ANTHROPIC_UPSTREAM_BASE.to_string(),
            embedding_service,
            proxy_global_semaphore: proxy_global_semaphore.clone(),
            proxy_per_credential_semaphores: proxy_per_credential_semaphores.clone(),
        });

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    tracing::info!("TraceVault server listening on {}", bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

async fn sync_repos_on_startup(
    pool: &sqlx::PgPool,
    repo_manager: &repo_manager::RepoManager,
    extensions: &extensions::ExtensionRegistry,
) {
    let rows = sqlx::query_as::<_, (uuid::Uuid, Option<String>)>(
        "SELECT id, deploy_key_encrypted FROM repos WHERE clone_status = 'ready' AND github_url IS NOT NULL",
    )
    .fetch_all(pool)
    .await;

    let repos = match rows {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to query repos for auto-sync: {e}");
            return;
        }
    };

    if repos.is_empty() {
        return;
    }

    tracing::info!("Auto-syncing {} repo(s) on startup...", repos.len());

    for (repo_id, has_key) in &repos {
        let deploy_key: Option<String> = if has_key.is_some() {
            api::repos::get_deploy_key(pool, *repo_id, extensions.encryption.as_ref())
                .await
                .unwrap_or_default()
        } else {
            None
        };

        match repo_manager.fetch_repo(*repo_id, deploy_key.as_deref()) {
            Ok(()) => {
                sqlx::query("UPDATE repos SET last_fetched_at = now() WHERE id = $1")
                    .bind(repo_id)
                    .execute(pool)
                    .await
                    .ok();
                tracing::info!("Synced repo {repo_id}");
            }
            Err(e) => {
                tracing::warn!("Failed to sync repo {repo_id}: {e}");
            }
        }
    }
}

fn build_extensions(cfg: &config::ServerConfig) -> extensions::ExtensionRegistry {
    #[cfg(feature = "enterprise")]
    {
        use tracevault_core::extensions::EnterpriseConfig;
        let enterprise_cfg = EnterpriseConfig {
            encryption_key: cfg.encryption_key.clone(),
        };
        tracevault_enterprise::register(&enterprise_cfg)
    }

    #[cfg(not(feature = "enterprise"))]
    {
        let mut ext = extensions::community_registry();
        ext.pricing = std::sync::Arc::new(extensions::FullPricingProvider);
        if let Some(ref key) = cfg.encryption_key {
            ext.encryption =
                std::sync::Arc::new(extensions::FullEncryptionProvider::new(key.clone()));
        }
        ext
    }
}
