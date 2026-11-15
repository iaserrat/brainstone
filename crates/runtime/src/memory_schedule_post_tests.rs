//! Schedules that post each run as a new conversation root elsewhere, through
//! the public store seams.
#![allow(clippy::too_many_lines)]

use std::time::Duration;

use agent::{Event, ExecutionIdentity, Outcome};
use serde_json::json;

use crate::{
    Address, DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget, Destination, Error,
    LaneStore, MAX_POSTS_PER_OWNER_PER_HOUR, MemoryStore, OperationId, Prompt, ScheduleRequest,
    ScheduleStore, ScheduleTime, Store,
};

const LEASE: Duration = Duration::from_secs(30);

fn identity(actor: &str, conversation: &str, private: bool) -> ExecutionIdentity {
    ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: actor.into(),
        conversation: conversation.into(),
        private,
    }
}

fn target(data: serde_json::Value) -> DeliveryTarget {
    DeliveryTarget::new("test", data).unwrap()
}

/// An accepted, finished operation in `lane` of `identity`'s conversation.
async fn origin(store: &MemoryStore, identity: ExecutionIdentity, lane: &str) -> OperationId {
    let address = Address::new("test", &identity.conversation_key(), lane).unwrap();
    let prompt = Prompt::new(address, &format!("{lane}-{}", identity.actor), "system", "hi")
        .unwrap()
        .with_identity(Some(identity))
        .with_delivery(target(json!({"thread": lane})));
    let id = store.accept(&prompt).await.unwrap().operation.id;
    if let Some(claim) = store.claim_operation(id, "origin", LEASE).await.unwrap() {
        finish(store, &claim.lease).await;
        drain(store).await;
    }
    id
}

async fn finish(store: &MemoryStore, lease: &crate::Lease) {
    store.record(lease, &Event::RunStarted).await.unwrap();
    let finished = Event::RunFinished {
        outcome: Outcome::Completed {
            text: "posted".into(),
        },
    };
    store.record(lease, &finished).await.unwrap();
}

async fn drain(store: &MemoryStore) {
    let claim = DeliveryClaim::new("test", "surface", LEASE, 32).unwrap();
    while let Some(batch) = store.claim_delivery(&claim).await.unwrap() {
        let through = batch.events.last().map_or(0, |event| event.sequence);
        let checkpoint = DeliveryCheckpoint::new(batch.lease, through, None);
        store.checkpoint_delivery(&checkpoint).await.unwrap();
    }
}

fn eng(actor: &str) -> Destination {
    let identity = identity(actor, "C9", false);
    Destination {
        surface: "test".into(),
        conversation: identity.conversation_key(),
        target: target(json!({"post": "C9"})),
        identity,
    }
}

fn post_now(origin: OperationId, key: &str, destination: Destination) -> ScheduleRequest {
    ScheduleRequest::new(origin, key, "post the plan", ScheduleTime::AfterSeconds(0))
        .unwrap()
        .with_destination(destination)
}

#[tokio::test]
async fn each_post_runs_in_a_new_lane_that_its_posted_root_then_names() {
    let store = MemoryStore::default();
    let dm = origin(&store, identity("U1", "D1", true), "d1").await;
    let scheduled = store.schedule(&post_now(dm, "k", eng("U1"))).await.unwrap();
    assert_eq!(scheduled.destination.as_deref(), Some("C9"));
    assert_eq!(scheduled.owner.as_deref(), Some("U1"));

    let run = store.claim("worker", LEASE).await.unwrap().expect("released");
    let origin_lane = store.operation(dm).await.unwrap().unwrap().lane_id;
    assert_ne!(run.operation.lane_id, origin_lane, "a post opens its own lane");
    assert_eq!(run.identity, Some(eng("U1").identity), "runs as the owner in #eng");
    assert_eq!(run.thread.last(), &agent::Message::User("post the plan".into()));
    assert_eq!(
        store.delivery_target(run.operation.id).await.unwrap(),
        Some(target(json!({"post": "C9"})))
    );
    finish(&store, &run.lease).await;
    let claim = DeliveryClaim::new("test", "surface", LEASE, 32).unwrap();
    let batch = store.claim_delivery(&claim).await.unwrap().expect("the post");
    assert!(batch.accepted_unix_ms > 0);

    let reply_target = target(json!({"thread": "1700.1"}));
    assert!(store.bind_lane(&batch.lease, "1700.1", &reply_target).await.unwrap());
    assert!(
        store.bind_lane(&batch.lease, "1700.1", &reply_target).await.unwrap(),
        "binding again is a no-op"
    );
    assert_eq!(
        store.delivery_target(run.operation.id).await.unwrap(),
        Some(reply_target),
        "a retry now finds the thread it posted"
    );
    let posted = Address::new("test", &eng("U1").conversation, "1700.1").unwrap();
    assert!(store.lane_is_followed(&posted).await.unwrap());
    let reply = Prompt::new(posted, "1700.2", "system", "thanks")
        .unwrap()
        .with_identity(Some(identity("U2", "C9", false)));
    let reply = store.accept(&reply).await.unwrap().operation;
    assert_eq!(reply.lane_id, run.operation.lane_id, "a reply joins the post's lane");
}

#[tokio::test]
async fn a_root_another_lane_already_holds_is_not_bound() {
    let store = MemoryStore::default();
    let dm = origin(&store, identity("U1", "D1", true), "d1").await;
    store.schedule(&post_now(dm, "k", eng("U1"))).await.unwrap();
    let run = store.claim("worker", LEASE).await.unwrap().expect("released");
    finish(&store, &run.lease).await;
    // A mention in the new thread arrived before the bind.
    let colleague = identity("U2", "C9", false);
    let thread = Address::new("test", &colleague.conversation_key(), "1700.1").unwrap();
    let mention = Prompt::new(thread, "1700.2", "system", "hi")
        .unwrap()
        .with_identity(Some(colleague));
    store.accept(&mention).await.unwrap();
    let claim = DeliveryClaim::new("test", "surface", LEASE, 32).unwrap();
    let batch = store.claim_delivery(&claim).await.unwrap().expect("the post");
    let reply_target = target(json!({"thread": "1700.1"}));
    assert!(!store.bind_lane(&batch.lease, "1700.1", &reply_target).await.unwrap());
    assert_eq!(
        store.delivery_target(run.operation.id).await.unwrap(),
        Some(target(json!({"post": "C9"})))
    );
}

#[tokio::test]
async fn only_a_persons_own_surface_message_can_schedule_a_post_as_them() {
    let store = MemoryStore::default();
    let dm = origin(&store, identity("U1", "D1", true), "d1").await;
    let mut as_dm = eng("U1");
    as_dm.identity.private = true;
    as_dm.conversation = as_dm.identity.conversation_key();
    let mut wrong_session = eng("U1");
    wrong_session.conversation = "C9".into();
    let refused = [
        ("someone else", eng("U2")),
        ("into a private conversation", as_dm),
        ("outside the channel's session", wrong_session),
    ];
    for (case, destination) in refused {
        assert_eq!(
            store.schedule(&post_now(dm, case, destination)).await,
            Err(Error::ScheduleDestinationForbidden),
            "{case}"
        );
    }
    let anonymous = Address::new("test", "C1", "t1").unwrap();
    let anonymous = Prompt::new(anonymous, "a", "system", "hi")
        .unwrap()
        .with_delivery(target(json!({"thread": "t1"})));
    let anonymous = store.accept(&anonymous).await.unwrap().operation.id;
    assert_eq!(
        store.schedule(&post_now(anonymous, "k", eng("U1"))).await,
        Err(Error::ScheduleDestinationForbidden),
        "unauthenticated"
    );
    let reply_in_place = ScheduleRequest::new(dm, "later", "again", ScheduleTime::AfterSeconds(0));
    store.schedule(&reply_in_place.unwrap()).await.unwrap();
    let scheduled_run = store.claim("worker", LEASE).await.unwrap().expect("released");
    assert_eq!(
        store
            .schedule(&post_now(scheduled_run.operation.id, "k", eng("U1")))
            .await,
        Err(Error::ScheduleDestinationForbidden),
        "a scheduled run's instructions came from the model"
    );
}

#[tokio::test]
async fn an_owner_creates_a_bounded_number_of_posts_per_hour() {
    let store = MemoryStore::default();
    let dm = origin(&store, identity("U1", "D1", true), "d1").await;
    for index in 0..MAX_POSTS_PER_OWNER_PER_HOUR {
        let request = post_now(dm, &format!("k{index}"), eng("U1"));
        store.schedule(&request).await.unwrap();
    }
    assert_eq!(
        store.schedule(&post_now(dm, "extra", eng("U1"))).await,
        Err(Error::SchedulePostRateExceeded)
    );
    let reply_in_place = ScheduleRequest::new(dm, "here", "x", ScheduleTime::AfterSeconds(60));
    assert!(store.schedule(&reply_in_place.unwrap()).await.is_ok());
}

#[tokio::test]
async fn anyone_in_the_destination_channel_can_see_and_cancel_its_posts() {
    let store = MemoryStore::default();
    let dm = origin(&store, identity("U1", "D1", true), "d1").await;
    let id = store.schedule(&post_now(dm, "k", eng("U1"))).await.unwrap().id;
    let colleague = origin(&store, identity("U2", "C9", false), "t7").await;
    let listed = store.schedules(colleague).await.unwrap();
    assert_eq!(listed.iter().map(|s| s.id).collect::<Vec<_>>(), vec![id]);
    let elsewhere = origin(&store, identity("U3", "C3", false), "t1").await;
    assert_eq!(store.schedules(elsewhere).await.unwrap(), []);
    assert_eq!(
        store.cancel_schedule(elsewhere, id).await,
        Err(Error::ScheduleNotFound)
    );
    let cancelled = store.cancel_schedule(colleague, id).await.unwrap();
    assert_eq!(cancelled.state, crate::ScheduleState::Cancelled);
    assert!(store.claim("worker", LEASE).await.unwrap().is_none());
}
