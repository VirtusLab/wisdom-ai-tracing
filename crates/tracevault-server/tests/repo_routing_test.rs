mod common;

use tracevault_server::repo::routing::RoutingRepo;
use uuid::Uuid;

/// Seed a bare credential row so the routing rules' FK
/// `(user_id, credential_name) -> credentials(user_id, name)` is satisfied.
/// The routing tests only care about the rule pointer, not key material.
async fn seed_credential(pool: &sqlx::PgPool, user_id: Uuid, name: &str) {
    sqlx::query(
        "INSERT INTO credentials (user_id, name, protocol, base_url, key_encrypted, key_nonce)
         VALUES ($1, $2, 'anthropic', 'https://api.anthropic.com', 'ct', 'nonce')",
    )
    .bind(user_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn ensure_default_is_idempotent_and_repointable(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    seed_credential(&pool, user_id, "default").await;
    seed_credential(&pool, user_id, "staging").await;

    RoutingRepo::ensure_default(&pool, user_id, "default")
        .await
        .unwrap();
    // Second ensure with a different name must NOT overwrite the first.
    RoutingRepo::ensure_default(&pool, user_id, "staging")
        .await
        .unwrap();
    assert_eq!(
        RoutingRepo::default_credential_name(&pool, user_id)
            .await
            .unwrap()
            .as_deref(),
        Some("default"),
    );

    // Explicit repoint changes it.
    assert!(
        RoutingRepo::set_default_credential(&pool, user_id, "staging")
            .await
            .unwrap()
    );
    assert_eq!(
        RoutingRepo::default_credential_name(&pool, user_id)
            .await
            .unwrap()
            .as_deref(),
        Some("staging"),
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn set_default_returns_false_without_rule(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    assert!(!RoutingRepo::set_default_credential(&pool, user_id, "x")
        .await
        .unwrap());
    assert!(RoutingRepo::default_credential_name(&pool, user_id)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn rule_crud(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    seed_credential(&pool, user_id, "default").await;
    seed_credential(&pool, user_id, "fast").await;
    RoutingRepo::ensure_default(&pool, user_id, "default")
        .await
        .unwrap();

    // add a model rule
    RoutingRepo::upsert_rule(
        &pool,
        user_id,
        Some("claude-haiku"),
        "fast",
        Some("x-haiku"),
    )
    .await
    .unwrap();
    let rules = RoutingRepo::list(&pool, user_id).await.unwrap();
    assert_eq!(rules.len(), 2); // default + model rule
    let model_rule = rules
        .iter()
        .find(|r| r.match_model.as_deref() == Some("claude-haiku"))
        .unwrap();
    assert_eq!(model_rule.credential_name, "fast");
    assert_eq!(model_rule.provider_model.as_deref(), Some("x-haiku"));

    // upsert same model updates it
    RoutingRepo::upsert_rule(&pool, user_id, Some("claude-haiku"), "default", None)
        .await
        .unwrap();
    let rules = RoutingRepo::list(&pool, user_id).await.unwrap();
    assert_eq!(
        rules
            .iter()
            .filter(|r| r.match_model.as_deref() == Some("claude-haiku"))
            .count(),
        1
    );

    // delete by id
    let id = rules
        .iter()
        .find(|r| r.match_model.as_deref() == Some("claude-haiku"))
        .unwrap()
        .id;
    assert!(RoutingRepo::delete_rule(&pool, user_id, id).await.unwrap());

    // FK: rule pointing at a missing credential is rejected
    assert!(
        RoutingRepo::upsert_rule(&pool, user_id, Some("m"), "ghost", None)
            .await
            .is_err()
    );
}
