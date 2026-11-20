//! Opt-in contract tests for tool media. The default suite never connects.
#![allow(clippy::too_many_lines)] // Each case is one crash-resume story.

use std::time::Duration;

use agent::{ExecutionProfile, File, Image, ImageMediaType};
use brainstone_runtime::{
    Address, DeliveryStore, Media, OperationId, PostgresStore, Prompt, Runtime, Store,
    SubagentConfig, SubagentRequest, SubagentStore,
};

async fn connect() -> (PostgresStore, sqlx::PgPool) {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    (store, sqlx::PgPool::connect(&url).await.unwrap())
}

fn address(lane: &str) -> Address {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    Address::new("contract", &format!("media-{nanos}"), lane).unwrap()
}

/// Every operation a case creates is settled, so a later suite's workers on
/// the same database never pick it up.
async fn settle(
    store: &PostgresStore,
    id: OperationId,
    claimed: Option<brainstone_runtime::Lease>,
) {
    let lease = if let Some(lease) = claimed {
        lease
    } else {
        let claim = store.claim_operation(id, "settle", Duration::from_secs(30));
        claim.await.unwrap().unwrap().lease
    };
    let done = agent::Outcome::Completed {
        text: String::new(),
    };
    store.settle(&lease, &done).await.unwrap();
}

fn image() -> Image {
    Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap()
}

/// Stored once under a lease, media resolves by reference for every lane of
/// the conversation from a freshly connected store, and for no other.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_resolves_tool_media_after_a_restart() {
    let (store, _) = connect().await;
    let here = address("lane-1");
    let first = Prompt::new(here.clone(), "capture", "system", "record ☕").unwrap();
    let id = store.accept(&first).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let media = [
        Media::Image(image()),
        Media::File(file("compte rendu ☕.md", "text/markdown")),
    ];
    store.save_media(&claimed.lease, &media).await.unwrap();
    // A retried call stores the same bytes again.
    store.save_media(&claimed.lease, &media).await.unwrap();
    let other_lane = Address::new("contract", here.conversation(), "lane-2").unwrap();
    let other_lane = Prompt::new(other_lane, "later", "system", "again").unwrap();
    let other_lane = store.accept(&other_lane).await.unwrap().operation.id;
    let elsewhere = Prompt::new(address("lane-1"), "other", "system", "hi").unwrap();
    let elsewhere = store.accept(&elsewhere).await.unwrap().operation.id;

    let (restarted, _) = connect().await;

    for item in &media {
        let reference = item.reference();
        assert_eq!(
            restarted.media(id, &reference).await.unwrap().as_ref(),
            Some(item)
        );
        assert_eq!(
            restarted
                .media(other_lane, &reference)
                .await
                .unwrap()
                .as_ref(),
            Some(item)
        );
        assert_eq!(restarted.media(elsewhere, &reference).await.unwrap(), None);
    }
    let unknown = OperationId::new(i64::MAX).unwrap();
    assert_eq!(
        restarted
            .media(unknown, &media[0].reference())
            .await
            .unwrap(),
        None
    );
    settle(&store, id, Some(claimed.lease)).await;
    settle(&store, other_lane, None).await;
    settle(&store, elsewhere, None).await;
}

fn file(name: &str, media_type: &str) -> File {
    File::new(name, media_type, b"# Notes".to_vec()).unwrap()
}

/// A file keeps its name and any plain media type, including an image's or a
/// video's: it reads back as the file it was stored as, never as an image.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_reads_a_file_back_as_a_file_whatever_its_media_type() {
    let (store, _) = connect().await;
    let prompt = Prompt::new(address("files"), "files", "system", "write").unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let files = [
        Media::File(file("notes.md", "text/plain")),
        Media::File(file("photo.png", "image/png")),
        Media::File(file("clip.mp4", "video/mp4")),
        Media::File(file("report.pdf", "application/pdf")),
    ];

    store.save_media(&claimed.lease, &files).await.unwrap();

    for item in &files {
        let read = store.media(id, &item.reference()).await.unwrap();
        assert_eq!(read.as_ref(), Some(item));
    }
    settle(&store, id, Some(claimed.lease)).await;
}

/// A subagent's capture is shown by its parent's reply.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_resolves_a_subagents_media_for_its_parent() {
    let (store, _) = connect().await;
    let runtime = Runtime::new(store.clone());
    let prompt = Prompt::new(address("parent"), "parent", "system", "delegate").unwrap();
    let parent = runtime.accept(&prompt).await.unwrap().operation.id;
    let config = SubagentConfig::new("recorder", "record", ExecutionProfile::default()).unwrap();
    let request = SubagentRequest::new(parent, "spawn", "record it", config).unwrap();
    let child = store.spawn_subagent(&request).await.unwrap().agent;
    let claimed = store
        .claim_operation(child.operation.id, "child", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let captured = Media::File(file("recording.md", "text/markdown"));
    store
        .save_media(&claimed.lease, std::slice::from_ref(&captured))
        .await
        .unwrap();

    let shown = store.media(parent, &captured.reference()).await;

    assert_eq!(shown.unwrap(), Some(captured));
    settle(&store, child.operation.id, Some(claimed.lease)).await;
    settle(&store, parent, None).await;
}

/// Media past retention no longer resolves, and the next write prunes it.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_prunes_tool_media_past_retention() {
    let (store, pool) = connect().await;
    let prompt = Prompt::new(address("old"), "old", "system", "record").unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .save_media(&claimed.lease, &[Media::Image(image())])
        .await
        .unwrap();
    let reference = media::reference(&image());
    sqlx::query("UPDATE tool_media SET stored_at = now() - interval '8 days' WHERE reference = $1")
        .bind(&reference)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(store.media(id, &reference).await.unwrap(), None);
    // A later write prunes what aged out; it is another image, so the aged row
    // is not refreshed into staying.
    let later = Image::new(ImageMediaType::Png, vec![7, 7, 7], 1, 1).unwrap();
    store
        .save_media(&claimed.lease, &[Media::Image(later)])
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM tool_media WHERE reference = $1")
        .bind(&reference)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
    settle(&store, id, Some(claimed.lease)).await;
}
