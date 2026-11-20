//! Stopping: a thread that stops being followed pauses the schedules that
//! reply in it, and a person's "stop" reaches a scheduled run in progress.

#![allow(clippy::too_many_lines)]

use std::time::Duration;

use agent::{Event, ExecutionIdentity, Outcome};
use jiff::civil::Time;
use serde_json::json;

use crate::{
    Address, Admitted, DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget,
    Destination, LaneStore, MemoryStore, OperationId, Prompt, Recurrence, ScheduleId,
    ScheduleRequest, ScheduleStore, ScheduleTime, Store,
};

const LEASE: Duration = Duration::from_secs(30);

fn person(actor: &str) -> ExecutionIdentity {
    ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: actor.into(),
        conversation: "C1".into(),
        private: false,
    }
}

fn thread() -> Address {
    Address::new("test", &person("U1").conversation_key(), "t1").unwrap()
}

fn said(actor: &str, request: &str, text: &str) -> Prompt {
    Prompt::new(thread(), request, "system", text)
        .unwrap()
        .with_identity(Some(person(actor)))
        .with_delivery(DeliveryTarget::new("test", json!({"thread": "t1"})).unwrap())
}

/// U1's finished, delivered message in thread `t1`.
async fn origin(store: &MemoryStore) -> OperationId {
    let id = store
        .accept(&said("U1", "1", "set it up"))
        .await
        .unwrap()
        .operation
        .id;
    let claim = store
        .claim_operation(id, "w", LEASE)
        .await
        .unwrap()
        .unwrap();
    finish(store, &claim.lease).await;
    id
}

async fn finish(store: &MemoryStore, lease: &crate::Lease) {
    store.record(lease, &Event::RunStarted).await.unwrap();
    let outcome = Outcome::Completed { text: "ok".into() };
    store
        .record(lease, &Event::RunFinished { outcome })
        .await
        .unwrap();
    let claim = DeliveryClaim::new("test", "surface", LEASE, 32).unwrap();
    while let Some(batch) = store.claim_delivery(&claim).await.unwrap() {
        let through = batch.events.last().map_or(0, |event| event.sequence);
        let checkpoint = DeliveryCheckpoint::new(batch.lease, through, None);
        store.checkpoint_delivery(&checkpoint).await.unwrap();
    }
}

fn make_due(store: &MemoryStore, id: ScheduleId) {
    let mut state = store.state.lock().unwrap();
    state
        .schedules
        .rows
        .get_mut(&id)
        .unwrap()
        .scheduled
        .run_at_unix_ms = 0;
}

fn daily() -> ScheduleTime {
    ScheduleTime::Repeat(Recurrence::new(Time::constant(9, 0, 0, 0), "UTC", None).unwrap())
}

fn elsewhere() -> Destination {
    let identity = ExecutionIdentity {
        conversation: "C9".into(),
        ..person("U1")
    };
    Destination {
        surface: "test".into(),
        conversation: identity.conversation_key(),
        target: DeliveryTarget::new("test", json!({"post": "C9"})).unwrap(),
        identity,
    }
}

#[tokio::test]
async fn a_stopped_thread_pauses_what_replies_in_it_until_a_mention_resumes_it() {
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    let here = ScheduleRequest::new(origin, "here", "standup", daily()).unwrap();
    let here = store.schedule(&here).await.unwrap().id;
    let post = ScheduleRequest::new(origin, "post", "post it", daily())
        .unwrap()
        .with_destination(elsewhere());
    let post = store.schedule(&post).await.unwrap().id;
    store.unfollow_lane(origin, "stop-call").await.unwrap();

    make_due(&store, here);
    make_due(&store, post);
    let run = store
        .claim("w", LEASE)
        .await
        .unwrap()
        .expect("the post still runs");
    assert_eq!(
        run.identity.as_ref().map(|i| i.conversation.as_str()),
        Some("C9")
    );
    finish(&store, &run.lease).await;
    assert!(
        store.claim("w", LEASE).await.unwrap().is_none(),
        "the thread's schedule waits"
    );
    let listed = store.schedules(origin).await.unwrap();
    let paused: Vec<_> = listed.iter().map(|s| (s.id, s.paused)).collect();
    assert!(
        paused.contains(&(here, true)) && paused.contains(&(post, false)),
        "{paused:?}"
    );

    store.follow_lane(&thread(), "mention-2").await.unwrap();
    let resumed = store
        .claim("w", LEASE)
        .await
        .unwrap()
        .expect("one catch-up run");
    assert_eq!(
        resumed.thread.last(),
        &agent::Message::User("standup".into())
    );
    assert!(
        store
            .schedules(origin)
            .await
            .unwrap()
            .iter()
            .all(|s| !s.paused)
    );
}

#[tokio::test]
async fn the_owners_stop_reaches_a_scheduled_run_in_progress_and_nobody_elses_does() {
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    let request =
        ScheduleRequest::new(origin, "k", "summarize", ScheduleTime::AfterSeconds(0)).unwrap();
    store.schedule(&request).await.unwrap();
    let run = store
        .claim("w", LEASE)
        .await
        .unwrap()
        .expect("the scheduled run");

    let stop = store
        .admit_with_tools(&said("U1", "2", "stop"), &[])
        .await
        .unwrap();
    assert!(matches!(stop, Admitted::Steering { .. }), "{stop:?}");
    let steering = store.steering(&run.lease).await.unwrap();
    assert_eq!(steering, vec![agent::Message::User("stop".into())]);
    let other = store
        .admit_with_tools(&said("U2", "3", "stop"), &[])
        .await
        .unwrap();
    assert!(
        matches!(other, Admitted::Operation(_)),
        "another person's message queues under their own authority: {other:?}"
    );
}
