// Lifecycle scenarios cover the full recurrence and cancellation boundaries.
#![allow(clippy::too_many_lines)]
use agent::{ExecutionIdentity, Outcome};
use brainstone_runtime::{
    Address, DurableStore, LaneStore, Prompt, Recurrence, RepeatSpec, ScheduleRequest,
    ScheduleStore, ScheduleTime, Store,
};
use std::time::Duration;

pub(crate) async fn check(url: &str, store: &DurableStore) {
    let identity = ExecutionIdentity {
        provider: "repeat".to_owned(),
        tenant: "contract".to_owned(),
        actor: "owner".to_owned(),
        conversation: "direct".to_owned(),
        private: true,
    };
    let address = Address::new("repeat", &identity.conversation_key(), "lane").unwrap();
    let prompt = Prompt::new(address.clone(), "one", "s", "repeat")
        .unwrap()
        .with_identity(Some(identity.clone()));
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let rule = Recurrence::try_from(RepeatSpec {
        every_minutes: Some(5),
        ..RepeatSpec::default()
    })
    .unwrap();
    let request = ScheduleRequest::new(
        id,
        "repeat",
        "follow up",
        ScheduleTime::Repeat(rule.clone()),
    )
    .unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    assert_eq!(scheduled.repeat, Some(rule));
    super::contracts::finish(store, id).await;
    force_due(url, scheduled.id.get(), 0).await;
    store.unfollow_lane(id, "repeat-stop").await.unwrap();
    assert!(store.schedules(id).await.unwrap()[0].paused);
    assert!(
        store
            .claim("paused", Duration::from_secs(30))
            .await
            .unwrap()
            .is_none()
    );
    store.follow_lane(&address, "resume").await.unwrap();
    for failure in 1..=brainstone_runtime::REPEAT_FAILURE_CANCEL_AFTER {
        force_due(url, scheduled.id.get(), i64::from(failure)).await;
        let run = store
            .claim("scheduler", Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(run.identity, Some(identity.clone()));
        assert!(
            store
                .claim("no-overlap", Duration::from_secs(30))
                .await
                .unwrap()
                .is_none()
        );
        store
            .settle(
                &run.lease,
                &Outcome::Failed {
                    code: "test_failure".to_owned(),
                    message: "failed".to_owned(),
                },
            )
            .await
            .unwrap();
        let remaining = store.schedules(id).await.unwrap();
        assert_eq!(
            remaining.is_empty(),
            failure == brainstone_runtime::REPEAT_FAILURE_CANCEL_AFTER
        );
    }
    assert!(
        store
            .claim("cancelled", Duration::from_secs(30))
            .await
            .unwrap()
            .is_none()
    );
}

async fn force_due(url: &str, id: i64, time: i64) {
    const SQL: &str = "UPDATE scheduled_work SET run_at_unix_ms=$2 WHERE id=$1";
    if url.starts_with("sqlite:") {
        let pool = sqlx::SqlitePool::connect(url).await.unwrap();
        sqlx::query(SQL)
            .bind(id)
            .bind(time)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    } else {
        let pool = sqlx::PgPool::connect(url).await.unwrap();
        sqlx::query(SQL)
            .bind(id)
            .bind(time)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }
}
