mod common;

use tracevault_server::repo::credentials::CredentialRepo;
use tracevault_server::repo::routing::RoutingRepo;

const ENC_KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="; // base64 of 32 bytes

#[sqlx::test(migrations = "./migrations")]
async fn resolve_for_model_prefers_matching_rule_then_default(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    // two credentials
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "default",
        "https://api.anthropic.com",
        "sk-ant-d",
        Some(8),
    )
    .await
    .unwrap();
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "fast",
        "https://gw.example.com",
        "sk-ant-f",
        Some(8),
    )
    .await
    .unwrap();
    RoutingRepo::ensure_default(&pool, user_id, "default")
        .await
        .unwrap();
    // a model rule: claude-haiku -> fast, rewrite to "claude-3-5-haiku-latest"
    RoutingRepo::upsert_rule(
        &pool,
        user_id,
        Some("claude-haiku"),
        "fast",
        Some("claude-3-5-haiku-latest"),
    )
    .await
    .unwrap();

    // matching model -> fast credential + rewrite target
    let m = CredentialRepo::resolve_for_model(&pool, user_id, Some("claude-haiku"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(m.base_url, "https://gw.example.com");
    assert_eq!(m.provider_model.as_deref(), Some("claude-3-5-haiku-latest"));

    // non-matching model -> default credential, no rewrite
    let d = CredentialRepo::resolve_for_model(&pool, user_id, Some("claude-opus"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(d.base_url, "https://api.anthropic.com");
    assert_eq!(d.provider_model, None);

    // no model (None) -> default
    let n = CredentialRepo::resolve_for_model(&pool, user_id, None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(n.base_url, "https://api.anthropic.com");
}

#[sqlx::test(migrations = "./migrations")]
async fn list_returns_all_named_credentials(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "default",
        "https://api.anthropic.com",
        "sk-ant-d",
        Some(8),
    )
    .await
    .unwrap();
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "fast",
        "https://gw.example.com",
        "sk-ant-f",
        Some(16),
    )
    .await
    .unwrap();
    let list = CredentialRepo::list(&pool, user_id).await.unwrap();
    assert_eq!(list.len(), 2);
    assert!(list
        .iter()
        .any(|c| c.name == "fast" && c.max_concurrent == 16));
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_then_resolve_default(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;

    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "default",
        "https://api.anthropic.com",
        "sk-ant-secret",
        Some(16),
    )
    .await
    .unwrap();

    // A default rule must exist for resolve_default to find it.
    sqlx::query(
        "INSERT INTO proxy_routing_rules (user_id, match_model, credential_name)
         VALUES ($1, NULL, 'default')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let resolved = CredentialRepo::resolve_default(&pool, user_id)
        .await
        .unwrap()
        .expect("default credential should resolve");
    assert_eq!(resolved.protocol, "anthropic");
    assert_eq!(resolved.base_url, "https://api.anthropic.com");
    assert_eq!(resolved.max_concurrent, 16);

    let plaintext =
        tracevault_server::encryption::decrypt(&resolved.encrypted, &resolved.nonce, ENC_KEY)
            .unwrap();
    assert_eq!(plaintext, "sk-ant-secret");
}

#[sqlx::test(migrations = "./migrations")]
async fn resolve_default_is_none_without_rule(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "default",
        "https://api.anthropic.com",
        "sk-ant-x",
        None,
    )
    .await
    .unwrap();
    // No routing rule inserted.
    assert!(CredentialRepo::resolve_default(&pool, user_id)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn update_cap_only_and_delete(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    CredentialRepo::upsert(
        &pool,
        ENC_KEY,
        user_id,
        "default",
        "https://api.anthropic.com",
        "sk-ant-x",
        Some(8),
    )
    .await
    .unwrap();

    assert!(
        CredentialRepo::update_max_concurrent(&pool, user_id, "default", 32)
            .await
            .unwrap()
    );
    assert!(
        !CredentialRepo::update_max_concurrent(&pool, user_id, "missing", 32)
            .await
            .unwrap()
    );

    CredentialRepo::delete(&pool, user_id, "default")
        .await
        .unwrap();
    CredentialRepo::delete(&pool, user_id, "default")
        .await
        .unwrap(); // idempotent
}
