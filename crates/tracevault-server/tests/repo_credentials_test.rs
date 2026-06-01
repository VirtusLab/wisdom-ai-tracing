mod common;

use tracevault_server::repo::credentials::CredentialRepo;

const ENC_KEY: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="; // base64 of 32 bytes

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
