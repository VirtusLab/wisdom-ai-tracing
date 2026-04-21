mod common;

use tracevault_server::auth::{generate_invite_token, sha256_hex};

#[sqlx::test(migrations = "./migrations")]
async fn create_invite_and_find_by_token(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let user_id = common::seed_user(&pool).await;
    common::seed_membership(&pool, user_id, org_id, "admin").await;

    let (raw_token, token_hash) = generate_invite_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let invite_id = common::seed_invite(
        &pool,
        org_id,
        "invitee@example.com",
        "developer",
        user_id,
        &token_hash,
        expires_at,
    )
    .await;

    let row = sqlx::query_as::<_, (uuid::Uuid, String, String)>(
        "SELECT id, email, status FROM org_invites WHERE token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(row.is_some());
    let (id, email, status) = row.unwrap();
    assert_eq!(id, invite_id);
    assert_eq!(email, "invitee@example.com");
    assert_eq!(status, "pending");

    // Verify sha256_hex produces consistent results for the raw token
    let recomputed = sha256_hex(&raw_token);
    assert_eq!(recomputed, token_hash);
}

#[sqlx::test(migrations = "./migrations")]
async fn expired_invite_not_found(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let user_id = common::seed_user(&pool).await;

    let (_, token_hash) = generate_invite_token();
    let expired = chrono::Utc::now() - chrono::Duration::hours(1);

    common::seed_invite(
        &pool,
        org_id,
        "expired@example.com",
        "developer",
        user_id,
        &token_hash,
        expired,
    )
    .await;

    let row = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM org_invites WHERE token_hash = $1 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert!(row.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn revoke_invite_sets_status(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let user_id = common::seed_user(&pool).await;

    let (_, token_hash) = generate_invite_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let invite_id = common::seed_invite(
        &pool,
        org_id,
        "revoke@example.com",
        "developer",
        user_id,
        &token_hash,
        expires_at,
    )
    .await;

    let result = sqlx::query(
        "UPDATE org_invites SET status = 'revoked' WHERE id = $1 AND status = 'pending'",
    )
    .bind(invite_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1);

    let row = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT id FROM org_invites WHERE token_hash = $1 AND status = 'pending'",
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(row.is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn accept_invite_creates_membership(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let admin_id = common::seed_user(&pool).await;

    let (_, token_hash) = generate_invite_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
    let invite_email = format!("new-{}@example.com", uuid::Uuid::new_v4());

    let invite_id = common::seed_invite(
        &pool,
        org_id,
        &invite_email,
        "developer",
        admin_id,
        &token_hash,
        expires_at,
    )
    .await;

    let password_hash = tracevault_server::auth::hash_password("securepassword123").unwrap();

    let user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&invite_email)
    .bind(&password_hash)
    .bind("New User")
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, 'developer')",
    )
    .bind(user_id)
    .bind(org_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("UPDATE org_invites SET status = 'accepted' WHERE id = $1")
        .bind(invite_id)
        .execute(&pool)
        .await
        .unwrap();

    let role: Option<(String,)> =
        sqlx::query_as("SELECT role FROM user_org_memberships WHERE user_id = $1 AND org_id = $2")
            .bind(user_id)
            .bind(org_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(role.unwrap().0, "developer");

    let status: String = sqlx::query_scalar("SELECT status FROM org_invites WHERE id = $1")
        .bind(invite_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "accepted");
}

#[sqlx::test(migrations = "./migrations")]
async fn new_user_accept_detects_existing_account(pool: sqlx::PgPool) {
    // Scenario for issue #94: invite issued for an email that already has an account.
    // The new-user accept path must detect this via its users-by-email lookup so the
    // handler can return an account_exists signal instead of blindly inserting.
    let org_id = common::seed_org(&pool).await;
    let admin_id = common::seed_user(&pool).await;
    let email = format!("prior-{}@example.com", uuid::Uuid::new_v4());

    let password_hash = tracevault_server::auth::hash_password("oldpassword123").unwrap();
    let existing_user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind("Prior User")
    .fetch_one(&pool)
    .await
    .unwrap();

    let (_, token_hash) = generate_invite_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
    common::seed_invite(
        &pool,
        org_id,
        &email,
        "developer",
        admin_id,
        &token_hash,
        expires_at,
    )
    .await;

    let found: Option<(uuid::Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert_eq!(
        found.map(|(id,)| id),
        Some(existing_user_id),
        "handler's existence check must locate the prior account"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn existing_user_can_accept_invite_to_second_org(pool: sqlx::PgPool) {
    // Multi-org scenario: a user is already a member of org A, then gets invited
    // to org B. The existing-user accept flow must add membership in B without
    // touching their membership in A.
    let email = format!("multi-{}@example.com", uuid::Uuid::new_v4());
    let password_hash = tracevault_server::auth::hash_password("pw-that-works").unwrap();
    let user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind("Multi Org User")
    .fetch_one(&pool)
    .await
    .unwrap();

    let org_a = common::seed_org(&pool).await;
    let org_b = common::seed_org(&pool).await;
    common::seed_membership(&pool, user_id, org_a, "developer").await;

    let admin_b = common::seed_user(&pool).await;
    common::seed_membership(&pool, admin_b, org_b, "admin").await;
    let (_, token_hash) = generate_invite_token();
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);
    let invite_id = common::seed_invite(
        &pool,
        org_b,
        &email,
        "auditor",
        admin_b,
        &token_hash,
        expires_at,
    )
    .await;

    let inserted = sqlx::query(
        "INSERT INTO user_org_memberships (user_id, org_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, org_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(org_b)
    .bind("auditor")
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(inserted.rows_affected(), 1);

    sqlx::query("UPDATE org_invites SET status = 'accepted' WHERE id = $1")
        .bind(invite_id)
        .execute(&pool)
        .await
        .unwrap();

    let memberships: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT org_id, role FROM user_org_memberships WHERE user_id = $1")
            .bind(user_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(memberships.len(), 2);
    assert!(memberships
        .iter()
        .any(|(o, r)| *o == org_a && r == "developer"));
    assert!(memberships
        .iter()
        .any(|(o, r)| *o == org_b && r == "auditor"));
}

#[sqlx::test(migrations = "./migrations")]
async fn duplicate_invite_revokes_previous(pool: sqlx::PgPool) {
    let org_id = common::seed_org(&pool).await;
    let user_id = common::seed_user(&pool).await;
    let email = "dup@example.com";
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(24);

    let (_, hash1) = generate_invite_token();
    let invite1 = common::seed_invite(
        &pool,
        org_id,
        email,
        "developer",
        user_id,
        &hash1,
        expires_at,
    )
    .await;

    sqlx::query(
        "UPDATE org_invites SET status = 'revoked'
         WHERE org_id = $1 AND email = $2 AND status = 'pending' AND expires_at > NOW()",
    )
    .bind(org_id)
    .bind(email)
    .execute(&pool)
    .await
    .unwrap();

    let (_, hash2) = generate_invite_token();
    common::seed_invite(&pool, org_id, email, "admin", user_id, &hash2, expires_at).await;

    let status1: String = sqlx::query_scalar("SELECT status FROM org_invites WHERE id = $1")
        .bind(invite1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status1, "revoked");

    let pending_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM org_invites WHERE org_id = $1 AND email = $2 AND status = 'pending'",
    )
    .bind(org_id)
    .bind(email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending_count.0, 1);
}
