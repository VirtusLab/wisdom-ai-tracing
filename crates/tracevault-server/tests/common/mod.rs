use sqlx::PgPool;
use uuid::Uuid;

/// Build a full `AppState` for tests, with a caller-supplied `Plugins`.
/// Field values mirror `dedup_test.rs::build_state`.
#[allow(dead_code)]
pub fn test_state_with_plugins(
    pool: sqlx::PgPool,
    plugins: std::sync::Arc<tracevault_server::plugins::Plugins>,
) -> tracevault_server::AppState {
    tracevault_server::AppState {
        pool,
        repo_manager: tracevault_server::repo_manager::RepoManager::new("/tmp"),
        extensions: tracevault_server::extensions::community_registry(),
        encryption_key: None,
        http_client: reqwest::Client::new(),
        proxy_http_client: reqwest::Client::new(),
        cors_origin: "*".to_string(),
        invite_expiry_minutes: 60,
        embedding_service: None,
        default_credential_base_url: "http://localhost".to_string(),
        proxy_global_semaphore: None,
        proxy_per_credential_semaphores: std::sync::Arc::new(dashmap::DashMap::new()),
        plugins,
    }
}

#[allow(dead_code)]
pub async fn seed_invite(
    pool: &PgPool,
    org_id: Uuid,
    email: &str,
    role: &str,
    invited_by: Uuid,
    token_hash: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO org_invites (org_id, email, role, token_hash, invited_by, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(org_id)
    .bind(email)
    .bind(role)
    .bind(token_hash)
    .bind(invited_by)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_org_with_member(pool: &PgPool, user_id: Uuid) -> Uuid {
    let org_id = seed_org(pool).await;
    seed_membership(pool, user_id, org_id, "admin").await;
    org_id
}

#[allow(dead_code)]
pub async fn seed_org(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO orgs (name) VALUES ('test-org-' || gen_random_uuid()::text) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_user(pool: &PgPool) -> Uuid {
    let email = format!("test-{}@example.com", Uuid::new_v4());
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (email, password_hash, name) \
         VALUES ($1, '$argon2id$v=19$m=19456,t=2,p=1$fake_salt$fake_hash', 'Test User') \
         RETURNING id",
    )
    .bind(&email)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_repo(pool: &PgPool, org_id: Uuid) -> Uuid {
    let name = format!("test-repo-{}", Uuid::new_v4());
    sqlx::query_scalar::<_, Uuid>("INSERT INTO repos (org_id, name) VALUES ($1, $2) RETURNING id")
        .bind(org_id)
        .bind(&name)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[allow(dead_code)]
pub async fn seed_membership(pool: &PgPool, user_id: Uuid, org_id: Uuid, role: &str) {
    sqlx::query("INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(org_id)
        .bind(role)
        .execute(pool)
        .await
        .unwrap();
}

#[allow(dead_code)]
pub async fn seed_session(pool: &PgPool, org_id: Uuid, repo_id: Uuid, user_id: Uuid) -> Uuid {
    use tracevault_server::repo::sessions::{SessionRepo, UpsertSession};
    SessionRepo::upsert(
        pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: format!("sess-{}", Uuid::new_v4()),
            model: Some("sonnet".into()),
            cwd: Some("/project".into()),
            tool: Some("claude-code".into()),
            timestamp: Some(chrono::Utc::now()),
        },
    )
    .await
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_event(pool: &PgPool, session_id: Uuid, event_index: i32) -> Uuid {
    use tracevault_server::repo::events::{EventRepo, InsertToolEvent};
    EventRepo::insert_tool_event(
        pool,
        &InsertToolEvent {
            session_id,
            event_index: Some(event_index),
            event_uuid: None,
            tool_name: Some("Read".into()),
            tool_input: Some(serde_json::json!({"file_path": "/tmp/test.rs"})),
            tool_response: None,
            tool_is_error: None,
            timestamp: Some(chrono::Utc::now()),
            hook_event_name: None,
            tool_use_id: None,
        },
    )
    .await
    .unwrap()
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_commit(pool: &PgPool, repo_id: Uuid, sha: &str) -> Uuid {
    use tracevault_server::repo::commits::{CommitRepo, UpsertCommit};
    CommitRepo::upsert(
        pool,
        &UpsertCommit {
            repo_id,
            commit_sha: sha.into(),
            branch: Some("main".into()),
            author: "dev@test.com".into(),
            message: Some("test commit".into()),
            diff_data: None,
            committed_at: Some(chrono::Utc::now()),
        },
    )
    .await
    .unwrap()
}

#[allow(dead_code)]
pub async fn seed_api_key(pool: &PgPool, org_id: Uuid) -> (Uuid, String) {
    use tracevault_server::repo::api_keys::ApiKeyRepo;
    let hash = format!("keyhash_{}", Uuid::new_v4());
    let id = ApiKeyRepo::create(pool, org_id, &hash, "test-key")
        .await
        .unwrap();
    (id, hash)
}

/// Insert an auth_sessions row for the given user with the given token's
/// sha256 hash and a far-future expiry. Returns the *raw* token so callers
/// can use it directly in an Authorization header or x-api-key header.
///
/// Distinct from `seed_session`, which seeds the trace `sessions` table.
#[allow(dead_code)]
pub async fn seed_auth_session(pool: &PgPool, user_id: Uuid) -> String {
    let (raw, hash) = tracevault_server::auth::generate_session_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(30);
    sqlx::query(
        "INSERT INTO auth_sessions (user_id, token_hash, expires_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&hash)
    .bind(expires_at)
    .execute(pool)
    .await
    .unwrap();
    raw
}

/// A deterministic 32-byte base64 encryption key suitable for `encrypt`/
/// `decrypt`. Fixture only — never use in production.
#[allow(dead_code)]
pub fn fixture_encryption_key() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode([0x5Au8; 32])
}
