use sqlx::PgPool;
use uuid::Uuid;

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
            event_index,
            tool_name: Some("Read".into()),
            tool_input: Some(serde_json::json!({"file_path": "/tmp/test.rs"})),
            tool_response: None,
            tool_is_error: None,
            timestamp: Some(chrono::Utc::now()),
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
