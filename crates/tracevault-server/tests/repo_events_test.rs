mod common;

use chrono::Utc;
use tracevault_server::repo::events::{
    EventRepo, InsertFileChange, InsertToolEvent, InsertTranscriptChunk,
};
use tracevault_server::repo::sessions::{SessionRepo, UpsertSession};

async fn create_session(pool: &sqlx::PgPool) -> (uuid::Uuid, uuid::Uuid) {
    let org_id = common::seed_org(pool).await;
    let repo_id = common::seed_repo(pool, org_id).await;
    let user_id = common::seed_user(pool).await;

    let session_pk = SessionRepo::upsert(
        pool,
        &UpsertSession {
            org_id,
            repo_id,
            user_id,
            session_id: format!("evt-sess-{}", uuid::Uuid::new_v4()),
            model: None,
            cwd: None,
            tool: Some("claude-code".into()),
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    (session_pk, org_id)
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_tool_event_returns_id(pool: sqlx::PgPool) {
    let (session_pk, _) = create_session(&pool).await;

    let id = EventRepo::insert_tool_event(
        &pool,
        &InsertToolEvent {
            session_id: session_pk,
            event_index: 1,
            tool_name: Some("read_file".into()),
            tool_input: Some(serde_json::json!({"path": "/foo/bar.rs"})),
            tool_response: Some(serde_json::json!({"content": "fn main() {}"})),
            tool_is_error: None,
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    assert!(id.is_some(), "first insert should return an id");
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_tool_event_conflict_returns_none(pool: sqlx::PgPool) {
    let (session_pk, _) = create_session(&pool).await;

    let req = InsertToolEvent {
        session_id: session_pk,
        event_index: 42,
        tool_name: Some("write_file".into()),
        tool_input: None,
        tool_response: None,
        tool_is_error: None,
        timestamp: Some(Utc::now()),
    };

    let first = EventRepo::insert_tool_event(&pool, &req).await.unwrap();
    assert!(first.is_some());

    // Same (session_id, event_index) => conflict => None
    let second = EventRepo::insert_tool_event(&pool, &req).await.unwrap();
    assert!(second.is_none(), "duplicate should return None");
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_file_change_succeeds(pool: sqlx::PgPool) {
    let (session_pk, _) = create_session(&pool).await;

    let event_id = EventRepo::insert_tool_event(
        &pool,
        &InsertToolEvent {
            session_id: session_pk,
            event_index: 1,
            tool_name: Some("edit_file".into()),
            tool_input: None,
            tool_response: None,
            tool_is_error: None,
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap()
    .unwrap();

    EventRepo::insert_file_change(
        &pool,
        &InsertFileChange {
            session_id: session_pk,
            event_id,
            file_path: "src/main.rs".into(),
            change_type: "modified".into(),
            diff_text: Some("@@ -1,3 +1,4 @@\n+use foo;\n".into()),
            content_hash: Some("abc123".into()),
            timestamp: Some(Utc::now()),
        },
    )
    .await
    .unwrap();

    // Verify row exists
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM file_changes WHERE session_id = $1 AND event_id = $2")
            .bind(session_pk)
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn insert_transcript_chunk_dedup(pool: sqlx::PgPool) {
    let (session_pk, _) = create_session(&pool).await;

    let chunk = InsertTranscriptChunk {
        session_id: session_pk,
        chunk_index: 0,
        data: serde_json::json!({"role": "user", "content": "hello"}),
    };

    let inserted = EventRepo::insert_transcript_chunk(&pool, &chunk)
        .await
        .unwrap();
    assert!(inserted, "first insert should return true");

    // Duplicate should not error (ON CONFLICT DO NOTHING) and should return false
    let inserted_again = EventRepo::insert_transcript_chunk(&pool, &chunk)
        .await
        .unwrap();
    assert!(!inserted_again, "duplicate insert should return false");

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transcript_chunks WHERE session_id = $1")
            .bind(session_pk)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}
