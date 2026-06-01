//! Verifies migration 027's end state on a fresh DB: the renamed `credentials`
//! table accepts inserts on its new columns, `(user_id, name)` is unique, the
//! `protocol` CHECK rejects unknown protocols, and at most one default routing
//! rule (match_model IS NULL) is allowed per user.
//!
//! Because `#[sqlx::test]` runs against a fresh, empty database, the data-
//! preservation rename and the backfill INSERT never execute here; those were
//! verified separately against a populated database (a one-off scratch script
//! during implementation), so this test does not cover the backfill.

mod common;

use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn credentials_table_has_new_shape_and_default_rule(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;

    // Insert a credential the way a freshly-migrated prod row would look.
    let cred_id: Uuid = sqlx::query_scalar(
        "INSERT INTO credentials (user_id, name, protocol, base_url, key_encrypted, key_nonce)
         VALUES ($1, 'default', 'anthropic', 'https://api.anthropic.com', 'ct', 'nonce')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_ne!(cred_id, Uuid::nil());

    // A second credential for the same user with a distinct name is allowed.
    sqlx::query(
        "INSERT INTO credentials (user_id, name, protocol, base_url, key_encrypted, key_nonce)
         VALUES ($1, 'staging', 'anthropic', 'https://api.anthropic.com', 'ct2', 'nonce2')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    // Duplicate (user_id, name) is rejected.
    let dup = sqlx::query(
        "INSERT INTO credentials (user_id, name, protocol, base_url, key_encrypted, key_nonce)
         VALUES ($1, 'default', 'anthropic', 'https://api.anthropic.com', 'x', 'y')",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(dup.is_err(), "(user_id, name) must be unique");

    // protocol CHECK rejects unknown protocols.
    let bad = sqlx::query(
        "INSERT INTO credentials (user_id, name, protocol, base_url, key_encrypted, key_nonce)
         VALUES ($1, 'oai', 'openai', 'https://api.openai.com', 'x', 'y')",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(
        bad.is_err(),
        "protocol CHECK must reject 'openai' in step 1"
    );

    // A default routing rule (match_model IS NULL) is unique per user.
    sqlx::query(
        "INSERT INTO proxy_routing_rules (user_id, match_model, credential_name)
         VALUES ($1, NULL, 'default')",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let second_default = sqlx::query(
        "INSERT INTO proxy_routing_rules (user_id, match_model, credential_name)
         VALUES ($1, NULL, 'staging')",
    )
    .bind(user_id)
    .execute(&pool)
    .await;
    assert!(
        second_default.is_err(),
        "only one default rule per user allowed"
    );
}
