//! Verifies the sliding-session-window behavior used by the auth
//! extractors. On a successful auth the matching `auth_sessions` row has
//! its `expires_at` slid forward to NOW() + 30 days — but only when the
//! expiry has drifted more than a day, so a fresh token is a pure read.
//!
//! The extractors are wired into HTTP handlers and not trivially callable
//! in isolation, so these tests run the *exact* statement the extractors
//! use — `tracevault_server::auth::SLIDING_SESSION_AUTH_SQL` — against a
//! real Postgres pool. Importing the shared const (rather than copying the
//! SQL here) means the test breaks if the production query ever diverges.

mod common;

use chrono::{Duration, Utc};
use tracevault_server::auth::{sha256_hex, SLIDING_SESSION_AUTH_SQL};
use uuid::Uuid;

async fn seed_session_with_expiry(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: chrono::DateTime<Utc>,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO auth_sessions (user_id, token_hash, expires_at) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Run the production auth statement and return the matched `user_id`
/// (or None if the token was expired / not found).
async fn run_auth(pool: &sqlx::PgPool, token_hash: &str) -> Option<Uuid> {
    sqlx::query_as::<_, (Uuid,)>(SLIDING_SESSION_AUTH_SQL)
        .bind(token_hash)
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|(id,)| id)
}

/// Read the current `expires_at` for a token's session row.
async fn current_expiry(pool: &sqlx::PgPool, token_hash: &str) -> chrono::DateTime<Utc> {
    sqlx::query_scalar::<_, chrono::DateTime<Utc>>(
        "SELECT expires_at FROM auth_sessions WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_extends_expires_at_for_a_stale_session(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let token = format!("tok_{}", Uuid::new_v4());
    let token_hash = sha256_hex(&token);

    // Seed an "almost expired" session: 2 minutes left. This is well past
    // the 1-day slide threshold, so auth should both succeed and bump it.
    let initial_expires = Utc::now() + Duration::minutes(2);
    seed_session_with_expiry(&pool, user_id, &token_hash, initial_expires).await;

    let returned = run_auth(&pool, &token_hash)
        .await
        .expect("valid (not-yet-expired) session should authenticate");
    assert_eq!(returned, user_id, "auth must return the owning user_id");

    // After the slide, expires_at should be ~30 days out — at minimum more
    // than 29 days from now. We don't pin it exactly because NOW() is
    // evaluated server-side and clock skew between the test host and the
    // Postgres container is real.
    let min_expected = Utc::now() + Duration::days(29);
    assert!(
        current_expiry(&pool, &token_hash).await > min_expected,
        "sliding window should extend a stale session's expiry to ~30 days"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_does_not_rewrite_a_fresh_session(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let token = format!("tok_{}", Uuid::new_v4());
    let token_hash = sha256_hex(&token);

    // Seed a freshly-bumped session: expires in 30 days. It is within the
    // 1-day slide threshold, so auth must still succeed but leave the row
    // untouched (the common hot-path that avoids a write + row lock).
    let fresh_expires = Utc::now() + Duration::days(30);
    seed_session_with_expiry(&pool, user_id, &token_hash, fresh_expires).await;
    let before = current_expiry(&pool, &token_hash).await;

    let returned = run_auth(&pool, &token_hash)
        .await
        .expect("a fresh session should still authenticate");
    assert_eq!(returned, user_id);

    let after = current_expiry(&pool, &token_hash).await;
    assert_eq!(
        before, after,
        "a fresh session must not be rewritten by auth (no needless write)"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_does_not_revive_an_already_expired_session(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let token = format!("tok_{}", Uuid::new_v4());
    let token_hash = sha256_hex(&token);

    // Seed a session that expired an hour ago.
    let expired_at = Utc::now() - Duration::hours(1);
    seed_session_with_expiry(&pool, user_id, &token_hash, expired_at).await;

    assert!(
        run_auth(&pool, &token_hash).await.is_none(),
        "an already-expired session must NOT authenticate or be revived"
    );

    // Belt-and-braces: the row's expires_at must remain in the past.
    assert!(
        current_expiry(&pool, &token_hash).await < Utc::now(),
        "row's expires_at should remain in the past after a no-op auth"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_is_a_noop_for_unknown_tokens(pool: sqlx::PgPool) {
    let unknown_hash = sha256_hex("definitely-not-a-real-token");
    assert!(run_auth(&pool, &unknown_hash).await.is_none());
}
