//! Integration tests for `UserAnthropicKeyRepo`. Verifies the
//! upsert / get / status / delete lifecycle, the on-disk ciphertext is
//! recoverable via `encryption::decrypt`, and that the per-credential
//! `max_concurrent` cap roundtrips correctly through upsert / read paths
//! (issue softwaremill/tracevault#210).

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

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, plaintext, None)
        .await
        .expect("upsert");

    let cred = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .expect("get")
        .expect("row present after upsert");

    let recovered = encryption::decrypt(&cred.encrypted, &cred.nonce, &master).expect("decrypt");
    assert_eq!(recovered, plaintext);
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_replaces_existing_key(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first", None)
        .await
        .unwrap();
    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second", None)
        .await
        .unwrap();

    let cred = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    let recovered = encryption::decrypt(&cred.encrypted, &cred.nonce, &master).unwrap();
    assert_eq!(recovered, "sk-ant-second");
}

#[sqlx::test(migrations = "./migrations")]
async fn get_credential_returns_none_when_missing(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let result = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap();
    assert!(result.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn status_reflects_presence(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    assert!(UserAnthropicKeyRepo::status(&pool, user_id)
        .await
        .unwrap()
        .is_none());

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test", None)
        .await
        .unwrap();

    let s = UserAnthropicKeyRepo::status(&pool, user_id)
        .await
        .unwrap()
        .expect("status should return Some after upsert");
    assert_eq!(
        s.max_concurrent, 8,
        "fresh upsert without explicit cap should use DB default of 8"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_advances_updated_at(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first", None)
        .await
        .unwrap();
    let t1 = UserAnthropicKeyRepo::status(&pool, user_id)
        .await
        .unwrap()
        .unwrap()
        .configured_at;

    // Sleep briefly so postgres `now()` resolves to a later timestamp.
    // Postgres `now()` has microsecond resolution; 10ms is plenty.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second", None)
        .await
        .unwrap();
    let t2 = UserAnthropicKeyRepo::status(&pool, user_id)
        .await
        .unwrap()
        .unwrap()
        .configured_at;

    assert!(
        t2 > t1,
        "updated_at should advance on re-upsert: t1={t1} t2={t2}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_persists_explicit_max_concurrent(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test", Some(32))
        .await
        .unwrap();

    let cred = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cred.max_concurrent, 32);

    let status = UserAnthropicKeyRepo::status(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status.max_concurrent, 32);
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_without_cap_preserves_existing_value(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    // First write picks an explicit non-default cap.
    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first", Some(16))
        .await
        .unwrap();

    // Rotate the key without specifying the cap — the existing 16 must be
    // preserved, *not* reset to the DB default of 8.
    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second", None)
        .await
        .unwrap();

    let cred = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cred.max_concurrent, 16,
        "rotating the key without a new cap must keep the existing cap"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn upsert_with_new_cap_overrides_existing_value(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-first", Some(16))
        .await
        .unwrap();
    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-second", Some(4))
        .await
        .unwrap();

    let cred = UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cred.max_concurrent, 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_removes_row(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let master = fixture_key();

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test", None)
        .await
        .unwrap();
    UserAnthropicKeyRepo::delete(&pool, user_id).await.unwrap();

    assert!(UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .is_none());
    assert!(UserAnthropicKeyRepo::status(&pool, user_id)
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

    UserAnthropicKeyRepo::upsert(&pool, &master, user_id, "sk-ant-test", None)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    assert!(UserAnthropicKeyRepo::get_credential(&pool, user_id)
        .await
        .unwrap()
        .is_none());
}
