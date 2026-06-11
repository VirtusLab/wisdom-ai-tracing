use std::net::SocketAddr;

use tracevault_server::{
    api, build_router, config, db, extensions, plugins, pricing_sync, repo_manager, AppState,
};

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

    // Sync pricing from LiteLLM on startup (non-blocking on failure)
    match pricing_sync::sync_pricing(&pool, &http_client).await {
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
                match pricing_sync::sync_pricing(&pool, &client).await {
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

    let plugins = std::sync::Arc::new(plugins::Plugins::default());

    let state = AppState {
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
        plugins: plugins.clone(),
    };

    let app = build_router(state);

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
