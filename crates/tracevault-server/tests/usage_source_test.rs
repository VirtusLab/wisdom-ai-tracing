mod common;

use tracevault_server::api::analytics::UsageSource;

#[sqlx::test(migrations = "./migrations")]
async fn usage_source_defaults_to_both(pool: sqlx::PgPool) {
    let user_id = common::seed_user(&pool).await;
    let org_id = common::seed_org_with_member(&pool, user_id).await;
    sqlx::query("INSERT INTO org_compliance_settings (org_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(org_id)
        .execute(&pool)
        .await
        .unwrap();

    let src = tracevault_server::api::analytics::fetch_usage_source_for_test(&pool, org_id).await;
    assert_eq!(src, UsageSource::Both);
}
