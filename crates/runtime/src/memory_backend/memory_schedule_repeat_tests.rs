//! Standing-schedule behavior through the public store seams; the only
//! backdoor is moving a row's run time into the past instead of waiting a day.

#![allow(clippy::too_many_lines)]

use std::time::Duration;

use agent::{Event, Outcome};
use jiff::civil::Time;
use serde_json::json;

use crate::schedule_repeat::{MAX_REPEAT_PER_LANE, now_unix_ms};
use crate::{
    Address, DeliveryCheckpoint, DeliveryClaim, DeliveryStore, DeliveryTarget, Error, MemoryStore,
    OperationId, Prompt, Recurrence, ScheduleId, ScheduleRequest, ScheduleState, ScheduleStore,
    ScheduleTime, Store,
};

const LEASE: Duration = Duration::from_secs(30);

fn identity() -> agent::ExecutionIdentity {
    agent::ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: "U1".into(),
        conversation: "D1".into(),
        private: true,
    }
}

fn eight(zone: &str) -> ScheduleTime {
    ScheduleTime::Repeat(Recurrence::new(Time::constant(8, 0, 0, 0), zone, None).unwrap())
}

async fn origin(store: &MemoryStore) -> OperationId {
    let address = Address::new("test", "D1", "D1").unwrap();
    let prompt = Prompt::new(address, "origin", "system", "set up my briefing")
        .unwrap()
        .with_identity(Some(identity()))
        .with_delivery(DeliveryTarget::new("test", json!({"conversation": "D1"})).unwrap());
    let id = store.accept(&prompt).await.unwrap().operation.id;
    run(store, &Outcome::Completed { text: "ok".into() }).await;
    deliver(store).await;
    id
}

/// Claim the next operation and finish it with `outcome`; returns its id.
async fn run(store: &MemoryStore, outcome: &Outcome) -> OperationId {
    let claim = store
        .claim("worker", LEASE)
        .await
        .unwrap()
        .expect("due work");
    store
        .record(&claim.lease, &Event::RunStarted)
        .await
        .unwrap();
    let finished = Event::RunFinished {
        outcome: outcome.clone(),
    };
    store.record(&claim.lease, &finished).await.unwrap();
    claim.operation.id
}

/// Deliver the next batch to the surface and return its runtime notice.
async fn deliver(store: &MemoryStore) -> Option<String> {
    let claim = DeliveryClaim::new("test", "surface", LEASE, 32).unwrap();
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

#[tokio::test]
async fn a_standing_schedule_is_released_again_after_every_run() {
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    let request =
        ScheduleRequest::new(origin, "daily", "brief me", eight("America/Toronto")).unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    assert!(scheduled.repeat.is_some());
    assert!(scheduled.run_at_unix_ms > now_unix_ms());
    assert_eq!(
        store.schedule(&request).await.unwrap(),
        scheduled,
        "idempotent retry"
    );
    let other = ScheduleRequest::new(origin, "daily", "brief me", eight("Europe/Lisbon")).unwrap();
    assert_eq!(
        store.schedule(&other).await,
        Err(Error::IdempotencyConflict)
    );

    make_due(&store, scheduled.id);
    let claim = store
        .claim("worker", LEASE)
        .await
        .unwrap()
        .expect("promoted");
    assert_eq!(claim.identity, Some(identity()));
    assert_eq!(
        claim.thread.last(),
        &agent::Message::User("brief me".into())
    );
    let listed = store.schedules(origin).await.unwrap();
    assert_eq!(listed.len(), 1, "the row stays pending: {listed:?}");
    assert_eq!(listed[0].state, ScheduleState::Pending);
    assert!(
        listed[0].run_at_unix_ms > now_unix_ms(),
        "advanced to the next occurrence"
    );

    make_due(&store, scheduled.id);
    assert!(store.claim("other", LEASE).await.unwrap().is_none());
    assert_eq!(
        store.schedules(origin).await.unwrap()[0].run_at_unix_ms,
        0,
        "a run still in flight blocks the next release"
    );
    let finished = Event::RunFinished {
        outcome: Outcome::Completed {
            text: "done".into(),
        },
    };
    store.record(&claim.lease, &finished).await.unwrap();
    let second = store
        .claim("worker", LEASE)
        .await
        .unwrap()
        .expect("released again");
    assert_ne!(second.operation.id, claim.operation.id);
    assert_eq!(second.operation.lane_id, claim.operation.lane_id);
    assert_eq!(
        second.identity,
        Some(identity()),
        "the principal survives every release"
    );
}

#[tokio::test]
async fn failure_streaks_nudge_at_three_reset_on_success_and_cancel_at_fourteen() {
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    let request = ScheduleRequest::new(origin, "daily", "brief me", eight("UTC")).unwrap();
    let id = store.schedule(&request).await.unwrap().id;
    let failed = Outcome::Failed {
        code: "model_unavailable".into(),
        message: "down".into(),
    };
    let mut notices = Vec::new();
    for _ in 0..3 {
        make_due(&store, id);
        run(&store, &failed).await;
        notices.push(deliver(&store).await);
    }
    assert_eq!(notices[0], None);
    assert_eq!(notices[1], None);
    assert!(
        notices[2]
            .as_deref()
            .is_some_and(|n| n.contains("3 runs in a row")),
        "{notices:?}"
    );

    make_due(&store, id);
    run(
        &store,
        &Outcome::Completed {
            text: "fine".into(),
        },
    )
    .await;
    assert_eq!(deliver(&store).await, None, "a success resets the streak");

    let mut last = None;
    for _ in 0..14 {
        make_due(&store, id);
        run(&store, &failed).await;
        last = deliver(&store).await;
    }
    assert!(
        last.as_deref().is_some_and(|n| n.contains("cancelled")),
        "{last:?}"
    );
    assert!(
        store.schedules(origin).await.unwrap().is_empty(),
        "the row is cancelled"
    );
    make_due(&store, id);
    assert!(store.claim("worker", LEASE).await.unwrap().is_none());
}

#[tokio::test]
async fn a_lane_holds_a_bounded_number_of_standing_schedules() {
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    for index in 0..MAX_REPEAT_PER_LANE {
        let request =
            ScheduleRequest::new(origin, &format!("k{index}"), "x", eight("UTC")).unwrap();
        store.schedule(&request).await.unwrap();
    }
    let extra = ScheduleRequest::new(origin, "extra", "x", eight("UTC")).unwrap();
    assert_eq!(
        store.schedule(&extra).await,
        Err(Error::ScheduleLimitExceeded)
    );
    let one_shot =
        ScheduleRequest::new(origin, "once", "x", ScheduleTime::AfterSeconds(60)).unwrap();
    assert!(
        store.schedule(&one_shot).await.is_ok(),
        "one-shots are not capped"
    );
}

#[tokio::test]
async fn an_interval_schedule_runs_on_whole_steps_from_its_creation() {
    const STEP: i64 = 30 * 60 * 1_000;
    let store = MemoryStore::default();
    let origin = origin(&store).await;
    let every_half_hour = crate::RepeatSpec {
        every_minutes: Some(30),
        ..crate::RepeatSpec::default()
    };
    let when = ScheduleTime::Repeat(Recurrence::try_from(every_half_hour).unwrap());
    let request = ScheduleRequest::new(origin, "pulse", "check the queue", when).unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    let anchor = store.state.lock().unwrap().schedules.rows[&scheduled.id].anchor_unix_ms;
    assert_eq!(
        scheduled.run_at_unix_ms,
        anchor + STEP,
        "one step after creation"
    );

    make_due(&store, scheduled.id);
    let claim = store
        .claim("worker", LEASE)
        .await
        .unwrap()
        .expect("promoted");
    assert_eq!(
        claim.thread.last(),
        &agent::Message::User("check the queue".into())
    );
    let next = store.schedules(origin).await.unwrap()[0].run_at_unix_ms;
    assert!(next > now_unix_ms(), "advanced past now");
    assert_eq!((next - anchor) % STEP, 0, "stays on the creation grid");
}
