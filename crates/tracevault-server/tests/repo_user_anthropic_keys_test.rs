//! Integration tests for `UserAnthropicKeyRepo`. Verifies the
//! upsert / get / configured_at / delete lifecycle and that the on-disk
//! ciphertext is recoverable via `encryption::decrypt` — i.e. the layer
//! that the proxy hot path will rely on.

mod common;

use base64::Engine;
use tracevault_server::encryption;
use tracevault_server::repo::user_anthropic_keys::UserAnthropicKeyRepo;

fn fixture_key() -> String {
    // Deterministic 32-byte key for test reproducibility. The real
    // master key comes from config; here we only need any valid value
    // that `encryption::encrypt` will accept.
    base64::engine::general_purpose::STANDARD.encode([0x5Au8; 32])
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_then_get_roundtrips_plaintext(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();
    let plaintext = "sk-ant-test-fixture-not-a-real-key";

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, plaintext)
        .await
        .expect("upsert");

    let (ct, nonce) = UserAnthropicKeyRepo::get_ciphertext(&pool, user_id)
        .await
        .expect("get")
        .expect("row present after upsert");

    let recovered = encryption::decrypt(&ct, &nonce, &master).expect("decrypt");
    assert_eq!(recovered, plaintext);
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_replaces_existing_key(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first")
        .await
        .unwrap();
    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second")
        .await
        .unwrap();

    let (ct, nonce) = UserAnthropicKeyRepo::get_ciphertext(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    let recovered = encryption::decrypt(&ct, &nonce, &master).unwrap();
    assert_eq!(recovered, "sk-ant-second");
}

#[sqlx::test(migrations = "./migrations")]
async fn get_ciphertext_returns_none_when_missing(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let result = UserAnthropicKeyRepo::get_ciphertext(&pool, user_id)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn configured_at_reflects_presence(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    assert!(UserAnthropicKeyRepo::configured_at(&pool, user_id)
        .await
        .unwrap()
        .is_none());

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test")
        .await
        .unwrap();

    let ts = UserAnthropicKeyRepo::configured_at(&pool, user_id)
        .await
        .unwrap();
    assert!(
        ts.is_some(),
        "configured_at should return Some after upsert"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_advances_updated_at(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first")
        .await
        .unwrap();
    let t1 = UserAnthropicKeyRepo::configured_at(&pool, user_id)
        .await
        .unwrap()
        .unwrap();

    // Sleep briefly so postgres `now()` resolves to a later timestamp.
    // Postgres `now()` has microsecond resolution; 10ms is plenty.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second")
        .await
        .unwrap();
    let t2 = UserAnthropicKeyRepo::configured_at(&pool, user_id)
        .await
        .unwrap()
        .unwrap();

    assert!(
        t2 > t1,
        "updated_at should advance on re-upsert: t1={t1} t2={t2}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_removes_row(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test")
        .await
        .unwrap();
    UserAnthropicKeyRepo::delete(&pool, user_id).await.unwrap();

    assert!(UserAnthropicKeyRepo::get_ciphertext(&pool, user_id)
        .await
        .unwrap()
        .is_none());
    assert!(UserAnthropicKeyRepo::configured_at(&pool, user_id)
        .await
        .unwrap()
        .is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_is_idempotent_when_no_row_exists(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    UserAnthropicKeyRepo::delete(&pool, user_id)
        .await
        .expect("delete with no row should succeed");
}

#[sqlx::test(migrations = "./migrations")]
async fn user_deletion_cascades_to_anthropic_key(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test")
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(UserAnthropicKeyRepo::get_ciphertext(&pool, user_id)
        .await
        .unwrap()
        .is_none());
}
