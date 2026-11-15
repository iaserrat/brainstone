#![expect(
    clippy::unused_async_trait_impl,
    reason = "Test doubles retain execution on poll."
)]
//! Opt-in contract tests. The default test suite never connects to Postgres.
#![allow(clippy::too_many_lines)]

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{
    Event, ExecutionProfile, Image, ImageMediaType, Message, Model, ModelResponse, Reasoning,
    Registry, Sink, StopReason, Thread, Tool, ToolError, ToolRequest, UserImages,
};
use brainstone_runtime::{
    Address, AdmissionQuota, Admitted, Compaction, Config, DeliveryCheckpoint, DeliveryClaim,
    DeliveryLease, DeliveryStore, DeliveryTarget, InteractionRequest, InteractionStore, LaneStore,
    Memories, Memory, OperationId, OperationState, PostgresStore, Prompt, PromptContent, Runtime,
    ScheduleRequest, ScheduleStore, ScheduleTime, Store, SubagentConfig, SubagentRequest,
    SubagentStore,
};
use serde_json::{Value, json};
use tokio::sync::Notify;

struct Answer(Mutex<Option<ModelResponse>>);

struct Capture(Mutex<Vec<Message>>);

struct Blocking {
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

struct SteeringBlock {
    calls: Arc<std::sync::atomic::AtomicUsize>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
}

struct Streaming;

struct Definitions(Mutex<Vec<Value>>);

struct NamedTool;

fn image_prompt(
    address: Address,
    request_id: &str,
    system: &str,
    images: Vec<Image>,
) -> Result<Prompt, brainstone_runtime::Error> {
    let content = PromptContent {
        images,
        ..PromptContent::default()
    };
    Prompt::with_content(address, request_id, system, content)
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_reclaims_an_exact_multimodal_prompt_after_lease_expiry() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "images").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let prompt = image_prompt(address.clone(), "image", "system", vec![image.clone()]).unwrap();
    let accepted = store.accept(&prompt).await.unwrap();
    assert!(
        store
            .request_exists(prompt.address(), prompt.request_id())
            .await
            .unwrap()
    );
    assert!(store.accept(&prompt).await.unwrap().duplicate);
    let changed = image_prompt(
        address,
        "image",
        "system",
        vec![Image::new(ImageMediaType::Png, vec![4], 2, 3).unwrap()],
    )
    .unwrap();
    assert_eq!(
        store.accept(&changed).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    let first = store
        .claim_operation(accepted.operation.id, "crashed", Duration::from_millis(1))
        .await
        .unwrap()
        .unwrap();
    drop(first);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let reclaimed = store
        .claim_operation(accepted.operation.id, "recovery", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reclaimed.thread.messages(),
        &[Message::UserImages(
            UserImages::new("", vec![image]).unwrap()
        )]
    );
    finish(&store, &reclaimed.lease).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn lane_following_survives_reconnect_and_can_resume() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("test", &unique_suffix(), "thread").unwrap();
    let prompt = Prompt::new(address.clone(), "request", "system", "leave").unwrap();
    let operation = store.accept(&prompt).await.unwrap().operation.id;

    // Retry identities are unique across the database, as production's
    // `operation/{id}/...` keys are, so a reused database never collides.
    let stop_1 = format!("operation/{}/stop-1", operation.get());
    let stop_2 = format!("operation/{}/stop-2", operation.get());
    store.unfollow_lane(operation, &stop_1).await.unwrap();
    assert!(!store.lane_is_followed(&address).await.unwrap());
    store.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopened store");
    assert!(!reopened.lane_is_followed(&address).await.unwrap());
    reopened.follow_lane(&address, "mention").await.unwrap();
    assert!(reopened.lane_is_followed(&address).await.unwrap());
    reopened.unfollow_lane(operation, &stop_1).await.unwrap();
    assert!(reopened.lane_is_followed(&address).await.unwrap());
    reopened.unfollow_lane(operation, &stop_2).await.unwrap();
    reopened.follow_lane(&address, "mention").await.unwrap();
    assert!(!reopened.lane_is_followed(&address).await.unwrap());
    reopened.follow_lane(&address, "new-mention").await.unwrap();
    assert!(reopened.lane_is_followed(&address).await.unwrap());
    let claimed = reopened
        .claim_operation(operation, "cleanup", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    reopened
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn an_idle_postgres_lane_adopts_a_new_turn_budget_after_reconnect() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("contract", &unique_suffix(), "profile").unwrap();
    let old = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "one")
        .unwrap()
        .with_profile(old);
    let Admitted::Operation(first) = store.admit_with_tools(&first, &[]).await.unwrap() else {
        panic!("first prompt must create an operation");
    };
    let claimed = store
        .claim_operation(first.operation.id, "first", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    store.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopened store");
    let new = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let next = Prompt::new(address, "next", "system", "two")
        .unwrap()
        .with_profile(new);
    let Admitted::Operation(next) = reopened.admit_with_tools(&next, &[]).await.unwrap() else {
        panic!("idle lane must accept a new operation");
    };
    let claimed = reopened
        .claim_operation(next.operation.id, "next", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.thread.profile().max_turns(), Some(100));
    finish(&reopened, &claimed.lease).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn a_postgres_lane_keeps_its_system_prompt_after_reconnect() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("contract", &unique_suffix(), "system").unwrap();
    let old = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let first = Prompt::new(address.clone(), "first", "original system", "one")
        .unwrap()
        .with_profile(old);
    let Admitted::Operation(first) = store.admit_with_tools(&first, &[]).await.unwrap() else {
        panic!("first prompt must create an operation");
    };
    let claimed = store
        .claim_operation(first.operation.id, "first", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.thread.system, "original system");
    assert_eq!(claimed.thread.profile().max_turns(), Some(25));
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    store.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopened store");
    let new = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let replay = Prompt::new(address.clone(), "first", "replacement system", "one")
        .unwrap()
        .with_profile(new.clone());
    let Admitted::Operation(replayed) = reopened.admit_with_tools(&replay, &[]).await.unwrap()
    else {
        panic!("redelivery must return the original operation");
    };
    assert!(replayed.duplicate);
    assert_eq!(replayed.operation.id, first.operation.id);
    let next = Prompt::new(address, "next", "replacement system", "two")
        .unwrap()
        .with_profile(new);
    let Admitted::Operation(next) = reopened.admit_with_tools(&next, &[]).await.unwrap() else {
        panic!("idle lane must accept a new operation");
    };
    let claimed = reopened
        .claim_operation(next.operation.id, "next", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(claimed.thread.system, "original system");
    assert_eq!(claimed.thread.profile().max_turns(), Some(100));
    finish(&reopened, &claimed.lease).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_does_not_inherit_multi_image_prompts_without_a_surface_cap() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "provider-images").unwrap();
    let images = (0..5)
        .map(|byte| Image::new(ImageMediaType::Png, vec![byte], 1, 1).unwrap())
        .collect();
    let prompt = image_prompt(address, "five-images", "system", images).unwrap();

    let id = store
        .accept(&prompt)
        .await
        .expect("provider-neutral images")
        .operation
        .id;
    let claimed = store
        .claim_operation(id, "cleanup", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
}

/// A write the database refuses, here a NUL byte that `TEXT` cannot hold, fails
/// the same way on every attempt, so it must not read as a transient outage
/// that workers retry. The pool is still reachable, so an outage reads as one.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_tells_a_refused_write_from_an_unreachable_store() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("contract", &unique_suffix(), "refused").unwrap();
    let prompt = Prompt::new(address, "refused", "system", "nul \u{0} byte").unwrap();

    let refused = store.accept(&prompt).await.expect_err("TEXT refuses NUL");
    assert_eq!(refused.code(), "store_failed");

    store.close().await;
    let closed = store.ping().await.expect_err("closed pool");
    assert_eq!(closed.code(), "store_unavailable");
}

/// A tool finds its thread through the operation's durable delivery record.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_reports_an_operations_delivery_target() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("contract", &unique_suffix(), "routed").unwrap();
    let target =
        DeliveryTarget::new("test", json!({"conversation": "C1", "post_id": "1.0"})).unwrap();
    let routed = Prompt::new(address.clone(), "routed", "system", "hello")
        .unwrap()
        .with_delivery(target.clone());
    let bare = Prompt::new(address, "bare", "system", "hello").unwrap();
    let routed = store.accept(&routed).await.unwrap().operation.id;
    let bare = store.accept(&bare).await.unwrap().operation.id;

    assert_eq!(store.delivery_target(routed).await.unwrap(), Some(target));
    assert_eq!(store.delivery_target(bare).await.unwrap(), None);
    for id in [routed, bare] {
        let claimed = store
            .claim_operation(id, "settle", Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let done = agent::Outcome::Completed {
            text: String::new(),
        };
        store.settle(&claimed.lease, &done).await.unwrap();
    }
}

/// A crashed worker's fetched file comes back byte-exact inside its tool
/// result, and only the event carrying binaries is written at schema 2, which
/// a pre-attachment binary rejects instead of replaying the result without it.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_reclaims_tool_result_attachments_behind_a_schema_fence() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let address = Address::new("contract", &unique_suffix(), "tool-files").unwrap();
    let prompt = Prompt::new(address, "fetch", "system", "read the screenshot").unwrap();
    let accepted = store.accept(&prompt).await.unwrap();
    let id = accepted.operation.id;
    let first = store
        .claim_operation(id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let call = agent::ToolCall {
        id: "call-1".to_owned(),
        name: "ReadThreadFile".to_owned(),
        args: json!({"file_id": "F1"}),
    };
    let asked = Message::Assistant(ModelResponse {
        tool_calls: vec![call],
        stop: StopReason::ToolUse,
        ..response("")
    });
    let result = agent::ToolResult {
        id: "call-1".to_owned(),
        name: "ReadThreadFile".to_owned(),
        content: r#"{"outcome":"file_read","name":"écran ☕.png"}"#.to_owned(),
        is_error: false,
        images: vec![Image::new(ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap()],
    };
    let fetched = Message::ToolResults(vec![result]);
    for message in [asked.clone(), fetched.clone()] {
        let event = Event::Appended { message };
        store.record(&first.lease, &event).await.unwrap();
    }
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let versions: Vec<i16> = sqlx::query_scalar(
        "SELECT schema_version FROM operation_events WHERE operation_id = $1 ORDER BY sequence",
    )
    .bind(id.get())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(versions, vec![1, 2]);
    sqlx::query("UPDATE sessions SET lease_expires_at = clock_timestamp() WHERE id = $1")
        .bind(first.lease.session_id.get())
        .execute(&pool)
        .await
        .unwrap();
    let reclaimed = store
        .claim_operation(id, "recovery", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reclaimed.thread.messages(),
        &[
            Message::User("read the screenshot".to_owned()),
            asked,
            fetched
        ]
    );
    let done = agent::Outcome::Completed {
        text: String::new(),
    };
    store.settle(&reclaimed.lease, &done).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_rejects_an_image_claim_from_an_old_binary() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "claim-version").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1], 1, 1).unwrap();
    let prompt = image_prompt(address, "image", "system", vec![image]).unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    let lane: i64 = sqlx::query_scalar("SELECT lane_id FROM operations WHERE id = $1")
        .bind(id.get())
        .fetch_one(&pool)
        .await
        .unwrap();
    let old_accept = sqlx::query(
        "INSERT INTO operations (lane_id, request_id, prompt, state) \
         VALUES ($1, 'old-writer', 'text only', 'accepted')",
    )
    .bind(lane)
    .execute(&pool)
    .await;
    assert!(old_accept.is_err(), "an old writer must fail closed");

    let old_claim =
        sqlx::query("UPDATE operations SET state = 'running', lease_token = 1 WHERE id = $1")
            .bind(id.get())
            .execute(&pool)
            .await;

    assert!(old_claim.is_err(), "an old claimant must fail closed");
    let claimed = store
        .claim_operation(id, "image-capable", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_rejects_corrupt_image_ordinals() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "corrupt-images").unwrap();
    let prompt = Prompt::new(address, "corrupt", "system", "text").unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("INSERT INTO operation_images (operation_id, ordinal, media_type, bytes, width, height) VALUES ($1, 1, 'image/png', $2, 1, 1)")
        .bind(id.get()).bind(vec![1_u8]).execute(&pool).await.unwrap();
    let error = store
        .claim_operation(id, "worker", Duration::from_secs(30))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "corrupt_record");
    sqlx::query("DELETE FROM operation_images WHERE operation_id = $1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    let claimed = store
        .claim_operation(id, "cleanup", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_rejects_missing_initial_and_steering_images() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "missing-images").unwrap();
    let images = vec![
        Image::new(ImageMediaType::Png, vec![1], 1, 1).unwrap(),
        Image::new(ImageMediaType::Png, vec![2], 1, 1).unwrap(),
        Image::new(ImageMediaType::Png, vec![3], 1, 1).unwrap(),
    ];
    let prompt = image_prompt(address.clone(), "missing", "system", images).unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("DELETE FROM operation_images WHERE operation_id = $1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .claim_operation(id, "worker", Duration::from_secs(30))
            .await
            .unwrap_err()
            .code(),
        "corrupt_record"
    );
    for (ordinal, byte) in [1_u8, 2, 3].into_iter().enumerate() {
        sqlx::query("INSERT INTO operation_images (operation_id, ordinal, media_type, bytes, width, height) VALUES ($1, $2, 'image/png', $3, 1, 1)")
            .bind(id.get()).bind(i16::try_from(ordinal).unwrap()).bind(vec![byte])
            .execute(&pool).await.unwrap();
    }
    sqlx::query("DELETE FROM operation_images WHERE operation_id = $1 AND ordinal = 1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .claim_operation(id, "worker", Duration::from_secs(30))
            .await
            .unwrap_err()
            .code(),
        "corrupt_record"
    );
    sqlx::query("DELETE FROM operation_images WHERE operation_id = $1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE operations SET image_count = 0 WHERE id = $1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap();
    let claimed = store
        .claim_operation(id, "cleanup", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let steer = image_prompt(
        address,
        "missing-steer",
        "system",
        vec![Image::new(ImageMediaType::Png, vec![4], 1, 1).unwrap()],
    )
    .unwrap();
    store.admit_with_tools(&steer, &[]).await.unwrap();
    let steering: i64 =
        sqlx::query_scalar("SELECT id FROM operation_steering WHERE request_id = 'missing-steer'")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("DELETE FROM operation_steering_images WHERE steering_id = $1")
        .bind(steering)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        store.steering(&claimed.lease).await.unwrap_err().code(),
        "corrupt_record"
    );
    sqlx::query("DELETE FROM operation_steering WHERE id = $1")
        .bind(steering)
        .execute(&pool)
        .await
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_compaction_invalidates_older_image_bodies() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "compacted-images").unwrap();
    let image = Image::new(ImageMediaType::Png, vec![1, 2, 3], 1, 1).unwrap();
    let prompt = image_prompt(address.clone(), "compacted", "system", vec![image]).unwrap();
    let accepted = store.accept(&prompt).await.unwrap();
    let claimed = store
        .claim_operation(accepted.operation.id, "crashed", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let steer = image_prompt(
        prompt.address().clone(),
        "compacted-steer",
        "system",
        vec![Image::new(ImageMediaType::Png, vec![4], 1, 1).unwrap()],
    )
    .unwrap();
    store.admit_with_tools(&steer, &[]).await.unwrap();
    store.steering(&claimed.lease).await.unwrap();
    store
        .record(
            &claimed.lease,
            &Event::Compacted {
                summary: "image summary".to_owned(),
                steps: 0,
            },
        )
        .await
        .unwrap();
    store
        .record(
            &claimed.lease,
            &Event::Appended {
                message: Message::User("after compaction".to_owned()),
            },
        )
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query(
        "UPDATE operation_events SET schema_version = 99 WHERE operation_id = $1 AND sequence = 0",
    )
    .bind(accepted.operation.id.get())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM operation_images WHERE operation_id = $1")
        .bind(accepted.operation.id.get())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM operation_steering_images WHERE steering_id IN \
         (SELECT id FROM operation_steering WHERE operation_id = $1)",
    )
    .bind(accepted.operation.id.get())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions SET lease_expires_at = clock_timestamp() - INTERVAL '1 second' \
         WHERE id = (SELECT l.session_id FROM operations o JOIN lanes l ON l.id = o.lane_id \
         WHERE o.id = $1)",
    )
    .bind(accepted.operation.id.get())
    .execute(&pool)
    .await
    .unwrap();

    let reclaimed = store
        .claim_operation(accepted.operation.id, "recovery", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reclaimed.thread.messages(),
        &[
            Message::Summary("image summary".to_owned()),
            Message::User("after compaction".to_owned()),
        ]
    );
    store
        .settle(
            &reclaimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
    let later = Prompt::new(address, "later", "system", "later operation").unwrap();
    let later = store.accept(&later).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(later, "later-worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed.thread.messages(),
        &[
            Message::Summary("image summary".to_owned()),
            Message::User("after compaction".to_owned()),
            Message::User("later operation".to_owned()),
        ]
    );
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: String::new(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_active_steering_preserves_images_and_legacy_text() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "steering-images").unwrap();
    let first = Prompt::new(address.clone(), "legacy", "system", "start").unwrap();
    let accepted = store.accept(&first).await.unwrap();
    let claimed = store
        .claim_operation(accepted.operation.id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed.thread.messages(),
        &[Message::User("start".to_owned())]
    );
    let image = Image::new(ImageMediaType::Webp, vec![1, 2, 3], 2, 3).unwrap();
    let steer = image_prompt(address, "steer", "system", vec![image.clone()]).unwrap();
    store.admit_with_tools(&steer, &[]).await.unwrap();
    assert!(
        store
            .request_exists(steer.address(), steer.request_id())
            .await
            .unwrap()
    );
    assert_eq!(
        store.steering(&claimed.lease).await.unwrap(),
        vec![Message::UserImages(
            UserImages::new("", vec![image]).unwrap()
        )]
    );
    finish(&store, &claimed.lease).await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_schedule_release_locks_the_lane_before_inserting() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "insert-fence").unwrap();
    let target = DeliveryTarget::new("test", json!({"conversation":"C1"})).unwrap();
    let origin = Prompt::new(address, "origin", "system", "A")
        .unwrap()
        .with_delivery(target);
    let origin = store.accept(&origin).await.unwrap().operation.id;
    Runtime::new(store.clone())
        .run_next(&answer("done"), &Registry::default())
        .await
        .unwrap();
    store
        .schedule(
            &ScheduleRequest::new(
                origin,
                "insert-fence",
                "scheduled",
                ScheduleTime::AfterSeconds(0),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let before: i64 = sqlx::query_scalar("SELECT last_value FROM operations_id_seq")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut transaction = pool.begin().await.expect("lane lock transaction");
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await
        .expect("blocker pid");
    sqlx::query(
        "SELECT l.id FROM lanes l JOIN sessions s ON s.id = l.session_id \
        WHERE s.external_id = $1 AND l.external_id = 'insert-fence' FOR UPDATE OF l",
    )
    .bind(&suffix)
    .execute(&mut *transaction)
    .await
    .expect("lane locked");
    let release_store = store.clone();
    let release_task = tokio::spawn(async move {
        release_store
            .claim("releaser", Duration::from_secs(1))
            .await
    });
    wait_for_database_lock(&pool, blocker).await;
    let after: i64 = sqlx::query_scalar("SELECT last_value FROM operations_id_seq")
        .fetch_one(&pool)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    let claimed = release_task
        .await
        .unwrap()
        .unwrap()
        .expect("released operation");
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "operation inserted before taking the lane fence"
    );
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_acceptance_does_not_hold_session_while_waiting_for_lane() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "lock-order").unwrap();
    let original = Prompt::new(address.clone(), "original", "system", "start").unwrap();
    let original = store.accept(&original).await.unwrap().operation.id;
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let (session, lane): (i64, i64) = sqlx::query_as(
        "SELECT s.id, l.id FROM sessions s JOIN lanes l ON l.session_id = s.id \
         WHERE s.external_id = $1 AND l.external_id = 'lock-order'",
    )
    .bind(&suffix)
    .fetch_one(&pool)
    .await
    .expect("database ids");
    let mut transaction = pool.begin().await.expect("session lock transaction");
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await
        .expect("blocker pid");
    sqlx::query("SELECT id FROM lanes WHERE id = $1 FOR UPDATE")
        .bind(lane)
        .execute(&mut *transaction)
        .await
        .expect("lane locked");
    let next = Prompt::new(address, "next", "system", "continue").unwrap();
    let accepting_store = store.clone();
    let accepting = tokio::spawn(async move { accepting_store.accept(&next).await });
    wait_for_database_lock(&pool, blocker).await;
    sqlx::query("SET LOCAL lock_timeout = '250ms'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let session_lock = sqlx::query("SELECT id FROM sessions WHERE id = $1 FOR UPDATE")
        .bind(session)
        .execute(&mut *transaction)
        .await;
    transaction.rollback().await.unwrap();
    let next = tokio::time::timeout(Duration::from_secs(1), accepting)
        .await
        .expect("acceptance resumed")
        .unwrap()
        .unwrap()
        .operation
        .id;
    assert!(
        session_lock.is_ok(),
        "acceptance held the session while waiting for the lane: {:?}",
        session_lock.err()
    );
    let runtime = Runtime::new(store);
    runtime
        .run_operation(original, &answer("done"), &Registry::default())
        .await
        .unwrap();
    runtime
        .run_operation(next, &answer("done"), &Registry::default())
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_steering_does_not_hold_session_while_waiting_for_operation() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "operation-order").unwrap();
    let prompt = Prompt::new(address, "original", "system", "start").unwrap();
    let operation = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(operation, "worker", Duration::from_secs(10))
        .await
        .unwrap()
        .expect("claimed operation");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let session: i64 = sqlx::query_scalar(
        "SELECT l.session_id FROM operations o JOIN lanes l ON l.id = o.lane_id WHERE o.id = $1",
    )
    .bind(operation.get())
    .fetch_one(&pool)
    .await
    .expect("session id");
    let mut transaction = pool.begin().await.expect("operation lock transaction");
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await
        .expect("blocker pid");
    sqlx::query("SELECT id FROM operations WHERE id = $1 FOR UPDATE")
        .bind(operation.get())
        .execute(&mut *transaction)
        .await
        .expect("operation locked");
    let steering_store = store.clone();
    let lease = claimed.lease.clone();
    let steering = tokio::spawn(async move { steering_store.steering(&lease).await });
    wait_for_database_lock(&pool, blocker).await;
    sqlx::query("SET LOCAL lock_timeout = '250ms'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let session_lock = sqlx::query("SELECT id FROM sessions WHERE id = $1 FOR UPDATE")
        .bind(session)
        .execute(&mut *transaction)
        .await;
    transaction.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), steering)
        .await
        .expect("steering resumed")
        .unwrap()
        .unwrap();
    assert!(
        session_lock.is_ok(),
        "steering held the session while waiting for the operation: {:?}",
        session_lock.err()
    );
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_admission_cannot_deadlock_with_steering() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "admit-order").unwrap();
    let prompt = Prompt::new(address, "original", "system", "start").unwrap();
    let operation = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(operation, "worker", Duration::from_secs(10))
        .await
        .unwrap()
        .expect("claimed operation");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let lane: i64 = sqlx::query_scalar("SELECT lane_id FROM operations WHERE id = $1")
        .bind(operation.get())
        .fetch_one(&pool)
        .await
        .expect("lane id");
    let mut transaction = pool.begin().await.expect("lane lock transaction");
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await
        .expect("blocker pid");
    sqlx::query("SELECT id FROM lanes WHERE id = $1 FOR UPDATE")
        .bind(lane)
        .execute(&mut *transaction)
        .await
        .expect("lane locked");
    let steering_store = store.clone();
    let lease = claimed.lease.clone();
    let steering = tokio::spawn(async move { steering_store.steering(&lease).await });
    wait_for_database_lock(&pool, blocker).await;
    sqlx::query("SET LOCAL lock_timeout = '250ms'")
        .execute(&mut *transaction)
        .await
        .unwrap();
    let inserted = sqlx::query(
        "INSERT INTO operation_steering (lane_id, operation_id, request_id, prompt, source_prompt, \
         image_count) VALUES ($1, $2, 'lock-probe', 'follow up', 'follow up', 0)",
    )
    .bind(lane)
    .bind(operation.get())
    .execute(&mut *transaction)
    .await;
    transaction.rollback().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), steering)
        .await
        .expect("steering resumed")
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &agent::Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    assert!(
        inserted.is_ok(),
        "admission foreign key waited on steering's operation lock: {:?}",
        inserted.err()
    );
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_repairs_an_invalid_concurrent_index() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url)
        .await
        .expect("initial migration");
    store.close().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    leave_invalid_representative_index(&pool).await;
    pool.close().await;

    let store = PostgresStore::connect(&url)
        .await
        .expect("migration recovery");
    store.close().await;
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("verification pool");
    let valid: bool = sqlx::query_scalar(INDEX_VALIDITY_SQL)
        .fetch_one(&pool)
        .await
        .expect("index validity");
    assert!(valid);
    sqlx::query("DROP TABLE migration_recovery_fixture")
        .execute(&pool)
        .await
        .expect("fixture cleanup");
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_repairs_an_invalid_claimable_delivery_index() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url)
        .await
        .expect("initial migration");
    store.close().await;
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    leave_invalid_claimable_index(&pool).await;
    pool.close().await;

    let store = PostgresStore::connect(&url)
        .await
        .expect("migration recovery");
    store.close().await;
    let pool = sqlx::PgPool::connect(&url)
        .await
        .expect("verification pool");
    let valid: bool = sqlx::query_scalar(CLAIMABLE_INDEX_VALIDITY_SQL)
        .fetch_one(&pool)
        .await
        .expect("index validity");
    assert!(valid);
    sqlx::query("DROP TABLE migration_claimable_fixture")
        .execute(&pool)
        .await
        .expect("fixture cleanup");
}

async fn leave_invalid_representative_index(pool: &sqlx::PgPool) {
    sqlx::query("DROP INDEX CONCURRENTLY IF EXISTS operations_delivery_representative")
        .execute(pool)
        .await
        .expect("remove valid index");
    sqlx::query("DROP TABLE IF EXISTS migration_recovery_fixture")
        .execute(pool)
        .await
        .expect("reset fixture table");
    sqlx::query("CREATE TABLE migration_recovery_fixture (value INTEGER)")
        .execute(pool)
        .await
        .expect("fixture table");
    sqlx::query("INSERT INTO migration_recovery_fixture VALUES (1), (1)")
        .execute(pool)
        .await
        .expect("duplicate fixture rows");
    sqlx::query(INVALID_INDEX_SQL)
        .execute(pool)
        .await
        .expect_err("unique index build must fail");
}

async fn leave_invalid_claimable_index(pool: &sqlx::PgPool) {
    sqlx::query("DROP INDEX CONCURRENTLY IF EXISTS operation_deliveries_claimable")
        .execute(pool)
        .await
        .expect("remove valid index");
    sqlx::query("DROP TABLE IF EXISTS migration_claimable_fixture")
        .execute(pool)
        .await
        .expect("reset fixture table");
    sqlx::query("CREATE TABLE migration_claimable_fixture (value INTEGER)")
        .execute(pool)
        .await
        .expect("fixture table");
    sqlx::query("INSERT INTO migration_claimable_fixture VALUES (1), (1)")
        .execute(pool)
        .await
        .expect("duplicate fixture rows");
    sqlx::query(INVALID_CLAIMABLE_INDEX_SQL)
        .execute(pool)
        .await
        .expect_err("unique index build must fail");
}

async fn wait_for_database_lock(pool: &sqlx::PgPool, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database() \
                 AND $1 = ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(pool)
            .await
            .expect("lock state");
            if waiting {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("operation insert waited for the lane lock");
}

const INVALID_INDEX_SQL: &str = "CREATE UNIQUE INDEX CONCURRENTLY \
    operations_delivery_representative ON migration_recovery_fixture (value)";
const INVALID_CLAIMABLE_INDEX_SQL: &str = "CREATE UNIQUE INDEX CONCURRENTLY \
    operation_deliveries_claimable ON migration_claimable_fixture (value)";
const INDEX_VALIDITY_SQL: &str = "SELECT indisvalid FROM pg_index \
    WHERE indexrelid = 'operations_delivery_representative'::regclass";
const CLAIMABLE_INDEX_VALIDITY_SQL: &str = "SELECT indisvalid FROM pg_index \
    WHERE indexrelid = 'operation_deliveries_claimable'::regclass";

impl Tool for NamedTool {
    fn name(&self) -> &'static str {
        "mcp__test__search"
    }

    fn definition(&self) -> Value {
        json!({"name": self.name(), "description": "search", "input_schema": {"type":"object"}})
    }

    fn call<'a>(
        &'a self,
        _request: ToolRequest,
    ) -> Pin<Box<dyn Future<Output = Result<agent::ToolOutput, ToolError>> + Send + 'a>> {
        Box::pin(async { Ok("{}".to_owned().into()) })
    }
}

impl Model for Definitions {
    async fn generate(&self, _thread: &Thread, tools: &[Value]) -> ModelResponse {
        *self.0.lock().expect("definitions poisoned") = tools.to_vec();
        response("done")
    }
}

impl Model for Streaming {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        response("durable answer")
    }

    async fn generate_stream<S: Sink + Send>(
        &self,
        _thread: &Thread,
        _tools: &[Value],
        sink: &mut S,
    ) -> Result<ModelResponse, S::Error> {
        sink.emit(Event::ModelDelta {
            delta: "durable ".to_owned(),
        })
        .await?;
        Ok(response("durable answer"))
    }
}

impl Model for Blocking {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.entered.notify_one();
        self.release.notified().await;
        answer("done")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

impl Model for SteeringBlock {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        response("done")
    }
}

impl Model for Capture {
    async fn generate(&self, thread: &Thread, _tools: &[Value]) -> ModelResponse {
        *self.0.lock().expect("capture poisoned") = thread.messages().to_vec();
        answer("next answer")
            .0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("answer")
    }
}

impl Model for Answer {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.0
            .lock()
            .expect("script poisoned")
            .take()
            .expect("one call")
    }
}

fn answer(text: &str) -> Answer {
    Answer(Mutex::new(Some(response(text))))
}

fn response(text: &str) -> ModelResponse {
    ModelResponse {
        model: None,
        reasoning: Vec::new(),
        text: text.to_owned(),
        tool_calls: Vec::new(),
        stop: StopReason::EndTurn,
    }
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_fences_and_recovers_surface_delivery() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store_a = PostgresStore::connect(&url).await.expect("first pool");
    store_a.ping().await.expect("store is ready");
    let store_b = PostgresStore::connect(&url).await.expect("second pool");
    let runtime = Runtime::new(store_a.clone());
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "thread-delivery").expect("address");
    let adapter = format!("fenced-{suffix}");
    let target = DeliveryTarget::new(&adapter, json!({"conversation": "C123"})).expect("target");
    let prompt = Prompt::new(address, "event-delivery", "system", "prompt")
        .expect("prompt")
        .with_delivery(target);
    let accepted = runtime.accept(&prompt).await.expect("accepted");
    runtime
        .run_operation(accepted.operation.id, &Streaming, &Registry::default())
        .await
        .expect("run");

    // The renewal below replaces this expiry; the first lease only has to
    // outlive the round trips before it, whatever the machine's load.
    let ttl = Duration::from_secs(30);
    let first = DeliveryClaim::new(&adapter, "delivery-a", ttl, 32).expect("claim");
    let batch_a = store_a
        .claim_delivery(&first)
        .await
        .expect("first claim")
        .expect("events");
    assert!(matches!(batch_a.events[1].event, Event::ModelDelta { .. }));
    let recovery_ttl = Duration::from_secs(1);
    let second = DeliveryClaim::new(&adapter, "delivery-b", recovery_ttl, 32).expect("claim");
    assert!(
        store_b
            .claim_delivery(&second)
            .await
            .expect("fenced")
            .is_none()
    );
    store_a
        .save_delivery_stream(&batch_a.lease, Some("stream-a"))
        .await
        .expect("stream identity saved under a live lease");
    assert_eq!(
        store_a
            .renew_delivery(&batch_a.lease, Duration::MAX)
            .await
            .expect_err("an unrepresentable lease duration is invalid input")
            .code(),
        "invalid_input"
    );
    store_a
        .renew_delivery(&batch_a.lease, Duration::from_millis(100))
        .await
        .expect("live presentation lease renewed");

    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        store_b
            .claim_delivery(&second)
            .await
            .expect("renewed fence")
            .is_none()
    );
    tokio::time::sleep(Duration::from_millis(90)).await;
    let batch_b = store_b
        .claim_delivery(&second)
        .await
        .expect("recovery claim")
        .expect("replayed events");
    assert_eq!(batch_b.external_id.as_deref(), Some("stream-a"));
    assert_eq!(
        store_a
            .save_delivery_stream(&batch_a.lease, Some("stream-stale"))
            .await
            .expect_err("stale fence")
            .code(),
        "lease_lost"
    );
    assert_eq!(
        store_a
            .renew_delivery(&batch_a.lease, Duration::from_secs(1))
            .await
            .expect_err("stale renewal")
            .code(),
        "lease_lost"
    );
    let through = batch_b.events.last().expect("events").sequence;
    let stale = DeliveryCheckpoint::new(batch_a.lease, through, None);
    assert_eq!(
        store_a
            .checkpoint_delivery(&stale)
            .await
            .expect_err("stale fence")
            .code(),
        "lease_lost"
    );
    let invalid = DeliveryCheckpoint::new(batch_b.lease.clone(), through + 1, None);
    assert_eq!(
        store_b
            .checkpoint_delivery(&invalid)
            .await
            .expect_err("cursor beyond journal")
            .code(),
        "invalid_delivery_checkpoint"
    );
    store_b
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            batch_b.lease,
            through,
            Some("ts".to_owned()),
        ))
        .await
        .expect("checkpoint");
    assert!(
        store_a
            .claim_delivery(&first)
            .await
            .expect("drained")
            .is_none()
    );
    store_a.close().await;
    store_b.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_steering_immediately_invalidates_live_presentation() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "steering-presence").expect("address");
    let adapter = format!("steering-{suffix}");
    let target = DeliveryTarget::new(&adapter, json!({"conversation": "C123"})).expect("target");
    let initial = Prompt::new(address.clone(), "event-1", "system", "research")
        .expect("prompt")
        .with_delivery(target.clone());
    let operation = store.accept(&initial).await.expect("accepted").operation.id;
    let delivery = DeliveryClaim::new(&adapter, "surface", Duration::from_secs(30), 32)
        .expect("claim")
        .with_presence(Duration::from_hours(1))
        .expect("presence");
    let waiting = store.claim_delivery(&delivery).await.unwrap().unwrap();
    assert_eq!(waiting.events, []);
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(waiting.lease))
        .await
        .expect("waiting operation presented");
    let claimed = store
        .claim_operation(operation, "runner", Duration::from_secs(30))
        .await
        .expect("claim")
        .expect("operation");
    store
        .record(&claimed.lease, &Event::RunStarted)
        .await
        .expect("started");
    let started = store.claim_delivery(&delivery).await.unwrap().unwrap();
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            started.lease,
            started.events[0].sequence,
            None,
        ))
        .await
        .expect("presented");
    let steering = Prompt::new(address.clone(), "event-2", "system", "focus on failures")
        .expect("steering")
        .with_delivery(target.clone());

    store
        .admit_with_tools(&steering, &[])
        .await
        .expect("admitted");

    let live = store
        .claim_delivery(&delivery)
        .await
        .expect("claim")
        .expect("presentation due");
    assert_eq!(live.events, []);
    let next = Prompt::new(address, "event-3", "system", "and retry evidence")
        .expect("steering")
        .with_delivery(target);
    store
        .admit_with_tools(&next, &[])
        .await
        .expect("second steering");
    let replacement = DeliveryClaim::new(&adapter, "surface-2", Duration::from_secs(30), 32)
        .expect("claim")
        .with_presence(Duration::from_hours(1))
        .expect("presence");
    let replacement = store.claim_delivery(&replacement).await.unwrap().unwrap();
    assert_eq!(
        store
            .checkpoint_delivery(&DeliveryCheckpoint::presented(live.lease))
            .await
            .expect_err("stale presentation")
            .code(),
        "lease_lost"
    );
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(replacement.lease))
        .await
        .unwrap();
    store.steering(&claimed.lease).await.unwrap();
    finish(&store, &claimed.lease).await;
    checkpoint_postgres_next(&store, &delivery).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_terminal_delivery_represents_a_live_lane_sibling() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let suffix = unique_suffix();
    let fixture = accept_presented_siblings(&store, &suffix).await;
    finish_postgres_operation(&store, fixture.first).await;
    let terminal = store
        .claim_delivery(&fixture.delivery)
        .await
        .unwrap()
        .unwrap();
    assert!(
        store
            .claim_delivery(&fixture.delivery)
            .await
            .unwrap()
            .is_none()
    );
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query(
        "UPDATE operation_deliveries SET lease_owner = 'old-worker', \
         lease_token = lease_token + 1, lease_expires_at = clock_timestamp() + INTERVAL '1 hour', \
         presented_at = clock_timestamp(), external_id = 'stream-b' WHERE operation_id = $1",
    )
    .bind(fixture.sibling.get())
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    let through = terminal.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(terminal.lease, through, None))
        .await
        .unwrap();
    store.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopen");
    let replacement = reopened
        .claim_delivery(&fixture.delivery)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replacement.lease.operation(), fixture.sibling);
    assert_eq!(replacement.events, []);
    assert_eq!(replacement.external_id.as_deref(), Some("stream-b"));
    assert!(
        reopened
            .claim_delivery(&fixture.other)
            .await
            .unwrap()
            .is_none()
    );
    reopened
        .checkpoint_delivery(&DeliveryCheckpoint::presented(replacement.lease))
        .await
        .unwrap();
    finish_postgres_operation(&reopened, fixture.sibling).await;
    checkpoint_postgres_next(&reopened, &fixture.delivery).await;
    finish_postgres_operation(&reopened, fixture.third).await;
    checkpoint_postgres_next(&reopened, &fixture.other).await;
    reopened.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_terminal_checkpoint_settles_delivery_without_resurrection() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let suffix = unique_suffix();
    let adapter = format!("settled-{suffix}");
    let address = Address::new("contract", &suffix, "settled-delivery").unwrap();
    let prompt = Prompt::new(address, "terminal", "system", "finish")
        .unwrap()
        .with_delivery(DeliveryTarget::new(&adapter, json!({})).unwrap());
    let operation = store.accept(&prompt).await.unwrap().operation.id;
    finish_postgres_operation(&store, operation).await;
    let claim = DeliveryClaim::new(&adapter, "surface", Duration::from_secs(30), 32).unwrap();
    checkpoint_postgres_next(&store, &claim).await;

    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let settled: bool =
        sqlx::query_scalar("SELECT settled FROM operation_deliveries WHERE operation_id = $1")
            .bind(operation.get())
            .fetch_one(&pool)
            .await
            .expect("delivery tombstone");
    assert!(settled);
    pool.close().await;
    store.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopen");
    let retried = reopened.accept(&prompt).await.unwrap().operation.id;
    assert_eq!(retried, operation);
    assert!(reopened.claim_delivery(&claim).await.unwrap().is_none());
    reopened.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_same_lane_terminal_deliveries_wait_for_their_predecessor() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let suffix = unique_suffix();
    let adapter = format!("ordered-{suffix}");
    let address = Address::new("contract", &suffix, "ordered-delivery").unwrap();
    let target = DeliveryTarget::new(&adapter, json!({})).unwrap();
    let first = Prompt::new(address.clone(), "ordered-a", "system", "A")
        .unwrap()
        .with_delivery(target.clone());
    let second = Prompt::new(address, "ordered-b", "system", "B")
        .unwrap()
        .with_delivery(target);
    let first_id = store.accept(&first).await.unwrap().operation.id;
    finish_postgres_operation(&store, first_id).await;
    let second_id = store.accept(&second).await.unwrap().operation.id;
    finish_postgres_operation(&store, second_id).await;
    let claim = DeliveryClaim::new(&adapter, "surface", Duration::from_secs(30), 32).unwrap();

    let predecessor = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(predecessor.lease.operation(), first_id);
    assert!(store.claim_delivery(&claim).await.unwrap().is_none());
    let through = predecessor.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(predecessor.lease, through, None))
        .await
        .unwrap();
    // An older binary checkpoints the cursor but cannot set the new tombstone.
    let pool = sqlx::PgPool::connect(&url).await.unwrap();
    sqlx::query("UPDATE operation_deliveries SET settled = FALSE WHERE operation_id = $1")
        .bind(first_id.get())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let successor = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(successor.lease.operation(), second_id);
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_event_preempts_an_empty_presence_lease() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let suffix = unique_suffix();
    let (adapter, operation, stale) = accepted_postgres_presence(&store, &suffix).await;
    let run = store
        .claim_operation(operation, "runner", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store.record(&run.lease, &Event::RunStarted).await.unwrap();
    let replacement = postgres_presence_claim(&adapter);

    let started = store.claim_delivery(&replacement).await.unwrap().unwrap();
    assert!(matches!(started.events[0].event, Event::RunStarted));
    let error = store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(stale))
        .await
        .expect_err("presence lease is stale");
    assert_eq!(error.code(), "lease_lost", "{error}");
    let through = started.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(started.lease, through, None))
        .await
        .unwrap();
    store
        .record(
            &run.lease,
            &Event::RunFinished {
                outcome: agent::Outcome::Completed {
                    text: "done".to_owned(),
                },
            },
        )
        .await
        .unwrap();
    checkpoint_postgres_next(&store, &replacement).await;
    store.close().await;
}

async fn accepted_postgres_presence(
    store: &PostgresStore,
    suffix: &str,
) -> (String, OperationId, DeliveryLease) {
    let adapter = format!("event-{suffix}");
    let address = Address::new("contract", suffix, "event-preemption").unwrap();
    let prompt = Prompt::new(address, "first", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new(&adapter, json!({})).unwrap());
    let operation = store.accept(&prompt).await.unwrap().operation.id;
    let claim = postgres_presence_claim(&adapter);
    let batch = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(batch.events, []);
    (adapter, operation, batch.lease)
}

struct PresentedSiblings {
    delivery: DeliveryClaim,
    other: DeliveryClaim,
    first: OperationId,
    sibling: OperationId,
    third: OperationId,
}

async fn accept_presented_siblings(store: &PostgresStore, suffix: &str) -> PresentedSiblings {
    let adapter = format!("contract-{suffix}");
    let other = format!("other-{suffix}");
    let [first, second, third] = sibling_prompts(suffix, &adapter, &other);
    let first = store.accept(&first).await.unwrap().operation.id;
    let sibling = store.accept(&second).await.unwrap().operation.id;
    let third = store.accept(&third).await.unwrap().operation.id;
    let claim = postgres_presence_claim(&adapter);
    present_postgres(store, &claim, 1).await;
    assert!(
        store.claim_delivery(&claim).await.unwrap().is_none(),
        "one lane and adapter has one live presence representative"
    );
    let other = postgres_presence_claim(&other);
    present_postgres(store, &other, 1).await;
    PresentedSiblings {
        delivery: claim,
        other,
        first,
        sibling,
        third,
    }
}

fn sibling_prompts(suffix: &str, adapter: &str, other: &str) -> [Prompt; 3] {
    let address = Address::new("contract", suffix, "shared-lane").unwrap();
    let first = Prompt::new(address.clone(), "first", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new(adapter, json!({"recipient": "one"})).unwrap());
    let second = Prompt::new(address.clone(), "second", "system", "B")
        .unwrap()
        .with_delivery(DeliveryTarget::new(adapter, json!({"recipient": "two"})).unwrap());
    let third = Prompt::new(address, "third", "system", "C")
        .unwrap()
        .with_delivery(DeliveryTarget::new(other, json!({})).unwrap());
    [first, second, third]
}

async fn present_postgres(store: &PostgresStore, claim: &DeliveryClaim, count: usize) {
    for _ in 0..count {
        let batch = store.claim_delivery(claim).await.unwrap().unwrap();
        store
            .checkpoint_delivery(&DeliveryCheckpoint::presented(batch.lease))
            .await
            .unwrap();
    }
}

fn postgres_presence_claim(adapter: &str) -> DeliveryClaim {
    DeliveryClaim::new(adapter, "surface", Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(Duration::from_hours(1))
        .unwrap()
}

async fn finish_postgres_operation(store: &PostgresStore, id: OperationId) {
    let run = store
        .claim_operation(id, "runner", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store.record(&run.lease, &Event::RunStarted).await.unwrap();
    store
        .record(
            &run.lease,
            &Event::RunFinished {
                outcome: agent::Outcome::Completed {
                    text: "done".to_owned(),
                },
            },
        )
        .await
        .unwrap();
}

async fn checkpoint_postgres_next(store: &PostgresStore, claim: &DeliveryClaim) {
    let batch = store.claim_delivery(claim).await.unwrap().unwrap();
    let through = batch.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(batch.lease, through, None))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_acceptance_matches_the_store_contract() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url)
        .await
        .expect("connect and migrate");
    let runtime = Runtime::new(store.clone());
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "thread-42").expect("valid address");
    let prompt = Prompt::new(address, "event-7", "be useful", "héllo ☕").expect("valid");

    let accepted = runtime.accept(&prompt).await.expect("accepted");
    let retry = runtime.accept(&prompt).await.expect("idempotent retry");
    let restored = runtime
        .operation(accepted.operation.id)
        .await
        .expect("read")
        .expect("stored");

    assert_eq!(accepted.operation.id, retry.operation.id);
    assert_eq!(accepted.operation.session_id, restored.session_id);
    assert_eq!(accepted.operation.lane_id, restored.lane_id);
    assert!(retry.duplicate);
    assert_eq!(restored.state, OperationState::Accepted);
    let changed = Prompt::new(
        prompt.address().clone(),
        "event-7",
        "replacement system",
        "héllo ☕",
    )
    .expect("changed configuration");
    assert_eq!(
        runtime.accept(&changed).await.unwrap_err(),
        brainstone_runtime::Error::LaneConfigurationConflict
    );

    let finished = runtime
        .run_operation(
            accepted.operation.id,
            &answer("durable answer"),
            &Registry::default(),
        )
        .await
        .expect("run")
        .expect("pending operation");
    assert_eq!(finished.state, OperationState::Completed);
    assert!(finished.outcome.is_some());
    let completed_retry = runtime.accept(&prompt).await.expect("completed retry");
    assert!(completed_retry.duplicate);
    assert_eq!(completed_retry.operation, finished);

    runtime
        .compact(&Compaction::new(
            finished.lane_id,
            finished.id,
            "Earlier greeting",
        ))
        .await
        .expect("compact");
    let address = Address::new("contract", &suffix, "thread-42").expect("valid address");
    let next = Prompt::new(address, "event-8", "be useful", "continue").expect("valid");
    let next = runtime.accept(&next).await.expect("next accepted");
    let capture = Capture(Mutex::new(Vec::new()));
    runtime
        .run_operation(next.operation.id, &capture, &Registry::default())
        .await
        .expect("next run")
        .expect("next work");
    {
        let seen = capture.0.lock().expect("capture poisoned");
        assert_eq!(seen[0], Message::Summary("Earlier greeting".to_owned()));
        assert_eq!(seen[1], Message::User("continue".to_owned()));
    }
    store.close().await;
}

/// Failover stamps each answer with its model; recovery must replay the stamp,
/// or the next turn cannot tell whose reasoning it is replaying.
#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_replays_the_answering_model_after_reconnect() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let address = Address::new("contract", &unique_suffix(), "thread-42").expect("address");
    let first = Prompt::new(address.clone(), "event-7", "be useful", "héllo ☕").unwrap();
    let accepted = Runtime::new(store.clone()).accept(&first).await.unwrap();
    let stamped = ModelResponse {
        model: Some("bedrock/us-east-1/claude-secondary".to_owned()),
        reasoning: vec![Reasoning::Thinking {
            text: String::new(),
            signature: Some("signed".to_owned()),
        }],
        ..response("from the secondary")
    };
    let script = Answer(Mutex::new(Some(stamped.clone())));
    Runtime::new(store.clone())
        .run_operation(accepted.operation.id, &script, &Registry::default())
        .await
        .expect("first run")
        .expect("first work");
    store.close().await;

    let store = PostgresStore::connect(&url).await.expect("reconnect");
    let runtime = Runtime::new(store.clone());
    let next = Prompt::new(address, "event-8", "be useful", "continue").unwrap();
    let next = runtime.accept(&next).await.expect("next accepted");
    let capture = Capture(Mutex::new(Vec::new()));
    runtime
        .run_operation(next.operation.id, &capture, &Registry::default())
        .await
        .expect("next run")
        .expect("next work");
    let seen = capture.0.lock().expect("capture poisoned").clone();
    assert_eq!(seen[1], Message::Assistant(stamped));
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_named_execution_preserves_lane_order() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("connect");
    let runtime = Runtime::new(store.clone());
    let suffix = unique_suffix();
    let address = Address::new("test", &suffix, "main").expect("address");
    let first = Prompt::new(address.clone(), "first", "system", "one").unwrap();
    let second = Prompt::new(address, "second", "system", "two").unwrap();
    let first = runtime.accept(&first).await.unwrap().operation.id;
    let second = runtime.accept(&second).await.unwrap().operation.id;

    assert!(
        runtime
            .run_operation(second, &answer("two"), &Registry::default())
            .await
            .unwrap()
            .is_none()
    );
    runtime
        .run_operation(first, &answer("one"), &Registry::default())
        .await
        .unwrap();
    assert!(
        runtime
            .run_operation(second, &answer("two"), &Registry::default())
            .await
            .unwrap()
            .is_some()
    );
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_retry_after_reopen_matches_original_pre_enrichment_text() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let first = PostgresStore::connect(&url).await.expect("first pool");
    let suffix = unique_suffix();
    let address = Address::new("test", &suffix, "main").expect("address");
    let raw = Prompt::new(address.clone(), "ci-run", "system", "héllo ☕").expect("prompt");
    let enriched = raw
        .clone()
        .with_text("<memories>first</memories>\n\nhéllo ☕")
        .unwrap();
    let accepted = first.accept(&enriched).await.expect("accepted");
    first.close().await;

    let reopened = PostgresStore::connect(&url).await.expect("reopen");
    let retry = raw
        .with_text("<memories>changed</memories>\n\nhéllo ☕")
        .unwrap();
    let duplicate = reopened.accept(&retry).await.expect("same source prompt");
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.operation.id, accepted.operation.id);

    let changed = Prompt::new(address, "ci-run", "system", "different")
        .unwrap()
        .with_text("<memories>changed</memories>\n\nhéllo ☕")
        .unwrap();
    assert_eq!(
        reopened.accept(&changed).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    reopened.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_reopens_lane_tools_and_interactive_answers() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let first = PostgresStore::connect(&url).await.expect("first pool");
    let runtime = Runtime::new(first.clone());
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "mcp").expect("address");
    let target = DeliveryTarget::new("test", json!({"conversation":"C1"})).expect("target");
    let prompt = Prompt::new(address, "mcp-event", "system", "search")
        .expect("prompt")
        .with_delivery(target);
    let registry = Registry::new(vec![Arc::new(NamedTool)]);
    let accepted = runtime
        .accept_with_tools(&prompt, &registry)
        .await
        .expect("accepted");
    let id = format!("operation/{}/input/confirm", accepted.operation.id.get());
    let request =
        InteractionRequest::new(accepted.operation.id, &id, json!({"message":"Proceed?"}))
            .expect("request");
    let opened = first.open_interaction(&request).await.expect("opened");
    first.close().await;

    let second = PostgresStore::connect(&url).await.expect("second pool");
    let restored = second
        .interaction(&id)
        .await
        .expect("read")
        .expect("stored");
    assert_eq!(restored.client_message_id, opened.client_message_id);
    second
        .answer_interaction(&id, &json!({"action":"accept"}))
        .await
        .expect("answered");
    let model = Definitions(Mutex::new(Vec::new()));
    Runtime::new(second.clone())
        .run_operation(accepted.operation.id, &model, &registry)
        .await
        .expect("run")
        .expect("operation");
    assert_eq!(model.0.lock().unwrap()[0]["name"], "mcp__test__search");
    let answered = second
        .interaction(&id)
        .await
        .expect("read")
        .expect("stored");
    assert_eq!(answered.response, Some(json!({"action":"accept"})));
    second.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_fences_a_session_across_independent_pools() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store_a = PostgresStore::connect(&url).await.expect("first pool");
    let store_b = PostgresStore::connect(&url).await.expect("second pool");
    let ttl = Duration::from_secs(1);
    let first = Runtime::with_config(
        store_a.clone(),
        Config::new("postgres-worker-a", ttl).expect("config"),
    );
    let second = Runtime::with_config(
        store_b.clone(),
        Config::new("postgres-worker-b", ttl).expect("config"),
    );
    let suffix = unique_suffix();
    accept_lane(&first, &suffix, "lane-a", "event-a").await;
    accept_lane(&first, &suffix, "lane-b", "event-b").await;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let model = Blocking {
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let running = tokio::spawn(async move { first.run_next(&model, &Registry::default()).await });
    entered.notified().await;

    let stolen = second
        .run_next(&answer("must not run"), &Registry::default())
        .await
        .expect("claim check");

    assert!(stolen.is_none());
    release.notify_one();
    running.await.expect("joined").expect("first completed");
    second
        .run_next(&answer("second done"), &Registry::default())
        .await
        .expect("second lane")
        .expect("pending lane");
    store_a.close().await;
    store_b.close().await;
}

async fn accept_lane(runtime: &Runtime<PostgresStore>, conversation: &str, lane: &str, id: &str) {
    let address = Address::new("contract", conversation, lane).expect("address");
    let prompt = Prompt::new(address, id, "system", "prompt").expect("prompt");
    runtime.accept(&prompt).await.expect("accepted");
}

/// Settle a claimed operation for good, taking back any steering the settle
/// hands over, so a reused database never offers it to a later `run_next`.
async fn finish(store: &PostgresStore, lease: &brainstone_runtime::Lease) {
    let done = agent::Outcome::Completed {
        text: String::new(),
    };
    while store.settle(lease, &done).await.unwrap() != brainstone_runtime::Settled::Finished {}
}

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    format!("runtime-contract-{}-{nanos}", std::process::id())
}

#[derive(Clone)]
struct Counted(Arc<std::sync::atomic::AtomicUsize>);

impl Model for Counted {
    async fn generate(&self, _thread: &Thread, _tools: &[Value]) -> ModelResponse {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        response("scheduled done")
    }
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_releases_one_schedule_once_under_replica_contention() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let first = PostgresStore::connect(&url).await.expect("first pool");
    let second = PostgresStore::connect(&url).await.expect("second pool");
    let suffix = unique_suffix();
    let runtime = Runtime::new(first.clone());
    let address = Address::new("contract", &suffix, "schedule").unwrap();
    let prompt = Prompt::new(address, "origin", "system", "start").unwrap();
    let origin = runtime.accept(&prompt).await.unwrap().operation.id;
    runtime
        .run_next(&answer("origin done"), &Registry::default())
        .await
        .unwrap();
    let request = ScheduleRequest::new(
        origin,
        "stable-key",
        "scheduled prompt",
        ScheduleTime::AfterSeconds(0),
    )
    .unwrap();
    let (created, retry) = tokio::join!(first.schedule(&request), second.schedule(&request));
    let created = created.unwrap();
    let retry = retry.unwrap();
    assert_eq!(created, retry);

    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let runtime_a = Runtime::new(first.clone());
    let runtime_b = Runtime::new(second.clone());
    let model_a = Counted(Arc::clone(&count));
    let model_b = Counted(Arc::clone(&count));
    let tools_a = Registry::default();
    let tools_b = Registry::default();
    let (a, b) = tokio::join!(
        runtime_a.run_next(&model_a, &tools_a),
        runtime_b.run_next(&model_b, &tools_b)
    );
    assert_eq!(
        usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
        1
    );
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    first.close().await;
    second.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_steers_after_a_deployment_configuration_change_across_pools() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let first = PostgresStore::connect(&url).await.expect("first pool");
    let second = PostgresStore::connect(&url).await.expect("second pool");
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "steering").unwrap();
    let old = ExecutionProfile::new(None, None, 25, 0).unwrap();
    let original = Prompt::new(address.clone(), "original", "system", "start")
        .unwrap()
        .with_profile(old);
    let runtime = Runtime::new(first.clone());
    let operation = runtime.accept(&original).await.unwrap().operation.id;
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let model = SteeringBlock {
        calls: Arc::clone(&calls),
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    };
    let running = tokio::spawn({
        let runtime = runtime.clone();
        async move {
            runtime
                .run_operation(operation, &model, &Registry::default())
                .await
        }
    });
    entered.notified().await;

    let new = ExecutionProfile::new(None, None, 100, 0).unwrap();
    let correction = Prompt::new(address, "correction", "replacement system", "change course")
        .unwrap()
        .with_profile(new);
    let admitted = Runtime::new(second.clone())
        .admit_with_tools(&correction, &Registry::default())
        .await
        .unwrap();
    assert!(
        matches!(admitted, Admitted::Steering { operation: value, .. } if value.id == operation)
    );
    release.notify_one();
    running.await.unwrap().unwrap();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);

    first.close().await;
    second.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_subagent_limit_preserves_idempotent_retries() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("pool");
    let runtime = Runtime::new(store.clone());
    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "subagents").expect("address");
    let prompt = Prompt::new(address, "parent", "system", "delegate").expect("prompt");
    let parent = runtime.accept(&prompt).await.expect("parent").operation.id;
    let config =
        SubagentConfig::new("worker", "work", ExecutionProfile::default()).expect("config");
    let mut children = Vec::new();
    for index in 0..20 {
        let key = format!("spawn-{index}");
        let request = SubagentRequest::new(parent, &key, "task", config.clone()).expect("request");
        let spawned = store.spawn_subagent(&request).await.expect("spawn");
        children.push((request, spawned.agent.id));
    }
    let (request, id) = children.first().expect("first spawn");
    let duplicate = store.spawn_subagent(request).await.expect("retry");
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.agent.id, *id);
    let overflow = SubagentRequest::new(parent, "spawn-20", "task", config).expect("request");
    assert_eq!(
        store.spawn_subagent(&overflow).await.unwrap_err().code(),
        "agent_concurrency_exceeded"
    );
    for (_, id) in children {
        store.stop_subagent(parent, id).await.expect("stop child");
    }
    runtime
        .run_operation(parent, &answer("parent done"), &Registry::default())
        .await
        .expect("parent run")
        .expect("parent operation");
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_persists_and_scopes_memories() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("pool");
    store.ping().await.expect("store is ready");
    let scope = format!("test/{}", unique_suffix());

    let entry =
        Memory::new(&scope, &format!("{scope}/k1"), "prefers Métric units").expect("memory");
    store.save_memory(&entry).await.expect("saved");
    store.save_memory(&entry).await.expect("replay is a no-op");
    let second =
        Memory::new(&scope, &format!("{scope}/k2"), "works at 100% capacity").expect("memory");
    store.save_memory(&second).await.expect("saved");
    let conflict = Memory::new(&scope, &format!("{scope}/k1"), "rewritten").expect("memory");
    assert_eq!(
        store.save_memory(&conflict).await.unwrap_err().code(),
        "idempotency_conflict"
    );

    let recalled = store
        .recall_memories(&scope, None, 10)
        .await
        .expect("recalled");
    assert_eq!(
        recalled,
        vec![
            "works at 100% capacity".to_owned(),
            "prefers Métric units".to_owned()
        ]
    );
    let filtered = store
        .recall_memories(&scope, Some("100%"), 10)
        .await
        .expect("filtered");
    assert_eq!(filtered, vec!["works at 100% capacity".to_owned()]);
    let folded = store
        .recall_memories(&scope, Some("métric"), 10)
        .await
        .expect("folded");
    assert_eq!(folded, vec!["prefers Métric units".to_owned()]);

    let suffix = unique_suffix();
    let address = Address::new("contract", &suffix, "thread-memory").expect("address");
    let prompt = Prompt::new(address, "event-memory", "system", "prompt")
        .expect("prompt")
        .with_actor("U-contract");
    let accepted = store.accept(&prompt).await.expect("accepted");
    let scope = store
        .memory_scope(accepted.operation.id)
        .await
        .expect("scope");
    assert_eq!(scope.surface, "contract");
    assert_eq!(scope.conversation, suffix);
    assert_eq!(scope.actor.as_deref(), Some("U-contract"));
    // Drain the accepted operation: a pending leftover would be claimed by the
    // next test's run_next instead of its own work.
    Runtime::new(store.clone())
        .run_operation(
            accepted.operation.id,
            &answer("noted"),
            &Registry::default(),
        )
        .await
        .expect("run")
        .expect("operation");
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_counts_actor_operations_in_the_trailing_day_only() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("pool");
    let suffix = unique_suffix();
    let actor = format!("actor-{suffix}");
    let runtime = Runtime::new(store.clone());
    for lane in ["thread-q1", "thread-q2"] {
        let address = Address::new("contract", &suffix, lane).expect("address");
        let prompt = Prompt::new(address, &format!("event-{lane}"), "system", "prompt")
            .expect("prompt")
            .with_actor(&actor);
        let accepted = store.accept(&prompt).await.expect("accepted");
        // Drain each operation so later tests cannot claim the leftovers.
        runtime
            .run_operation(
                accepted.operation.id,
                &answer("noted"),
                &Registry::default(),
            )
            .await
            .expect("run")
            .expect("operation");
    }
    assert_eq!(
        store
            .actor_operations_last_day(&actor)
            .await
            .expect("count"),
        2
    );

    let pool = sqlx::PgPool::connect(&url).await.expect("raw pool");
    sqlx::query(
        "UPDATE operations SET created_at = now() - INTERVAL '2 days' \
         WHERE actor = $1 AND request_id = 'event-thread-q1'",
    )
    .bind(&actor)
    .execute(&pool)
    .await
    .expect("backdated");

    assert_eq!(
        store
            .actor_operations_last_day(&actor)
            .await
            .expect("count"),
        1
    );
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_rejects_a_claim_ttl_that_cannot_be_represented() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let adapter = format!("ttl-{suffix}");
    let address = Address::new("contract", &suffix, "ttl").unwrap();
    let prompt = Prompt::new(address, "first", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new(&adapter, json!({})).unwrap());
    store.accept(&prompt).await.unwrap();

    let overflow = DeliveryClaim::new(&adapter, "delivery", Duration::MAX, 32).unwrap();
    assert_eq!(
        store
            .claim_delivery(&overflow)
            .await
            .expect_err("an unrepresentable claim TTL is invalid input")
            .code(),
        "invalid_input"
    );
    let claim = DeliveryClaim::new(&adapter, "delivery", Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(Duration::from_hours(1))
        .unwrap();
    assert!(
        store
            .claim_delivery(&claim)
            .await
            .expect("a representable TTL still claims")
            .is_some(),
        "the rejected claim must not leave the row locked or changed"
    );
    drain(&store, "done").await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_presence_checkpoint_keeps_a_saved_stream_identity() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let suffix = unique_suffix();
    let adapter = format!("stream-keep-{suffix}");
    let address = Address::new("contract", &suffix, "stream-keep").unwrap();
    let prompt = Prompt::new(address, "first", "system", "A")
        .unwrap()
        .with_delivery(DeliveryTarget::new(&adapter, json!({})).unwrap());
    store.accept(&prompt).await.unwrap();

    let claim = DeliveryClaim::new(&adapter, "delivery", Duration::from_secs(30), 32)
        .unwrap()
        .with_presence(Duration::from_millis(1))
        .unwrap();
    let batch = store.claim_delivery(&claim).await.unwrap().unwrap();
    store
        .save_delivery_stream(&batch.lease, Some("stream-p"))
        .await
        .expect("stream identity saved under a live lease");
    store
        .checkpoint_delivery(&DeliveryCheckpoint::presented(batch.lease))
        .await
        .expect("presence checkpoint");
    tokio::time::sleep(Duration::from_millis(5)).await;

    let reclaim = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(
        reclaim.external_id.as_deref(),
        Some("stream-p"),
        "a presence checkpoint without a learned identity keeps the saved one"
    );
    drain(&store, "done").await;
    store.close().await;
}

/// Run the one operation this test accepted to a terminal state, so later
/// tests that claim the global work queue find only their own work.
async fn drain(store: &PostgresStore, text: &str) {
    Runtime::new(store.clone())
        .run_next(&answer(text), &Registry::default())
        .await
        .expect("run")
        .expect("the test's own operation");
}

/// A tenant per test keeps owner-wide counts from reaching across tests and runs.
fn standing_identity(tenant: &str) -> agent::ExecutionIdentity {
    agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: tenant.into(),
        actor: "U1".into(),
        conversation: "D1".into(),
        private: true,
    }
}

fn eight_toronto() -> ScheduleTime {
    let time = jiff::civil::Time::constant(8, 0, 0, 0);
    ScheduleTime::Repeat(
        brainstone_runtime::Recurrence::new(time, "America/Toronto", None).unwrap(),
    )
}

fn completed() -> agent::Outcome {
    agent::Outcome::Completed {
        text: "done".into(),
    }
}

fn failed() -> agent::Outcome {
    agent::Outcome::Failed {
        code: "model_unavailable".into(),
        message: "down".into(),
    }
}

/// Move the next run into the past instead of waiting a day. Each backdated
/// run time is distinct, as real releases are, so request ids never collide.
async fn make_due(pool: &sqlx::PgPool, id: brainstone_runtime::ScheduleId) {
    static BACKDATED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    let run_at = BACKDATED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    sqlx::query("UPDATE scheduled_work SET run_at_unix_ms = $2 WHERE id = $1")
        .bind(id.get())
        .bind(run_at)
        .execute(pool)
        .await
        .unwrap();
}

async fn finish_with(
    store: &PostgresStore,
    lease: &brainstone_runtime::Lease,
    outcome: &agent::Outcome,
) {
    store.record(lease, &Event::RunStarted).await.unwrap();
    let finished = Event::RunFinished {
        outcome: outcome.clone(),
    };
    store.record(lease, &finished).await.unwrap();
}

/// Claim until `lane` yields work, completing whatever other tests left due in
/// the shared database on the way; `None` once nothing is claimable.
async fn claim_lane(
    store: &PostgresStore,
    lane: brainstone_runtime::LaneId,
) -> Option<brainstone_runtime::Claimed> {
    loop {
        let claimed = store
            .claim("worker", Duration::from_secs(30))
            .await
            .unwrap()?;
        if claimed.operation.lane_id == lane {
            return Some(claimed);
        }
        finish_with(store, &claimed.lease, &completed()).await;
    }
}

/// Claim the lane's next run and finish it with `outcome`.
async fn run_lane(
    store: &PostgresStore,
    lane: brainstone_runtime::LaneId,
    outcome: &agent::Outcome,
) {
    let claimed = claim_lane(store, lane).await.expect("a due run");
    finish_with(store, &claimed.lease, outcome).await;
}

/// Deliver the next batch on the origin's target and return its runtime notice.
async fn deliver_next(store: &PostgresStore, adapter: &str) -> Option<String> {
    let claim = DeliveryClaim::new(adapter, "surface", Duration::from_secs(30), 32).unwrap();
    let batch = store
        .claim_delivery(&claim)
        .await
        .unwrap()
        .expect("a batch");
    let through = batch.events.last().expect("a finished run").sequence;
    let notice = batch.notice.clone();
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(batch.lease, through, None))
        .await
        .unwrap();
    notice
}

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

struct Origin {
    operation: OperationId,
    lane: brainstone_runtime::LaneId,
    adapter: String,
    identity: agent::ExecutionIdentity,
}

/// One origin per test: accepted, run, and delivered, so the lane is idle.
async fn standing_origin(store: &PostgresStore, suffix: &str) -> Origin {
    let adapter = format!("test-{suffix}");
    let address = Address::new("contract", suffix, "D1").unwrap();
    let prompt = Prompt::new(address, "origin", "system", "set up my briefing")
        .unwrap()
        .with_identity(Some(standing_identity(suffix)))
        .with_delivery(DeliveryTarget::new(&adapter, json!({"conversation": "D1"})).unwrap());
    let accepted = store.accept(&prompt).await.unwrap().operation;
    let claimed = store
        .claim_operation(accepted.id, "origin", Duration::from_secs(30))
        .await
        .unwrap()
        .expect("origin claimed");
    finish_with(store, &claimed.lease, &completed()).await;
    deliver_next(store, &adapter).await;
    Origin {
        operation: accepted.id,
        lane: accepted.lane_id,
        adapter,
        identity: standing_identity(suffix),
    }
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_standing_schedule_is_released_again_with_its_identity() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let origin = standing_origin(&store, &unique_suffix()).await;
    let request =
        ScheduleRequest::new(origin.operation, "daily", "brief me", eight_toronto()).unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    assert!(scheduled.repeat.is_some());
    assert!(scheduled.run_at_unix_ms > now_ms());
    assert_eq!(
        store.schedule(&request).await.unwrap(),
        scheduled,
        "idempotent retry"
    );

    make_due(&pool, scheduled.id).await;
    let first = claim_lane(&store, origin.lane).await.expect("promoted");
    assert_eq!(first.identity, Some(origin.identity.clone()));
    assert_eq!(first.thread.last(), &Message::User("brief me".into()));
    let listed = store.schedules(origin.operation).await.unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].state, brainstone_runtime::ScheduleState::Pending);
    assert!(
        listed[0].run_at_unix_ms > now_ms(),
        "advanced to the next occurrence"
    );

    make_due(&pool, scheduled.id).await;
    assert!(claim_lane(&store, origin.lane).await.is_none());
    assert!(
        store.schedules(origin.operation).await.unwrap()[0].run_at_unix_ms < now_ms(),
        "a run still in flight blocks the next release"
    );
    // Crash before the run finishes: the lease lapses and the same run resumes.
    let session = first.operation.session_id;
    drop(first);
    let reopened = PostgresStore::connect(&url).await.expect("reopened store");
    sqlx::query(
        "UPDATE sessions SET lease_expires_at = clock_timestamp() - INTERVAL '1 second' \
         WHERE id = $1",
    )
    .bind(session.get())
    .execute(&pool)
    .await
    .unwrap();
    let resumed = claim_lane(&reopened, origin.lane).await.expect("resumed");
    assert_eq!(resumed.identity, Some(origin.identity.clone()));
    finish_with(&reopened, &resumed.lease, &completed()).await;
    let second = claim_lane(&reopened, origin.lane)
        .await
        .expect("released again");
    assert_ne!(second.operation.id, resumed.operation.id);
    assert_eq!(second.identity, Some(origin.identity.clone()));
    let runs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM operations WHERE lane_id = $1 AND request_id LIKE 'schedule/%'",
    )
    .bind(origin.lane.get())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(runs, 2, "one operation per release, none for the crash");
    finish_with(&reopened, &second.lease, &completed()).await;
    store.close().await;
    reopened.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_failure_streaks_nudge_reset_and_cancel() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let origin = standing_origin(&store, &unique_suffix()).await;
    let request =
        ScheduleRequest::new(origin.operation, "daily", "brief me", eight_toronto()).unwrap();
    let id = store.schedule(&request).await.unwrap().id;
    let mut notices = Vec::new();
    for _ in 0..3 {
        make_due(&pool, id).await;
        run_lane(&store, origin.lane, &failed()).await;
        notices.push(deliver_next(&store, &origin.adapter).await);
    }
    assert_eq!(notices[0], None);
    assert_eq!(notices[1], None);
    assert!(
        notices[2]
            .as_deref()
            .is_some_and(|n| n.contains("3 runs in a row")),
        "{notices:?}"
    );
    make_due(&pool, id).await;
    run_lane(&store, origin.lane, &completed()).await;
    assert_eq!(
        deliver_next(&store, &origin.adapter).await,
        None,
        "a success resets the streak"
    );
    let mut last = None;
    for _ in 0..14 {
        make_due(&pool, id).await;
        run_lane(&store, origin.lane, &failed()).await;
        last = deliver_next(&store, &origin.adapter).await;
    }
    assert!(
        last.as_deref().is_some_and(|n| n.contains("cancelled")),
        "{last:?}"
    );
    assert_eq!(store.schedules(origin.operation).await.unwrap(), []);
    make_due(&pool, id).await;
    assert!(claim_lane(&store, origin.lane).await.is_none());
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_caps_standing_schedules_per_lane() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let origin = standing_origin(&store, &unique_suffix()).await;
    for index in 0..brainstone_runtime::MAX_REPEAT_PER_LANE {
        let request =
            ScheduleRequest::new(origin.operation, &format!("k{index}"), "x", eight_toronto())
                .unwrap();
        store.schedule(&request).await.unwrap();
    }
    let extra = ScheduleRequest::new(origin.operation, "extra", "x", eight_toronto()).unwrap();
    assert_eq!(
        store.schedule(&extra).await.unwrap_err().code(),
        "schedule_limit_exceeded"
    );
    let once = ScheduleRequest::new(
        origin.operation,
        "once",
        "x",
        ScheduleTime::AfterSeconds(60),
    )
    .unwrap();
    assert!(
        store.schedule(&once).await.is_ok(),
        "one-shots are not capped"
    );
    store.close().await;
}

/// Cancel every schedule a test's tenant created, so none comes due later and
/// is claimed by another test sharing the database.
async fn cancel_tenant_schedules(pool: &sqlx::PgPool, tenant: &str) {
    sqlx::query(
        "UPDATE scheduled_work s SET state = 'cancelled' FROM operations o \
         WHERE o.id = s.origin_operation_id AND o.execution_identity->>'tenant' = $1 \
         AND s.state = 'pending'",
    )
    .bind(tenant)
    .execute(pool)
    .await
    .unwrap();
}

fn every_half_hour() -> ScheduleTime {
    let spec = brainstone_runtime::RepeatSpec {
        every_minutes: Some(30),
        ..brainstone_runtime::RepeatSpec::default()
    };
    ScheduleTime::Repeat(brainstone_runtime::Recurrence::try_from(spec).unwrap())
}

const HALF_HOUR_MS: i64 = 30 * 60 * 1_000;

/// A row's run time measured from its anchor, the row's `created_at`.
async fn since_created(pool: &sqlx::PgPool, id: brainstone_runtime::ScheduleId) -> i64 {
    sqlx::query_scalar(
        "SELECT run_at_unix_ms - (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT \
         FROM scheduled_work WHERE id = $1",
    )
    .bind(id.get())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_interval_schedule_stores_its_rule_and_stays_on_its_grid() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let origin = standing_origin(&store, &tenant).await;
    let request = ScheduleRequest::new(
        origin.operation,
        "pulse",
        "check the queue",
        every_half_hour(),
    )
    .unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    assert_eq!(
        store.schedule(&request).await.unwrap(),
        scheduled,
        "idempotent retry"
    );
    assert_eq!(since_created(&pool, scheduled.id).await, HALF_HOUR_MS);
    let (kind, rule): (String, Value) =
        sqlx::query_as("SELECT schedule_kind, recurrence FROM scheduled_work WHERE id = $1")
            .bind(scheduled.id.get())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (kind.as_str(), rule),
        ("recur", json!({"every_minutes": 30}))
    );

    make_due(&pool, scheduled.id).await;
    let run = claim_lane(&store, origin.lane).await.expect("promoted");
    assert_eq!(run.thread.last(), &Message::User("check the queue".into()));
    let listed = store.schedules(origin.operation).await.unwrap();
    assert!(listed[0].run_at_unix_ms > now_ms(), "advanced past now");
    assert_eq!(since_created(&pool, scheduled.id).await % HALF_HOUR_MS, 0);
    assert_eq!(
        serde_json::to_value(&listed[0].repeat).unwrap(),
        json!({"every_minutes": 30})
    );
    finish_with(&store, &run.lease, &completed()).await;
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_a_repeat_row_written_before_rule_json_still_releases() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let origin = standing_origin(&store, &tenant).await;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO scheduled_work (origin_operation_id, lane_id, schedule_key, prompt, \
         run_at_unix_ms, schedule_kind, schedule_value, state, delivery_target, repeat_minute, \
         repeat_time_zone, repeat_days) SELECT id, lane_id, 'legacy', 'legacy brief', 1, \
         'repeat', 0, 'pending', delivery_target, 480, 'America/Toronto', \
         ARRAY[1, 2, 3, 4, 5]::SMALLINT[] FROM operations WHERE id = $1 RETURNING id",
    )
    .bind(origin.operation.get())
    .fetch_one(&pool)
    .await
    .unwrap();
    let listed = store.schedules(origin.operation).await.unwrap();
    assert_eq!(
        serde_json::to_value(&listed[0].repeat).unwrap(),
        json!({"time": "08:00:00", "time_zone": "America/Toronto", "days": [1, 2, 3, 4, 5]})
    );

    let run = claim_lane(&store, origin.lane).await.expect("promoted");
    assert_eq!(run.thread.last(), &Message::User("legacy brief".into()));
    let listed = store.schedules(origin.operation).await.unwrap();
    assert_eq!(listed[0].id.get(), id);
    assert_eq!(listed[0].state, brainstone_runtime::ScheduleState::Pending);
    assert!(
        listed[0].run_at_unix_ms > now_ms(),
        "advanced to the next weekday"
    );
    finish_with(&store, &run.lease, &completed()).await;
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_refuses_to_release_a_standing_row_as_a_one_shot() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let origin = standing_origin(&store, &tenant).await;
    let request =
        ScheduleRequest::new(origin.operation, "pulse", "check", every_half_hour()).unwrap();
    let id = store.schedule(&request).await.unwrap().id;
    // What a binary that predates 'recur' does with a row it cannot read.
    let error = sqlx::query("UPDATE scheduled_work SET state = 'released' WHERE id = $1")
        .bind(id.get())
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("scheduled_work_standing_never_released"),
        "{error}"
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

fn principal(
    tenant: &str,
    actor: &str,
    conversation: &str,
    private: bool,
) -> agent::ExecutionIdentity {
    agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: tenant.into(),
        actor: actor.into(),
        conversation: conversation.into(),
        private,
    }
}

/// A finished operation in `lane` of `identity`'s conversation. Finishing it
/// keeps it from being claimed by another test's `run_next`.
async fn accepted_as(
    store: &PostgresStore,
    identity: agent::ExecutionIdentity,
    lane: &str,
) -> OperationId {
    let address = Address::new("contract", &identity.conversation_key(), lane).unwrap();
    let request = format!("{lane}-{}", identity.actor);
    let prompt = Prompt::new(address, &request, "system", "hi")
        .unwrap()
        .with_identity(Some(identity));
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "origin", Duration::from_secs(30))
        .await
        .unwrap();
    if let Some(claimed) = claimed {
        finish_with(store, &claimed.lease, &completed()).await;
    }
    id
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_an_owner_sees_and_cancels_their_schedules_from_direct_messages() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let thread = accepted_as(&store, principal(&tenant, "U1", "C1", false), "t1").await;
    let request = ScheduleRequest::new(thread, "standup", "post it", eight_toronto()).unwrap();
    let id = store.schedule(&request).await.unwrap().id;

    let own_dm = accepted_as(&store, principal(&tenant, "U1", "D1", true), "d1").await;
    let listed = store.schedules(own_dm).await.unwrap();
    assert_eq!(listed.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
    assert_eq!(listed[0].owner.as_deref(), Some("U1"));
    let other_dm = accepted_as(&store, principal(&tenant, "U2", "D2", true), "d1").await;
    assert_eq!(store.schedules(other_dm).await.unwrap(), []);
    assert_eq!(
        store
            .cancel_schedule(other_dm, id)
            .await
            .unwrap_err()
            .code(),
        "schedule_not_found"
    );
    let other_thread = accepted_as(&store, principal(&tenant, "U1", "C1", false), "t2").await;
    assert_eq!(store.schedules(other_thread).await.unwrap(), []);

    let cancelled = store.cancel_schedule(own_dm, id).await.unwrap();
    assert_eq!(
        cancelled.state,
        brainstone_runtime::ScheduleState::Cancelled
    );
    assert_eq!(
        store.cancel_schedule(own_dm, id).await.unwrap(),
        cancelled,
        "idempotent"
    );
    assert_eq!(store.schedules(thread).await.unwrap(), []);
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_caps_standing_schedules_per_owner_across_lanes() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let per_lane = brainstone_runtime::MAX_REPEAT_PER_LANE;
    for index in 0..brainstone_runtime::MAX_REPEAT_PER_OWNER {
        let lane = format!("d{}", index / per_lane);
        let origin = accepted_as(&store, principal(&tenant, "U1", "D1", true), &lane).await;
        let request =
            ScheduleRequest::new(origin, &format!("k{index}"), "x", eight_toronto()).unwrap();
        store.schedule(&request).await.unwrap();
    }
    let fresh = accepted_as(&store, principal(&tenant, "U1", "D1", true), "fresh").await;
    let extra = ScheduleRequest::new(fresh, "extra", "x", eight_toronto()).unwrap();
    assert_eq!(
        store.schedule(&extra).await.unwrap_err().code(),
        "schedule_owner_limit_exceeded"
    );
    let other = accepted_as(&store, principal(&tenant, "U2", "D2", true), "d0").await;
    let theirs = ScheduleRequest::new(other, "theirs", "x", eight_toronto()).unwrap();
    assert!(
        store.schedule(&theirs).await.is_ok(),
        "the cap is per owner"
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_owner_cap_counts_after_taking_the_owner_lock() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let owner = || principal(&tenant, "U1", "D1", true);
    let per_lane = brainstone_runtime::MAX_REPEAT_PER_LANE;
    for index in 0..brainstone_runtime::MAX_REPEAT_PER_OWNER - 1 {
        let origin = accepted_as(&store, owner(), &format!("d{}", index / per_lane)).await;
        let request =
            ScheduleRequest::new(origin, &format!("k{index}"), "x", eight_toronto()).unwrap();
        store.schedule(&request).await.unwrap();
    }
    let rival = accepted_as(&store, owner(), "rival").await;
    let last = accepted_as(&store, owner(), "last").await;
    // Another replica holds the owner lock and is about to take the last slot.
    let mut replica = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *replica)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(json!(["test", tenant, "U1"]).to_string())
        .execute(&mut *replica)
        .await
        .unwrap();
    let waiting = store.clone();
    let request = ScheduleRequest::new(last, "last", "x", eight_toronto()).unwrap();
    let task = tokio::spawn(async move { waiting.schedule(&request).await });
    wait_for_database_lock(&pool, blocker).await;
    sqlx::query(
        "INSERT INTO scheduled_work (origin_operation_id, lane_id, schedule_key, prompt, \
         run_at_unix_ms, schedule_kind, schedule_value, state, recurrence) \
         SELECT id, lane_id, 'rival', 'x', 9999999999999, 'recur', 0, 'pending', \
         '{\"every_minutes\": 60}'::JSONB FROM operations WHERE id = $1",
    )
    .bind(rival.get())
    .execute(&mut *replica)
    .await
    .unwrap();
    replica.commit().await.unwrap();
    assert_eq!(
        task.await.unwrap().unwrap_err().code(),
        "schedule_owner_limit_exceeded",
        "the count must see the rival's committed row"
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

/// A finished DM operation of owner U1 in `tenant`, delivered on its own adapter.
async fn posting_origin(store: &PostgresStore, tenant: &str) -> OperationId {
    let identity = principal(tenant, "U1", "D1", true);
    let address = Address::new("contract", &identity.conversation_key(), "d1").unwrap();
    let adapter = format!("test-{tenant}");
    let prompt = Prompt::new(address, "post-origin", "system", "post the plan in #eng")
        .unwrap()
        .with_identity(Some(identity))
        .with_delivery(DeliveryTarget::new(&adapter, json!({"thread": "d1"})).unwrap());
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "origin", Duration::from_secs(30))
        .await
        .unwrap()
        .expect("origin claimed");
    finish_with(store, &claimed.lease, &completed()).await;
    deliver_next(store, &adapter).await;
    id
}

fn eng(tenant: &str, actor: &str) -> brainstone_runtime::Destination {
    let identity = principal(tenant, actor, "C9", false);
    let adapter = format!("test-{tenant}");
    brainstone_runtime::Destination {
        surface: "contract".into(),
        conversation: identity.conversation_key(),
        target: DeliveryTarget::new(&adapter, json!({"post": "C9"})).unwrap(),
        identity,
    }
}

fn post_now(
    origin: OperationId,
    key: &str,
    destination: brainstone_runtime::Destination,
) -> ScheduleRequest {
    ScheduleRequest::new(origin, key, "post the plan", ScheduleTime::AfterSeconds(0))
        .unwrap()
        .with_destination(destination)
}

/// Claim until an operation runs as `identity`, finishing others on the way.
async fn claim_as(
    store: &PostgresStore,
    identity: &agent::ExecutionIdentity,
) -> Option<brainstone_runtime::Claimed> {
    loop {
        let claimed = store
            .claim("worker", Duration::from_secs(30))
            .await
            .unwrap()?;
        if claimed.identity.as_ref() == Some(identity) {
            return Some(claimed);
        }
        finish_with(store, &claimed.lease, &completed()).await;
    }
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_each_post_runs_in_a_new_lane_that_its_posted_root_then_names() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let tenant = unique_suffix();
    let dm = posting_origin(&store, &tenant).await;
    let request = post_now(dm, "k", eng(&tenant, "U1"));
    let scheduled = store.schedule(&request).await.unwrap();
    assert_eq!(
        store.schedule(&request).await.unwrap(),
        scheduled,
        "idempotent retry"
    );
    let moved = post_now(dm, "k", eng(&tenant, "U1")).with_destination({
        let mut other = eng(&tenant, "U1");
        other.target = DeliveryTarget::new("elsewhere", json!({})).unwrap();
        other
    });
    assert_eq!(
        store.schedule(&moved).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    assert_eq!(scheduled.destination.as_deref(), Some("C9"));
    assert_eq!(scheduled.owner.as_deref(), Some("U1"));

    let run = claim_as(&store, &eng(&tenant, "U1").identity)
        .await
        .expect("released");
    let origin_lane = store.operation(dm).await.unwrap().unwrap().lane_id;
    assert_ne!(
        run.operation.lane_id, origin_lane,
        "a post opens its own lane"
    );
    assert_eq!(run.thread.last(), &Message::User("post the plan".into()));
    finish_with(&store, &run.lease, &completed()).await;
    let adapter = format!("test-{tenant}");
    let claim = DeliveryClaim::new(&adapter, "surface", Duration::from_secs(30), 32).unwrap();
    let batch = store
        .claim_delivery(&claim)
        .await
        .unwrap()
        .expect("the post");
    assert_eq!(batch.target, eng(&tenant, "U1").target);
    assert!(
        batch.accepted_unix_ms > now_ms() - 60_000,
        "{}",
        batch.accepted_unix_ms
    );

    let reply = DeliveryTarget::new(&adapter, json!({"thread": "1700.1"})).unwrap();
    assert!(
        store
            .bind_lane(&batch.lease, "1700.1", &reply)
            .await
            .unwrap()
    );
    assert!(
        store
            .bind_lane(&batch.lease, "1700.1", &reply)
            .await
            .unwrap(),
        "no-op"
    );
    assert_eq!(
        store.delivery_target(run.operation.id).await.unwrap(),
        Some(reply)
    );
    let posted = Address::new("contract", &eng(&tenant, "U1").conversation, "1700.1").unwrap();
    assert!(store.lane_is_followed(&posted).await.unwrap());
    let colleague = principal(&tenant, "U2", "C9", false);
    let answer = Prompt::new(posted, "1700.2", "system", "thanks")
        .unwrap()
        .with_identity(Some(colleague));
    let answer = store.accept(&answer).await.unwrap().operation;
    assert_eq!(
        answer.lane_id, run.operation.lane_id,
        "a reply joins the post's lane"
    );
    let through = batch.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(batch.lease, through, None))
        .await
        .unwrap();
    let answered = store
        .claim_operation(answer.id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .expect("reply claimed");
    assert!(
        answered
            .thread
            .messages()
            .contains(&Message::User("post the plan".into())),
        "the reply sees the post's history"
    );
    finish_with(&store, &answered.lease, &completed()).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_a_root_another_lane_already_holds_is_not_bound() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let tenant = unique_suffix();
    let dm = posting_origin(&store, &tenant).await;
    store
        .schedule(&post_now(dm, "k", eng(&tenant, "U1")))
        .await
        .unwrap();
    let run = claim_as(&store, &eng(&tenant, "U1").identity)
        .await
        .expect("released");
    finish_with(&store, &run.lease, &completed()).await;
    let colleague = principal(&tenant, "U2", "C9", false);
    accepted_as(&store, colleague, "1700.1").await;
    let adapter = format!("test-{tenant}");
    let claim = DeliveryClaim::new(&adapter, "surface", Duration::from_secs(30), 32).unwrap();
    let batch = store
        .claim_delivery(&claim)
        .await
        .unwrap()
        .expect("the post");
    let reply = DeliveryTarget::new(&adapter, json!({"thread": "1700.1"})).unwrap();
    assert!(
        !store
            .bind_lane(&batch.lease, "1700.1", &reply)
            .await
            .unwrap()
    );
    assert_eq!(
        store.delivery_target(run.operation.id).await.unwrap(),
        Some(eng(&tenant, "U1").target)
    );
    let through = batch.events.last().unwrap().sequence;
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(batch.lease, through, None))
        .await
        .unwrap();
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_only_a_persons_own_surface_message_can_schedule_a_post_as_them() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let dm = posting_origin(&store, &tenant).await;
    let mut as_dm = eng(&tenant, "U1");
    as_dm.identity.private = true;
    as_dm.conversation = as_dm.identity.conversation_key();
    for (case, destination) in [("someone else", eng(&tenant, "U2")), ("a DM", as_dm)] {
        assert_eq!(
            store
                .schedule(&post_now(dm, case, destination))
                .await
                .unwrap_err()
                .code(),
            "schedule_destination_forbidden",
            "{case}"
        );
    }
    let no_target = accepted_as(&store, principal(&tenant, "U1", "D1", true), "child-like").await;
    assert_eq!(
        store
            .schedule(&post_now(no_target, "k", eng(&tenant, "U1")))
            .await
            .unwrap_err()
            .code(),
        "schedule_destination_forbidden",
        "no delivery target"
    );
    let in_place =
        ScheduleRequest::new(dm, "in-place", "again", ScheduleTime::AfterSeconds(0)).unwrap();
    store.schedule(&in_place).await.unwrap();
    let owner = principal(&tenant, "U1", "D1", true);
    let scheduled_run = claim_as(&store, &owner).await.expect("released run");
    assert_eq!(
        store
            .schedule(&post_now(
                scheduled_run.operation.id,
                "k",
                eng(&tenant, "U1")
            ))
            .await
            .unwrap_err()
            .code(),
        "schedule_destination_forbidden",
        "a scheduled run's instructions came from the model"
    );
    finish_with(&store, &scheduled_run.lease, &completed()).await;
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_an_owner_creates_a_bounded_number_of_posts_per_hour() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let dm = posting_origin(&store, &tenant).await;
    for index in 0..brainstone_runtime::MAX_POSTS_PER_OWNER_PER_HOUR {
        let request = post_now(dm, &format!("k{index}"), eng(&tenant, "U1"));
        store.schedule(&request).await.unwrap();
    }
    assert_eq!(
        store
            .schedule(&post_now(dm, "extra", eng(&tenant, "U1")))
            .await
            .unwrap_err()
            .code(),
        "schedule_post_rate_exceeded"
    );
    let in_place = ScheduleRequest::new(dm, "here", "x", ScheduleTime::AfterSeconds(3600)).unwrap();
    assert!(
        store.schedule(&in_place).await.is_ok(),
        "replies in place are not posts"
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_anyone_in_the_destination_channel_can_see_and_cancel_its_posts() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let dm = posting_origin(&store, &tenant).await;
    let later = ScheduleRequest::new(dm, "k", "post it", ScheduleTime::AfterSeconds(3600))
        .unwrap()
        .with_destination(eng(&tenant, "U1"));
    let id = store.schedule(&later).await.unwrap().id;
    let colleague = accepted_as(&store, principal(&tenant, "U2", "C9", false), "t7").await;
    let listed = store.schedules(colleague).await.unwrap();
    assert_eq!(listed.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
    let elsewhere = accepted_as(&store, principal(&tenant, "U3", "C3", false), "t1").await;
    assert_eq!(store.schedules(elsewhere).await.unwrap(), []);
    assert_eq!(
        store
            .cancel_schedule(elsewhere, id)
            .await
            .unwrap_err()
            .code(),
        "schedule_not_found"
    );
    let cancelled = store.cancel_schedule(colleague, id).await.unwrap();
    assert_eq!(
        cancelled.state,
        brainstone_runtime::ScheduleState::Cancelled
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

/// U1's delivered message in channel thread `t1` of `tenant`.
async fn thread_origin(store: &PostgresStore, tenant: &str) -> (OperationId, Address) {
    let identity = principal(tenant, "U1", "C1", false);
    let thread = Address::new("contract", &identity.conversation_key(), "t1").unwrap();
    let adapter = format!("test-{tenant}");
    let prompt = Prompt::new(thread.clone(), "1", "system", "set it up")
        .unwrap()
        .with_identity(Some(identity))
        .with_delivery(DeliveryTarget::new(&adapter, json!({"thread": "t1"})).unwrap());
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(id, "origin", Duration::from_secs(30))
        .await
        .unwrap()
        .expect("origin claimed");
    finish_with(store, &claimed.lease, &completed()).await;
    deliver_next(store, &adapter).await;
    (id, thread)
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_a_stopped_thread_pauses_what_replies_in_it_until_a_mention() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let pool = sqlx::PgPool::connect(&url).await.expect("test pool");
    let tenant = unique_suffix();
    let (origin, thread) = thread_origin(&store, &tenant).await;
    let here = ScheduleRequest::new(origin, "here", "standup", eight_toronto()).unwrap();
    let here = store.schedule(&here).await.unwrap().id;
    let mut channel = eng(&tenant, "U1");
    channel.target = DeliveryTarget::new(&format!("test-{tenant}"), json!({"post": "C9"})).unwrap();
    let post = ScheduleRequest::new(origin, "post", "post it", eight_toronto())
        .unwrap()
        .with_destination(channel.clone());
    let post = store.schedule(&post).await.unwrap().id;
    store
        .unfollow_lane(origin, &format!("stop-{tenant}"))
        .await
        .unwrap();

    make_due(&pool, here).await;
    make_due(&pool, post).await;
    let run = claim_as(&store, &channel.identity)
        .await
        .expect("the post still runs");
    finish_with(&store, &run.lease, &completed()).await;
    let owner = principal(&tenant, "U1", "C1", false);
    assert!(
        claim_as(&store, &owner).await.is_none(),
        "the thread's schedule waits"
    );
    let listed = store.schedules(origin).await.unwrap();
    let paused: Vec<_> = listed.iter().map(|s| (s.id, s.paused)).collect();
    assert!(
        paused.contains(&(here, true)) && paused.contains(&(post, false)),
        "{paused:?}"
    );

    store.follow_lane(&thread, "mention-2").await.unwrap();
    let resumed = claim_as(&store, &owner).await.expect("one catch-up run");
    assert_eq!(resumed.thread.last(), &Message::User("standup".into()));
    finish_with(&store, &resumed.lease, &completed()).await;
    assert!(
        store
            .schedules(origin)
            .await
            .unwrap()
            .iter()
            .all(|s| !s.paused)
    );
    cancel_tenant_schedules(&pool, &tenant).await;
    store.close().await;
}

#[tokio::test]
#[ignore = "requires a dedicated BRAINSTONE_TEST_DATABASE_URL"]
async fn postgres_the_owners_stop_reaches_a_scheduled_run_and_nobody_elses_does() {
    let url = std::env::var("BRAINSTONE_TEST_DATABASE_URL").expect("test database URL");
    let store = PostgresStore::connect(&url).await.expect("store");
    let tenant = unique_suffix();
    let (origin, thread) = thread_origin(&store, &tenant).await;
    let request =
        ScheduleRequest::new(origin, "k", "summarize", ScheduleTime::AfterSeconds(0)).unwrap();
    store.schedule(&request).await.unwrap();
    let owner = principal(&tenant, "U1", "C1", false);
    let run = claim_as(&store, &owner).await.expect("the scheduled run");
    let said = |actor: &str, request: &str| {
        Prompt::new(thread.clone(), request, "system", "stop")
            .unwrap()
            .with_identity(Some(principal(&tenant, actor, "C1", false)))
    };
    let stop = store.admit_with_tools(&said("U1", "2"), &[]).await.unwrap();
    assert!(matches!(stop, Admitted::Steering { .. }), "{stop:?}");
    assert_eq!(
        store.steering(&run.lease).await.unwrap(),
        vec![Message::User("stop".into())]
    );
    let other = store.admit_with_tools(&said("U2", "3"), &[]).await.unwrap();
    assert!(matches!(other, Admitted::Operation(_)), "{other:?}");
    finish_with(&store, &run.lease, &completed()).await;
    let queued = claim_as(&store, &principal(&tenant, "U2", "C1", false))
        .await
        .expect("the other message runs as its own operation");
    finish_with(&store, &queued.lease, &completed()).await;
    store.close().await;
}
