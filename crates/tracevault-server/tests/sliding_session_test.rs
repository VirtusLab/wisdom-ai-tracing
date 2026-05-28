//! Verifies the sliding-session-window behavior added to the auth
//! extractors. On every successful auth, the matching `auth_sessions` row
//! has its `expires_at` bumped to NOW() + 30 days.
//!
//! The extractor is wired into HTTP handlers and not trivially callable in
//! isolation, but the SQL pattern it uses is `UPDATE auth_sessions SET
//! expires_at = NOW() + INTERVAL '30 days' WHERE token_hash = $1 AND
//! expires_at > NOW() RETURNING user_id`. These tests exercise that
//! statement directly against a real Postgres pool, which is the precise
//! source of the behavior we want to lock in.

mod common;

use chrono::{Duration, Utc};
use tracevault_server::auth::sha256_hex;
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

/// Run the sliding-window UPDATE used by the extractor and return the
/// updated `expires_at` for the session row (or None if the row was
/// expired / not found).
async fn run_sliding_update(
    pool: &sqlx::PgPool,
    token_hash: &str,
) -> Option<chrono::DateTime<Utc>> {
    sqlx::query_as::<_, (chrono::DateTime<Utc>,)>(
        "UPDATE auth_sessions
         SET expires_at = NOW() + INTERVAL '30 days'
         WHERE token_hash = $1 AND expires_at > NOW()
         RETURNING expires_at",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .unwrap()
    .map(|(ts,)| ts)
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_extends_expires_at_on_each_hit(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let token = format!("tok_{}", Uuid::new_v4());
    let token_hash = sha256_hex(&token);

    // Seed an "almost expired" session: 2 minutes left.
    let initial_expires = Utc::now() + Duration::minutes(2);
    seed_session_with_expiry(&pool, user_id, &token_hash, initial_expires).await;

    let bumped = run_sliding_update(&pool, &token_hash)
        .await
        .expect("valid (not-yet-expired) session should be extended");

    // After the bump, expires_at should be ~30 days in the future — at
    // minimum, more than 29 days from now. We don't pin it exactly because
    // NOW() in Postgres is evaluated server-side and clock skew between
    // test machine and server is real (postgres in Docker, test in host).
    let min_expected = Utc::now() + Duration::days(29);
    assert!(
        bumped > min_expected,
        "sliding window should extend expires_at to ~30 days; got {bumped}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_does_not_extend_already_expired_session(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let token = format!("tok_{}", Uuid::new_v4());
    let token_hash = sha256_hex(&token);

    // Seed a session that expired an hour ago.
    let expired_at = Utc::now() - Duration::hours(1);
    seed_session_with_expiry(&pool, user_id, &token_hash, expired_at).await;

    let result = run_sliding_update(&pool, &token_hash).await;
    assert!(
        result.is_none(),
        "an already-expired session must NOT be revived by a sliding-window update; got {result:?}"
    );

    // Belt-and-braces: confirm the row's expires_at is still in the past.
    let row_expires_at = sqlx::query_scalar::<_, chrono::DateTime<Utc>>(
        "SELECT expires_at FROM auth_sessions WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        row_expires_at < Utc::now(),
        "row's expires_at should remain in the past after a no-op sliding update"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn auth_sliding_update_is_a_noop_for_unknown_tokens(pool: sqlx::PgPool) {
    let unknown_hash = sha256_hex("definitely-not-a-real-token");
    let result = run_sliding_update(&pool, &unknown_hash).await;
    assert!(result.is_none());
}
